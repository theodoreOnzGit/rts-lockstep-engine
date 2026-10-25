// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code). It names, for each method, the
// upstream call it replaces; see `logic::weapon_host` for the code it serves.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! [`WeaponXferLookups`] and [`WeaponHost`] for [`World`] (layer 12, batch W1b): the weapon code
//! (`logic::weapon_fire`, `logic::weapon_set_state`) run on the game's own objects.
//!
//! # The shape
//!
//! Each method is a thin delegation to what the world already has, in the order the weapon code makes the call: the objects' own
//! fields; `World`'s functions for the object system (`attempt_damage`, `estimate_damage`, `is_able_to_attack`, the relationships, the
//! controlling player); a [`ModuleCall`] to the module that owns the interface upstream asks (the contain, the AI, the stealth update, the
//! spawn behaviour, the projectile update, the special power completion die, the land mine, the parking place, the physics); the world's
//! cut points for the systems not yet behind the world (the terrain's heights, the partition manager's scans, the pathfinder, the
//! object creation lists, the client: [`CutCall`] records and `cuts` tables, as every other module reaches them). So the weapon code meets
//! the same world the modules meet, and the layer-12 work that puts the real terrain, partition manager and pathfinder behind those cut
//! points serves both at once.
//!
//! # The store
//!
//! `TheWeaponStore` is `statics.weapon_store`. The weapon code changes it (a template's historic damage, the delayed damage list):
//! [`WeaponHost::store_mut`] takes it with `Arc::make_mut`, which copies the statics only when another holder of the `Arc` is alive at
//! that moment (the world then owns its own copy from there on); the world's own code holds such a clone only inside a call.
//!
//! # Verified
//!
//! The weapon code itself is verified against upstream's `Weapon.cpp` and `WeaponSet.cpp` by `tests/differential_weapon_fire.rs`
//! (on its scenario host); the world's functions these methods delegate to are verified by their own tests; the delegations are pinned by
//! the unit tests below.

use std::sync::Arc;

use crate::common::base_type::{Coord3D, Region3D};
use crate::common::game_common::{PlayerMaskType, Relationship};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID};
use crate::common::geometry::GeometryInfo;
use crate::common::kind_of::{KindOfMaskType, KindOfType};
use crate::common::name_key_generator::NameKeyType;
use crate::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_AIRBORNE_TARGET};
use crate::common::random_value::RandomValues;
use crate::common::sparse_match_finder::SparseMatchFinder;
use crate::logic::damage::{DamageInfo, DamageInfoInput};
use crate::logic::weapon::{FxName, WeaponError, WeaponStore};
use crate::logic::weapon_bonus::{WeaponBonus, WeaponBonusSet};
use crate::logic::weapon_types::WeaponStatus;
use crate::logic::weapon_host::{
    AbleToAttackType, CanAttackResult, CommandSourceType, ContainFacts, ContainedItems, DistanceCalculationType, LaunchOffset, WeaponFx,
    WeaponHost, WeaponTemplateSetRef, WeaponXferLookups,
};
use crate::logic::weapon_set::WeaponTemplateSet;
use crate::logic::weapon_set_flags::WeaponSetFlags;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleReply};
use crate::logic::world::object_weapons::ObjectWeaponHost;
use crate::logic::weapon_fire::Weapon;
use crate::logic::weapon_set_state::WeaponSet;
use crate::common::model_state::{ModelConditionFlags, MODELCONDITION_COUNT};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::ww::matrix3d::Matrix3D;

impl World {
    /// The module index answering a `Has...Interface` question with yes, if any.
    fn module_with_interface(&mut self, o: ObjectId, has: ModuleCall) -> Option<ModuleIndex> {
        let n = self.objects.find(o)?.modules.len();
        (0..n).map(|i| i as ModuleIndex).find(|&i| self.module_call(o, i, has.clone()).as_bool())
    }

    /// The contain module's answer, `None` without a contain module.
    fn contain_ask(&mut self, o: ObjectId, call: ModuleCall) -> Option<ModuleReply> {
        let c = self.get_contain(o)?;
        Some(self.module_call(o, c, call))
    }

    /// The AI's answer, `None` without an AI.
    fn ai_ask(&mut self, o: ObjectId, call: ModuleCall) -> Option<ModuleReply> {
        let ai = self.objects.find(o)?.ai?;
        Some(self.module_call(o, ai, call))
    }
}

impl WeaponXferLookups for World {
    fn find_thing_template(&self, name: &[u8]) -> Option<u16> {
        self.statics.thing_factory.find_template(name).map(|t| t.get_final_override().data.get_template_id())
    }
    fn thing_template_name(&self, id: u16) -> Vec<u8> {
        self.statics.thing_factory.find_by_template_id(id).map(|t| t.get_final_override().data.get_name().to_vec()).unwrap_or_default()
    }
    fn find_weapon_template_set_by_flags(&mut self, thing_template: u16, flags: &WeaponSetFlags) -> Option<WeaponTemplateSetRef> {
        let t = self.statics.thing_factory.find_by_template_id(thing_template)?;
        let sets = t.get_final_override().data.get_weapon_template_sets();
        // (the template's match cache answers what the slow search does)
        SparseMatchFinder::find_best_info_slow::<_, WeaponTemplateSet>(sets, flags).map(|index| WeaponTemplateSetRef { thing_template, index })
    }
    fn weapon_template_set(&self, r: WeaponTemplateSetRef) -> &WeaponTemplateSet {
        &self.statics.thing_factory.find_by_template_id(r.thing_template).expect("a set ref names a template of the factory").get_final_override().data.get_weapon_template_sets()
            [r.index]
    }
    fn weapon_template_names(&self) -> Vec<(Vec<u8>, usize)> {
        self.statics.weapon_store.templates().iter().enumerate().map(|(i, t)| (t.name.to_vec(), i)).collect()
    }
    fn weapon_template_index(&self, key: NameKeyType) -> Option<usize> {
        self.statics.weapon_store.templates().iter().position(|t| t.name_key == key)
    }
    fn weapon_template_name(&self, index: usize) -> Vec<u8> {
        self.statics.weapon_store.templates()[index].name.to_vec()
    }
}

impl WeaponHost for World {
    // ---- TheGameLogic, TheGlobalData, TheWeaponStore, the random numbers --------------------------------------

    fn frame(&self) -> u32 {
        World::frame(self)
    }
    fn object_exists(&self, o: ObjectId) -> bool {
        self.objects.find(o).is_some()
    }
    fn store(&self) -> &WeaponStore {
        &self.statics.weapon_store
    }
    fn store_mut(&mut self) -> &mut WeaponStore {
        &mut Arc::make_mut(&mut self.statics).weapon_store
    }
    fn random_int(&mut self, lo: i32, hi: i32) -> i32 {
        self.random.game_logic_random_value(lo, hi)
    }
    fn random_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.random.game_logic_random_value_real(lo, hi)
    }
    fn delay_between_shots(&mut self, template: usize, bonus: &WeaponBonus) -> i32 {
        let (statics, rng) = (&self.statics, &mut self.random);
        statics.weapon_store.templates()[template].get_delay_between_shots(bonus, rng)
    }
    fn global_weapon_bonus_set(&self) -> Option<&WeaponBonusSet> {
        self.statics.global_data.the_global_data().map(|g| &g.weapon_bonus_set)
    }
    fn historic_damage_limit(&self) -> u32 {
        self.statics.global_data.the_global_data().map_or(0, |g| g.historic_damage_limit)
    }
    fn rng(&mut self) -> &mut RandomValues {
        &mut self.random
    }

    // ---- an Object's own state -----------------------------------------------------------------------------------

    fn position(&self, o: ObjectId) -> Coord3D {
        self.objects.find(o).map(|x| x.get_position()).unwrap_or_default()
    }
    fn orientation(&self, o: ObjectId) -> f32 {
        self.objects.find(o).map_or(0.0, |x| x.thing.orientation())
    }
    fn transform_matrix(&self, o: ObjectId) -> Matrix3D {
        self.objects.find(o).map_or_else(Matrix3D::identity, |x| *x.thing.transform_matrix())
    }
    fn geometry(&self, o: ObjectId) -> GeometryInfo {
        self.objects.find(o).map_or_else(|| GeometryInfo::new(crate::common::geometry::GeometryType::Sphere, false, 0.0, 0.0, 0.0), |x| x.geometry_info.clone())
    }
    fn is_kind_of(&self, o: ObjectId, kind: KindOfType) -> bool {
        self.objects.find(o).is_some_and(|x| x.is_kind_of(kind))
    }
    fn is_kind_of_multi(&self, o: ObjectId, must_be_set: &KindOfMaskType, must_be_clear: &KindOfMaskType) -> bool {
        self.objects.find(o).is_some_and(|x| x.thing.is_kind_of_multi(must_be_set, must_be_clear))
    }
    fn test_status(&self, o: ObjectId, status: ObjectStatusTypes) -> bool {
        self.objects.find(o).is_some_and(|x| x.status.test(status))
    }
    fn test_script_status_bit(&self, o: ObjectId, bit: u32) -> bool {
        self.objects.find(o).is_some_and(|x| u32::from(x.script_status) & bit != 0)
    }
    fn veterancy_level(&self, o: ObjectId) -> i32 {
        self.objects.find(o).map_or(0, |x| x.get_veterancy_level() as i32)
    }
    fn has_drawable(&self, o: ObjectId) -> bool {
        World::has_drawable(self, o)
    }
    fn is_locally_controlled(&mut self, o: ObjectId) -> bool {
        World::is_locally_controlled(self, o)
    }
    fn weapon_bonus_condition(&self, o: ObjectId) -> u32 {
        self.objects.find(o).map_or(0, |x| x.weapon_bonus_condition)
    }
    fn contained_by(&self, o: ObjectId) -> Option<ObjectId> {
        self.objects.find(o).map(|x| x.contained_by).filter(|&c| c != INVALID_ID)
    }
    fn container_passed_weapon_bonus(&mut self, container: ObjectId) -> Option<u32> {
        if !self.contain_ask(container, ModuleCall::IsWeaponBonusPassedToPassengers)?.as_bool() {
            return None;
        }
        Some(self.contain_ask(container, ModuleCall::GetWeaponBonusPassedToPassengers)?.as_int(0) as u32)
    }
    fn is_reload_time_shared(&self, o: ObjectId) -> bool {
        match self.objects.find(o).and_then(|x| x.weapon_set.cur_weapon_template_set()) {
            Some(r) => self.weapon_template_set(r).is_shared_reload_time(),
            None => false,
        }
    }
    fn apply_shared_reload(&mut self, o: ObjectId, next_shot_frame: u32, status: WeaponStatus) {
        let Some(x) = self.objects.find_mut(o) else { return };
        for w in x.weapon_set.slots_mut().iter_mut().flatten() {
            w.set_possible_next_shot_frame(next_shot_frame);
            w.set_status(status);
        }
    }
    fn num_consecutive_shots_fired_at_target(&mut self, o: ObjectId, victim: Option<ObjectId>) -> i32 {
        // (the firing tracker helper is not an object module yet: the table `cuts.consecutive_shots_at_victim`)
        self.cuts.consecutive_shots_at_victim.get(&(o, victim.unwrap_or(INVALID_ID))).copied().unwrap_or(0)
    }
    fn is_above_terrain(&mut self, o: ObjectId) -> bool {
        World::is_above_terrain(self, o)
    }
    fn is_airborne_target(&mut self, o: ObjectId) -> bool {
        // `m_status.test( OBJECT_STATUS_AIRBORNE_TARGET )` (Object.h)
        self.objects.find(o).is_some_and(|x| x.status.test(OBJECT_STATUS_AIRBORNE_TARGET))
    }
    fn is_significantly_above_terrain(&mut self, o: ObjectId) -> bool {
        World::is_significantly_above_terrain(self, o)
    }
    fn layer(&self, o: ObjectId) -> PathfindLayer {
        self.objects.find(o).map_or_else(PathfindLayer::default, |x| x.layer)
    }
    fn producer_id(&self, o: ObjectId) -> ObjectId {
        self.objects.find(o).map_or(INVALID_ID, |x| x.producer_id)
    }
    fn relationship(&self, a: ObjectId, b: ObjectId) -> Relationship {
        self.get_relationship(a, b)
    }
    fn is_equivalent_template(&self, a: ObjectId, b: ObjectId) -> bool {
        let Some(t) = self.objects.find(a).map(|x| x.thing.template().get_template_id()) else { return false };
        self.template_is_equivalent_to_object(t, b)
    }
    fn fence_width(&self, o: ObjectId) -> f32 {
        self.objects.find(o).map_or(0.0, |x| x.thing.template().get_fence_width())
    }
    fn is_effectively_dead(&self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.is_effectively_dead())
    }
    fn is_destroyed(&self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.is_destroyed())
    }
    fn is_able_to_attack(&mut self, o: ObjectId) -> bool {
        World::is_able_to_attack(self, o)
    }
    fn team_relationship_to_player_index(&self, o: ObjectId, player_index: i32) -> Relationship {
        let team = self.objects.find(o).map_or(0, |x| x.team);
        let default_team = self.cuts.player_default_team(player_index);
        self.cuts.team_relationship(team, (default_team != 0).then_some(default_team))
    }
    fn find_weapon_template_set(&mut self, o: ObjectId) -> Option<WeaponTemplateSetRef> {
        let (t, flags) = self.objects.find(o).map(|x| (x.thing.template().get_template_id(), x.cur_weapon_set_flags))?;
        self.find_weapon_template_set_by_flags(t, &flags)
    }
    fn estimate_damage(&mut self, o: ObjectId, input: &DamageInfoInput) -> f32 {
        World::estimate_damage(self, o, input)
    }
    fn attempt_damage(&mut self, o: ObjectId, info: &mut DamageInfo) {
        World::attempt_damage(self, o, info)
    }
    fn destroy_object(&mut self, o: ObjectId) {
        World::destroy_object(self, o)
    }

    // ---- an Object's player ---------------------------------------------------------------------------------------

    fn controlling_player(&self, o: ObjectId) -> Option<i32> {
        self.get_controlling_player(o)
    }
    fn controlling_player_mask(&self, o: ObjectId) -> Option<PlayerMaskType> {
        self.get_controlling_player(o).map(|p| self.cuts.player_mask(p))
    }
    fn controlling_player_battle_plans(&self, o: ObjectId) -> i32 {
        self.get_controlling_player(o).map_or(0, |p| self.cuts.player_num_battle_plans_active(p))
    }
    fn apply_battle_plan_bonuses(&mut self, source: ObjectId, projectile: ObjectId) {
        if let Some(player) = self.get_controlling_player(source) {
            self.cut(CutCall::PlayerApplyBattlePlanBonusesForObject { player, object: projectile });
        }
    }
    fn record_mine_cleared(&mut self, o: ObjectId) {
        if let Some(player) = self.get_controlling_player(o) {
            self.cut(CutCall::AcademyStatsRecordMineCleared { player });
        }
    }
    fn controlling_player_objects(&self, o: ObjectId) -> Vec<ObjectId> {
        self.get_controlling_player(o).and_then(|p| self.cuts.player_object_list.get(&p).cloned()).unwrap_or_default()
    }

    // ---- modules of an Object ---------------------------------------------------------------------------------------

    fn contain_facts(&mut self, o: ObjectId) -> Option<ContainFacts> {
        let count = self.contain_ask(o, ModuleCall::GetContainCount)?.as_int(0);
        let garrisonable = self.contain_ask(o, ModuleCall::IsGarrisonable)?.as_bool();
        let immune_to_clear_building_attacks = self.contain_ask(o, ModuleCall::IsImmuneToClearBuildingAttacks)?.as_bool();
        Some(ContainFacts { count, garrisonable, immune_to_clear_building_attacks })
    }
    fn contain_is_enclosing_container_for(&mut self, container: ObjectId, inner: ObjectId) -> bool {
        self.contain_ask(container, ModuleCall::IsEnclosingContainerFor(inner)).is_some_and(|r| r.as_bool())
    }
    fn contain_is_garrisonable(&mut self, container: ObjectId) -> bool {
        self.contain_ask(container, ModuleCall::IsGarrisonable).is_some_and(|r| r.as_bool())
    }
    fn contain_calc_best_garrison_position(&mut self, container: ObjectId, target_pos: &Coord3D) -> Option<Coord3D> {
        match self.contain_ask(container, ModuleCall::CalcBestGarrisonPosition { target_pos: *target_pos })? {
            ModuleReply::Coord(p) => Some(p),
            _ => None,
        }
    }
    fn contain_is_passenger_allowed_to_fire(&mut self, o: ObjectId) -> bool {
        self.contain_ask(o, ModuleCall::IsPassengerAllowedToFire(INVALID_ID)).is_some_and(|r| r.as_bool())
    }
    fn contain_items(&mut self, o: ObjectId) -> Option<ContainedItems> {
        match self.contain_ask(o, ModuleCall::GetContainedItems)? {
            ModuleReply::Ids(ids) => Some(ids),
            _ => None,
        }
    }
    fn contain_apparent_controlling_player(&mut self, o: ObjectId, observer: Option<i32>) -> Option<i32> {
        match self.contain_ask(o, ModuleCall::GetApparentControllingPlayer { observer: observer.unwrap_or(-1) })? {
            ModuleReply::Int(p) if p >= 0 => Some(p),
            _ => None,
        }
    }
    fn has_contain(&mut self, o: ObjectId) -> bool {
        self.get_contain(o).is_some()
    }
    fn stealth_disguised_player_index(&mut self, o: ObjectId) -> Option<i32> {
        let s = self.objects.find(o)?.stealth?;
        match self.module_call(o, s, ModuleCall::StealthGetDisguisedPlayerIndex) {
            ModuleReply::Int(p) => Some(p),
            _ => None,
        }
    }
    fn player_relationship_to_player(&self, observer: Option<i32>, other_index: i32) -> Option<Relationship> {
        let observer = observer?;
        self.players.get_nth(other_index)?;
        self.players.get_nth(observer)?;
        Some(self.cuts.player_relationship_to_player_default_team(observer, other_index))
    }
    fn spawn_slaves_can_use_weapon_against_target(
        &mut self,
        o: ObjectId,
        attack_type: AbleToAttackType,
        victim: Option<ObjectId>,
        pos: Option<Coord3D>,
        command_source: CommandSourceType,
    ) -> Option<CanAttackResult> {
        let spawn = self.module_with_interface(o, ModuleCall::HasSpawnBehaviorInterface)?;
        let call = ModuleCall::SpawnCanAnySlavesUseWeaponAgainstTarget {
            attack_type,
            victim: victim.unwrap_or(INVALID_ID),
            pos,
            command_source: command_source as i32,
        };
        Some(can_attack_result(self.module_call(o, spawn, call).as_int(0)))
    }
    fn member_able_to_use_weapon_against_target(
        &mut self,
        member: ObjectId,
        attack_type: AbleToAttackType,
        victim: Option<ObjectId>,
        pos: Option<Coord3D>,
        command_source: CommandSourceType,
    ) -> Result<CanAttackResult, WeaponError> {
        crate::logic::world::object_weapons::get_able_to_use_weapon_against_target(self, member, attack_type, victim, pos, command_source, -1)
    }

    // ---- the AI of an Object -----------------------------------------------------------------------------------------

    fn ai_sneaky_targeting_offset(&mut self, o: ObjectId) -> Option<Coord3D> {
        match self.ai_ask(o, ModuleCall::AiGetSneakyTargetingOffset)? {
            ModuleReply::Coord(c) => Some(c),
            _ => None,
        }
    }
    fn ai_cur_locomotor_set_type(&mut self, o: ObjectId) -> Option<i32> {
        Some(self.ai_ask(o, ModuleCall::AiGetCurLocomotorSetType)?.as_int(0))
    }
    fn ai_is_aircraft_that_adjusts_destination(&mut self, o: ObjectId) -> bool {
        self.ai_ask(o, ModuleCall::AiIsAircraftThatAdjustsDestination).is_some_and(|r| r.as_bool())
    }
    fn ai_begin_assault(&mut self, o: ObjectId, victim: Option<ObjectId>) {
        self.ai_ask(o, ModuleCall::AiBeginAssault { victim: victim.unwrap_or(INVALID_ID) });
    }
    fn ai_which_turret_for_weapon_slot(&mut self, o: ObjectId, wslot: i32) -> Option<(i32, f32, f32)> {
        match self.ai_ask(o, ModuleCall::AiGetWhichTurretForWeaponSlot { slot: wslot })? {
            ModuleReply::Turret { turret, angle, pitch } => Some((turret, angle, pitch)),
            _ => None,
        }
    }
    fn ai_is_weapon_slot_on_turret_and_aiming_at_target(&mut self, o: ObjectId, wslot: i32, victim: ObjectId) -> bool {
        self.ai_ask(o, ModuleCall::AiIsWeaponSlotOnTurretAndAimingAtTarget { slot: wslot, victim }).is_some_and(|r| r.as_bool())
    }
    fn adjust_target_destination(
        &mut self,
        source: ObjectId,
        target: Option<ObjectId>,
        pos: Option<Coord3D>,
        template: usize,
        approach_target_pos: &mut Coord3D,
    ) {
        // `TheAI->pathfinder()->adjustTargetDestination(...)`: the pathfinder is layer 11's; a record, the position as it is
        self.cut(CutCall::PathfinderAdjustTargetDestination { source, target: target.unwrap_or(INVALID_ID), pos, template, approach: *approach_target_pos });
    }

    // ---- the partition manager and the terrain --------------------------------------------------------------------

    fn distance_squared_to_object(&self, a: ObjectId, b: ObjectId, calc: DistanceCalculationType) -> f32 {
        self.partition_distance_squared(a, b, calc)
    }
    fn distance_squared_to_pos(&self, a: ObjectId, pos: &Coord3D, calc: DistanceCalculationType) -> f32 {
        self.partition_distance_squared_to_pos(a, pos, calc)
    }
    fn goal_distance_squared_to_object(&self, a: ObjectId, goal: &Coord3D, b: ObjectId, calc: DistanceCalculationType) -> f32 {
        self.partition_goal_distance_squared(a, goal, b, calc)
    }
    fn goal_distance_squared_to_pos(&self, a: ObjectId, goal: &Coord3D, pos: &Coord3D, calc: DistanceCalculationType) -> f32 {
        self.partition_goal_distance_squared_to_pos(a, goal, pos, calc)
    }
    fn vector_to_object(&self, a: ObjectId, b: ObjectId, calc: DistanceCalculationType) -> Coord3D {
        self.partition_vector_to(a, b, calc)
    }
    fn vector_to_pos(&self, a: ObjectId, pos: &Coord3D, calc: DistanceCalculationType) -> Coord3D {
        self.partition_vector_to_pos(a, pos, calc)
    }
    fn objects_in_range(&mut self, pos: &Coord3D, radius: f32, calc: DistanceCalculationType) -> Vec<(ObjectId, f32)> {
        self.partition_iterate_objects_in_range_numeric_at(*pos, radius, calc as i32)
    }
    fn potential_collisions(&self, source: ObjectId) -> Vec<ObjectId> {
        self.cut_partition_potential_collisions(source)
    }
    fn is_clear_line_of_sight_terrain(&self, from: &Coord3D, to: &Coord3D) -> bool {
        self.cut_partition_is_clear_line_of_sight_terrain(from, to)
    }
    fn bridge_attack_points(&self, bridge: ObjectId) -> (Coord3D, Coord3D) {
        self.cuts.bridge_attack_points.get(&bridge).copied().unwrap_or_default()
    }
    fn layer_height(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        self.terrain_get_layer_height(x, y, layer)
    }
    fn ground_height(&mut self, x: f32, y: f32) -> f32 {
        self.terrain_get_ground_height(x, y)
    }
    fn terrain_extent(&self) -> Region3D {
        self.terrain.extent().unwrap_or_default()
    }

    // ---- the thing factory, projectiles and what they carry ----------------------------------------------------------

    fn new_object_for_player_of(&mut self, template: u16, source: ObjectId) -> ObjectId {
        let team = self.get_controlling_player(source).map(|p| self.cuts.player_default_team(p)).filter(|&t| t != 0);
        self.thing_factory_new_object(template, team, Default::default()).unwrap_or(INVALID_ID)
    }
    fn set_producer(&mut self, projectile: ObjectId, source: ObjectId) {
        World::set_producer(self, projectile, source)
    }
    fn has_special_power_completion_die(&mut self, o: ObjectId) -> bool {
        self.module_with_interface(o, ModuleCall::IsSpecialPowerCompletionDie).is_some()
    }
    fn special_power_completion_die_notify_script_engine(&mut self, o: ObjectId) {
        if let Some(d) = self.module_with_interface(o, ModuleCall::IsSpecialPowerCompletionDie) {
            self.module_call(o, d, ModuleCall::SpecialPowerCompletionDieNotifyScriptEngine);
        }
    }
    fn special_power_completion_die_set_creator(&mut self, o: ObjectId, creator: ObjectId) {
        if let Some(d) = self.module_with_interface(o, ModuleCall::IsSpecialPowerCompletionDie) {
            self.module_call(o, d, ModuleCall::SetCreator { creator });
        }
    }
    fn has_projectile_update_interface(&mut self, o: ObjectId) -> bool {
        self.module_with_interface(o, ModuleCall::HasProjectileUpdateInterface).is_some()
    }
    fn projectile_launch(
        &mut self,
        projectile: ObjectId,
        victim: Option<ObjectId>,
        destination: &Coord3D,
        source: ObjectId,
        wslot: i32,
        barrel: i32,
        template: usize,
        exhaust: &FxName,
    ) {
        if let Some(p) = self.module_with_interface(projectile, ModuleCall::HasProjectileUpdateInterface) {
            let call = ModuleCall::ProjectileLaunchAtObjectOrPosition {
                victim: victim.unwrap_or(INVALID_ID),
                destination: *destination,
                source,
                wslot,
                barrel,
                template,
                exhaust: exhaust.clone(),
            };
            self.module_call(projectile, p, call);
        }
    }
    fn projectile_launcher_id(&mut self, o: ObjectId) -> Option<ObjectId> {
        let p = self.module_with_interface(o, ModuleCall::HasProjectileUpdateInterface)?;
        Some(self.module_call(o, p, ModuleCall::ProjectileGetLauncherId).as_id())
    }
    fn set_position(&mut self, o: ObjectId, pos: &Coord3D) {
        World::set_position(self, o, pos)
    }
    fn has_countermeasures(&mut self, o: ObjectId) -> bool {
        self.module_with_interface(o, ModuleCall::HasCountermeasuresInterface).is_some()
    }
    fn report_missile_for_countermeasures(&mut self, o: ObjectId, projectile: ObjectId) {
        if let Some(c) = self.module_with_interface(o, ModuleCall::HasCountermeasuresInterface) {
            self.module_call(o, c, ModuleCall::ReportMissileForCountermeasures { missile: projectile });
        }
    }
    fn has_land_mine_interface(&mut self, o: ObjectId) -> bool {
        self.module_with_interface(o, ModuleCall::HasLandMineInterface).is_some()
    }
    fn disarm_land_mine(&mut self, o: ObjectId) {
        if let Some(m) = self.module_with_interface(o, ModuleCall::HasLandMineInterface) {
            self.module_call(o, m, ModuleCall::LandMineDisarm);
        }
    }
    fn projectile_stream_add(
        &mut self,
        stream: ObjectId,
        source: ObjectId,
        projectile: ObjectId,
        victim: Option<ObjectId>,
        victim_pos: Option<Coord3D>,
    ) -> bool {
        let Some(s) = self.find_module(stream, b"ProjectileStreamUpdate") else { return false };
        let pos = self.objects.find(stream).map(|x| x.get_position()).unwrap_or_default();
        self.module_call(stream, s, ModuleCall::ProjectileStreamSetPosition(pos));
        self.module_call(stream, s, ModuleCall::ProjectileStreamAddProjectile { source, new_id: projectile, victim: victim.unwrap_or(INVALID_ID), victim_pos });
        true
    }
    fn assisted_targeting_is_free(&mut self, o: ObjectId) -> Option<bool> {
        let a = self.find_module(o, b"AssistedTargetingUpdate")?;
        Some(self.module_call(o, a, ModuleCall::AssistedTargetingIsFreeToAssist).as_bool())
    }
    fn assisted_targeting_assist_attack(&mut self, o: ObjectId, requesting: ObjectId, victim: ObjectId) {
        if let Some(a) = self.find_module(o, b"AssistedTargetingUpdate") {
            self.module_call(o, a, ModuleCall::AssistedTargetingAssistAttack { requesting, victim });
        }
    }
    fn laser_init(&mut self, laser: ObjectId, source: ObjectId, victim: Option<ObjectId>, pos: &Coord3D, bone_name: &[u8]) {
        if !self.has_drawable(laser) || !self.client_update_module_named(laser, b"LaserUpdate") {
            return;
        }
        let start = self.objects.find(source).map(|x| x.get_position()).unwrap_or_default();
        self.emit(ObjectClientEvent::LaserInit { laser, parent: source, target: victim.unwrap_or(INVALID_ID), start, end: *pos, bone: bone_name.to_vec() });
    }
    fn create_ocl(&mut self, ocl: u32, primary: ObjectId) {
        self.cut(CutCall::WeaponOclCreate { ocl, primary });
    }

    // ---- the drawable (model data the simulation reads) ---------------------------------------------------------------

    fn barrel_count(&mut self, o: ObjectId, wslot: i32) -> Option<i32> {
        if !self.has_drawable(o) {
            return None;
        }
        Some(self.cuts.drawable_barrel_count.get(&(o, wslot)).copied().unwrap_or(0))
    }
    fn projectile_launch_offset(&mut self, o: ObjectId, wslot: i32, barrel: i32, turret: i32) -> Option<LaunchOffset> {
        if !self.has_drawable(o) {
            return None;
        }
        self.cuts.drawable_launch_offset.get(&(o, wslot, barrel, turret)).copied()
    }
    fn convert_bone_transform_to_world(&self, o: ObjectId, bone_transform: &Matrix3D) -> Matrix3D {
        let Some(x) = self.objects.find(o) else { return *bone_transform };
        x.thing.convert_bone_pos_to_world_pos(&Coord3D::default(), bone_transform).1
    }
    fn weapon_fx(&mut self, fx: WeaponFx) {
        self.emit(ObjectClientEvent::WeaponFx(fx));
    }

    // ---- positioning a projectile for launch -------------------------------------------------------------------------------

    fn unhide_drawable(&mut self, o: ObjectId) {
        if self.has_drawable(o) {
            self.emit(ObjectClientEvent::DrawableSetHidden { object_id: o, hidden: false });
        }
    }
    fn set_transform_matrix(&mut self, o: ObjectId, m: &Matrix3D) {
        World::set_transform_matrix(self, o, m)
    }
    fn set_experience_sink(&mut self, o: ObjectId, sink: ObjectId) {
        if let Some(x) = self.objects.find_mut(o) {
            x.experience.sink = sink;
        }
    }
    fn transfer_velocity_and_ignore_collisions(&mut self, launcher: ObjectId, projectile: ObjectId) {
        let (Some(lp), Some(pp)) = (self.objects.find(launcher).and_then(|x| x.physics), self.objects.find(projectile).and_then(|x| x.physics)) else {
            return;
        };
        let v = self.module_call(launcher, lp, ModuleCall::PhysicsGetVelocity).as_coord();
        self.module_call(projectile, pp, ModuleCall::PhysicsTransferVelocityFrom(v));
        self.module_call(projectile, pp, ModuleCall::PhysicsSetIgnoreCollisionsWith(launcher));
    }
    fn parking_place_has_reserved_space(&mut self, o: ObjectId, intended: ObjectId) -> bool {
        match self.module_with_interface(o, ModuleCall::HasParkingPlaceInterface) {
            Some(p) => self.module_call(o, p, ModuleCall::ParkingPlaceHasReservedSpace { intended }).as_bool(),
            None => false,
        }
    }
}

impl ObjectWeaponHost for World {
    fn weapon_set(&self, o: ObjectId) -> &WeaponSet {
        &self.objects.find(o).expect("the weapon code asks for the set of an object that exists").weapon_set
    }
    fn take_weapon_set(&mut self, o: ObjectId) -> WeaponSet {
        self.objects.find_mut(o).map(|x| std::mem::take(&mut x.weapon_set)).unwrap_or_default()
    }
    fn put_weapon_set(&mut self, o: ObjectId, set: WeaponSet) {
        if let Some(x) = self.objects.find_mut(o) {
            x.weapon_set = set;
        }
    }
    fn weapon_set_mut(&mut self, o: ObjectId) -> &mut WeaponSet {
        &mut self.objects.find_mut(o).expect("the weapon code changes the set of an object that exists").weapon_set
    }
    fn take_weapon(&mut self, o: ObjectId, slot: i32) -> Option<Weapon> {
        self.objects.find_mut(o)?.weapon_set.slots_mut()[slot as usize].take()
    }
    fn put_weapon(&mut self, o: ObjectId, slot: i32, weapon: Weapon) {
        if let Some(x) = self.objects.find_mut(o) {
            x.weapon_set.slots_mut()[slot as usize] = Some(weapon);
        }
    }
    fn last_weapon_condition(&self, o: ObjectId, slot: usize) -> i8 {
        self.objects.find(o).map_or(0, |x| x.last_weapon_condition[slot])
    }
    fn set_last_weapon_condition(&mut self, o: ObjectId, slot: usize, v: i8) {
        if let Some(x) = self.objects.find_mut(o) {
            x.last_weapon_condition[slot] = v;
        }
    }
    fn firing_tracker_shot_fired(&mut self, o: ObjectId, weapon: &Weapon, victim: ObjectId) -> bool {
        // (the firing tracker helper is a stub module yet: a record when the object has one)
        if self.objects.find(o).and_then(|x| x.helpers.firing_tracker).is_none() {
            return false;
        }
        self.cut(CutCall::FiringTrackerShotFired { object: o, template: weapon.template(), victim });
        true
    }
    fn friend_set_undetected_defector(&mut self, o: ObjectId, undetected: bool) {
        World::friend_set_undetected_defector(self, o, undetected)
    }
    fn drawable_update_clip_status(&mut self, o: ObjectId, ammo_in_clip: u32, max_clip_size: i32, slot: i32) {
        if self.has_drawable(o) {
            self.emit(ObjectClientEvent::DrawableUpdateClipStatus { object_id: o, ammo_in_clip, max_clip_size, slot });
        }
    }
    fn drawable_clear_and_set_model_condition_flags(&mut self, o: ObjectId, clear: &ModelConditionFlags, set: &ModelConditionFlags) {
        let bits = |f: &ModelConditionFlags| (0..MODELCONDITION_COUNT).filter(|&i| f.test(i)).collect::<Vec<i32>>();
        let (c, s) = (bits(clear), bits(set));
        self.clear_and_set_model_condition_flags(o, &c, &s);
    }
    fn drawable_set_animation_loop_duration(&mut self, o: ObjectId, frames: u32) {
        if self.has_drawable(o) {
            self.emit(ObjectClientEvent::DrawableSetAnimationLoopDuration { object_id: o, frames });
        }
    }
}

/// `CanAttackResult` from its value (`ATTACKRESULT_NOT_POSSIBLE` for one out of range).
fn can_attack_result(v: i32) -> CanAttackResult {
    match v {
        3 => CanAttackResult::Possible,
        2 => CanAttackResult::PossibleAfterMoving,
        1 => CanAttackResult::InvalidShot,
        _ => CanAttackResult::NotPossible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::module::{MODULEINTERFACE_BODY, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE};
    use crate::common::thing_template::{ObjectTemplateValues, ThingTemplate};
    use crate::logic::damage::DAMAGE_EXPLOSION;
    use crate::logic::world::module_probe::{ProbeAnswers, ProbeModule};
    use crate::logic::world::object_create::{NewModule, NewObject};
    use crate::logic::world::statics::GameStatics;
    use crate::logic::world::update_body::UpdateBody;

    fn probe(name: &[u8], interfaces: i32, answers: ProbeAnswers, max: f32) -> NewModule {
        let mut m = NewModule::plain(name, b"ModuleTag_Probe");
        m.interfaces = interfaces;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, max, Default::default())));
        m
    }

    fn object(w: &mut World, modules: Vec<NewModule>, pos: Coord3D) -> ObjectId {
        let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
        let id = w.new_object(NewObject { template, status: Default::default(), team: None, modules }).unwrap();
        w.set_position(id, &pos);
        id
    }

    #[test]
    fn the_store_is_the_worlds_and_a_shared_one_is_copied_before_it_changes() {
        let mut statics = GameStatics::default();
        let mut names = crate::common::name_key_generator::NameKeyGenerator::new();
        statics.weapon_store.new_weapon_template(&mut names, b"Gun").unwrap();
        let mut w = World::new();
        w.statics = Arc::new(statics);
        let held = w.statics.clone();
        WeaponHost::store_mut(&mut w).templates_mut()[0].attack_range = 77.0;
        assert_eq!(WeaponHost::store(&w).templates()[0].attack_range, 77.0);
        assert_ne!(held.weapon_store.templates()[0].attack_range, 77.0, "the other holder keeps its own");
        assert_eq!(w.historic_damage_limit(), 0, "no global data: 0");
        assert!(w.global_weapon_bonus_set().is_none());
    }

    #[test]
    fn the_contain_facts_are_the_contain_modules_answers() {
        let mut w = World::new();
        let answers = ProbeAnswers { contained: vec![5, 6], garrisonable: true, ..Default::default() };
        let c = object(&mut w, vec![probe(b"Contain", MODULEINTERFACE_CONTAIN, answers, 0.0)], Coord3D::default());
        let none = object(&mut w, Vec::new(), Coord3D::default());
        let f = w.contain_facts(c).expect("a contain module");
        assert_eq!((f.count, f.garrisonable), (2, true));
        assert_eq!(w.contain_items(c), Some(vec![5, 6]));
        assert!(w.contain_facts(none).is_none() && !w.has_contain(none) && w.has_contain(c));
    }

    #[test]
    fn a_temporary_weapon_fired_through_the_world_damages_its_target() {
        let mut statics = GameStatics::default();
        let mut names = crate::common::name_key_generator::NameKeyGenerator::new();
        let gun = statics.weapon_store.new_weapon_template(&mut names, b"Gun").unwrap().unwrap();
        {
            let t = &mut statics.weapon_store.templates_mut()[gun];
            t.primary_damage = 25.0;
            t.attack_range = 100.0;
            t.damage_type = DAMAGE_EXPLOSION;
        }
        let mut w = World::new();
        w.names = names;
        w.statics = Arc::new(statics);
        w.cuts.trace = Some(Vec::new());
        w.cuts.probe_effect_trace = true;
        let source = object(&mut w, Vec::new(), Coord3D { x: 0.0, y: 0.0, z: 0.0 });
        let body = probe(b"Body", MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE | MODULEINTERFACE_DIE, ProbeAnswers::default(), 100.0);
        let target = object(&mut w, vec![body], Coord3D { x: 10.0, y: 0.0, z: 0.0 });
        crate::logic::weapon_fire::create_and_fire_temp_weapon(&mut w, Some(gun), Some(source), target).unwrap();
        let trace = w.cuts.trace.take().unwrap();
        assert!(
            trace.iter().any(|t| t.starts_with(&format!("damage {target} {:08x} {DAMAGE_EXPLOSION}", 25.0f32.to_bits()))),
            "the target's body took the shot: {trace:?}"
        );
    }
}
