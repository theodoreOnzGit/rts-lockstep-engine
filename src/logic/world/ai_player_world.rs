// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp (`m_ai`: its creation, update, new map and the calls a player
//     forwards to it) and GameLogic/AI/AIPlayer.cpp, AISkirmishPlayer.cpp (the world they ask, as the World answers it)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! Integration batch I9e: the computer players in the World, live with `cuts.live_ai` (without it a player's `m_ai` stays the
//! cut table `cuts.player_host.ai`, as before).
//!
//! **Where an AI player lives:** upstream a `Player` owns its `AIPlayer*`. The World keeps them by player index
//! ([`AiPlayers`]); `Player::ai` stays the record of whether the player has one and of which class. An AI player runs checked
//! out of the table ([`World::with_ai_player`]) with the World as its host ([`AiPlayerHost`]) and the AI data
//! (`TheAI->getAiData()`, `statics.ai_data`) beside it. One deleted while it runs (its player's `m_ai` gone) is not put back.
//!
//! **The answers:** a template is its id, a team and a team prototype their ids (0: none), an upgrade its name key. A command
//! to a unit's AI goes through its AI module (`ModuleCall`), as the groups' do.
//!
//! `p1w13-sonnet`, integration batch I9e, 2026-11-01.

use std::collections::BTreeMap;

use crate::common::base_type::{Coord3D, Region2D};
use crate::common::game_common::Relationship;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::KindOfType;
use crate::common::science::ScienceType;
use crate::logic::ai::ai_data::AiDataEntry;
use crate::logic::ai::ai_player::{AiPlayer, AiPlayerHost, ClosestQuery, PlayerAiCommand, PlayerRef, ProtoRef, TeamRef, TemplateRef, UpgradeRef};
use crate::logic::sides_list::BuildListInfo;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::PlayerIndex;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::team::TeamTemplateInfo;

/// The players' `m_ai`, by player index.
#[derive(Clone, Debug, Default)]
pub struct AiPlayers {
    pub by_player: BTreeMap<PlayerIndex, AiPlayer>,
}

impl World {
    /// The AI player of `p`, checked out of the table while `f` runs with the World and the AI data (`None`: no AI player).
    pub(crate) fn with_ai_player<R>(&mut self, p: PlayerIndex, f: impl FnOnce(&mut AiPlayer, &mut World, &AiDataEntry) -> R) -> Option<R> {
        let mut a = self.ai_players.by_player.remove(&p)?;
        let statics = self.statics.clone();
        let r = f(&mut a, self, &statics.ai_data);
        // (one deleted while it ran is not put back)
        if self.players.get_nth(p).is_some_and(|x| x.ai.is_some()) && !self.ai_players.by_player.contains_key(&p) {
            self.ai_players.by_player.insert(p, a);
        }
        Some(r)
    }

    /// The final override of the template a handle names.
    fn apl_tmpl(&self, t: TemplateRef) -> Option<&crate::common::thing_template::ThingTemplate> {
        u16::try_from(t).ok().and_then(|id| self.statics.thing_factory.find_by_template_id(id)).map(|o| &o.get_final_override().data)
    }

    /// A command of the AI player to a unit's AI (its `CMD_FROM_AI` commands, a dock from the player), or `joinTeam()`.
    fn apl_ai_command(&mut self, o: ObjectId, cmd: PlayerAiCommand) {
        use crate::logic::ai::ai_commands::{AICMD_DOCK, AICMD_REPAIR};
        use crate::logic::ai::ai_update_types::{CMD_FROM_AI, CMD_FROM_PLAYER};
        use crate::logic::world::ai_module_world::with_obj;
        let call = match cmd {
            PlayerAiCommand::Dock(d) => ModuleCall::AiDoCommandParms(with_obj(AICMD_DOCK, d, CMD_FROM_PLAYER)),
            PlayerAiCommand::Idle => ModuleCall::AiIdleFrom { command_source: CMD_FROM_AI },
            PlayerAiCommand::MoveToPosition(pos) => ModuleCall::AiMoveToPosition { pos, command_source: CMD_FROM_AI },
            PlayerAiCommand::ResumeConstruction(b) => ModuleCall::AiResumeConstruction { object: b, command_source: CMD_FROM_AI },
            PlayerAiCommand::Repair(b) => ModuleCall::AiDoCommandParms(with_obj(AICMD_REPAIR, b, CMD_FROM_AI)),
            PlayerAiCommand::FollowExitProductionPath(path) => ModuleCall::AiFollowExitProductionPath { path, ignore_object: INVALID_ID, command_source: CMD_FROM_AI },
            PlayerAiCommand::JoinTeam => ModuleCall::AiJoinTeam,
        };
        self.ai_ask(o, call);
    }

    /// `updateObjValuesFromMapProperties(&d)` with a dictionary of the build list entry's name, script, initial health and unsellable
    /// flag (`onStructureProduced`, `buildStructureNow`).
    fn apl_map_properties(&mut self, o: ObjectId, name: &[u8], script: &[u8], health: i32, unsellable: bool) {
        use crate::common::well_known_keys::{self as wk, WellKnownKeys};
        let keys = WellKnownKeys::new();
        let mut d = crate::common::dict::Dict::new();
        let r = (|| {
            d.set_ascii_string(keys.key(wk::OBJECT_NAME, &mut self.names), name)?;
            d.set_ascii_string(keys.key(wk::OBJECT_SCRIPT_ATTACHMENT, &mut self.names), script)?;
            d.set_int(keys.key(wk::OBJECT_INITIAL_HEALTH, &mut self.names), health)?;
            d.set_bool(keys.key(wk::OBJECT_UNSELLABLE, &mut self.names), unsellable)
        })();
        if let Err(e) = r {
            self.cuts.team_live_errors.push(format!("Dict::set: {e:?}"));
            return;
        }
        if let Err(e) = self.update_obj_values_from_map_properties(o, &d) {
            self.cuts.team_live_errors.push(format!("Object::updateObjValuesFromMapProperties: {e:?}"));
        }
    }

    /// `queueCreateUnit(thing, requestUniqueUnitID())` of the object's production.
    fn apl_queue_create_unit(&mut self, o: ObjectId, t: TemplateRef) -> bool {
        let Some(i) = self.get_production_update_interface(o) else { return false };
        let id = self.module_call(o, i, ModuleCall::ProductionRequestUniqueUnitId).as_int(0);
        self.module_call(o, i, ModuleCall::ProductionQueueCreateUnit { template: u16::try_from(t).ok(), production_id: id }).as_bool()
    }

    /// The AI player's three closest-object questions, with `AIPlayer.cpp`'s filters, from the bounding circles in 2-D:
    /// a supply source not the player's, on the map (`queueSupplyTruck`); a cash generator of the player's, on the map
    /// (`findSupplyCenter`); an enemy: not allied or neutral, alive, not stealthed undetected, not an insignificant building,
    /// not a harvester or a dozer (`isLocationSafe`).
    fn apl_closest(&mut self, q: ClosestQuery, p: PlayerRef, center: &Coord3D, radius: f32) -> Option<ObjectId> {
        use crate::common::kind_of::{KindOfMaskType, KINDOF_CASH_GENERATOR, KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_SUPPLY_SOURCE};
        use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED};
        use crate::logic::world::partition::filters::{allow_player, PartitionFilter as F};
        let kind = |k| KindOfMaskType::from_indices(&[k]);
        let filters = match q {
            ClosestQuery::SupplySourceNotMine => vec![
                F::AcceptByKindOf { must_be_set: kind(KINDOF_SUPPLY_SOURCE), must_be_clear: KindOfMaskType::new() },
                F::Player { player: Some(p), match_: false },
                F::OnMap,
            ],
            ClosestQuery::MyCashGenerator => vec![
                F::AcceptByKindOf { must_be_set: kind(KINDOF_CASH_GENERATOR), must_be_clear: KindOfMaskType::new() },
                F::Player { player: Some(p), match_: true },
                F::OnMap,
            ],
            ClosestQuery::Threat => {
                let mut stealthed = ObjectStatusMaskType::default();
                stealthed.set(OBJECT_STATUS_STEALTHED, 1);
                let mut seen = ObjectStatusMaskType::default();
                seen.set(OBJECT_STATUS_DETECTED, 1);
                seen.set(OBJECT_STATUS_DISGUISED, 1);
                vec![
                    F::PlayerAffiliation { player: p, affiliation: allow_player::ALLIES | allow_player::NEUTRAL, match_: false },
                    F::Alive,
                    F::RejectByObjectStatus { must_be_set: stealthed, must_be_clear: seen },
                    F::InsignificantBuildings { allow_non_buildings: true, allow_insignificant: false },
                    F::RejectByKindOf { must_be_set: kind(KINDOF_HARVESTER), must_be_clear: KindOfMaskType::new() },
                    F::RejectByKindOf { must_be_set: kind(KINDOF_DOZER), must_be_clear: KindOfMaskType::new() },
                ]
            }
        };
        let dc = crate::logic::world::partition::collide::DistanceCalculationType::FromBoundingSphere2D;
        self.with_partition(|pm, w| pm.get_closest_object_at(w, center, radius, dc, &filters).object)
    }

    // ---- the player's calls of its `m_ai`, live

    /// `newInstance(AIPlayer)(this)` or `AISkirmishPlayer`: with the script engine's global difficulty.
    pub(crate) fn ai_player_create(&mut self, p: PlayerIndex, skirmish: bool) {
        let difficulty = crate::logic::world::player_ops::PlayerHost::globals(self).global_difficulty;
        let statics = self.statics.clone();
        let a = if skirmish {
            AiPlayer::new_skirmish(self, p, difficulty, &statics.ai_data)
        } else {
            AiPlayer::new(self, p, false, difficulty, &statics.ai_data)
        };
        self.ai_players.by_player.insert(p, a);
    }

    /// `m_ai->deleteInstance()` (`~AIPlayer`: its queued teams deleted).
    pub(crate) fn ai_player_delete(&mut self, p: PlayerIndex) {
        if let Some(mut a) = self.ai_players.by_player.remove(&p) {
            a.clear_teams_in_queue(self);
        }
    }

    /// `m_ai->checkBridges(unit, way)`: the waypoints from `way` on in the terrain's list (`getNext()`).
    pub(crate) fn ai_player_check_bridges(&mut self, p: PlayerIndex, unit: Option<ObjectId>, way: Option<u32>) -> bool {
        let (Some(unit), Some(way)) = (unit, way) else { return false };
        let mut points = Vec::new();
        let mut w = self.terrain.waypoint_by_id(way);
        while let Some(h) = w {
            if let Some(x) = self.terrain.waypoint(h) {
                points.push(*x.location());
            }
            w = self.terrain.next_waypoint(h);
        }
        self.with_ai_player(p, |a, w, _| a.check_bridges(w, unit, &points)).unwrap_or(false)
    }

    /// `m_ai->computeSuperweaponTarget(power, pos, playerNdx, weaponRadius)`: the power by its template's type; an undefined
    /// answer (no position valued above -1) recorded and refused.
    pub(crate) fn ai_player_superweapon_target(&mut self, p: PlayerIndex, power: u32, pos: &mut Coord3D, player_ndx: i32, weapon_radius: f32) -> bool {
        let Some(ty) = self.statics.special_power_store.find_special_power_template_by_id(power).map(|t| t.special_power_type()) else { return false };
        match self.with_ai_player(p, |a, w, _| a.compute_superweapon_target(w, ty, player_ndx, weapon_radius)) {
            Some(Ok((target, found))) => {
                *pos = target;
                found
            }
            Some(Err(e)) => {
                self.cuts.team_live_errors.push(format!("AIPlayer::computeSuperweaponTarget: {e:?}"));
                false
            }
            None => false,
        }
    }
}

impl AiPlayerHost for World {
    fn frame(&mut self) -> u32 {
        crate::common::state_machine::StateContext::frame(self)
    }
    fn logic_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.random.game_logic_random_value(lo, hi)
    }
    fn debug_ai(&mut self) -> bool {
        self.statics.global_data.the_global_data().is_some_and(|g| g.debug_ai != 0)
    }
    fn base_value_per_supply_box(&mut self) -> i32 {
        self.statics.global_data.the_global_data().map_or(0, |g| g.base_value_per_supply_box)
    }
    fn append_debug_message(&mut self, msg: &[u8]) {
        let _ = msg; // (the script engine's debug display is the client's)
    }
    fn add_object_to_cache(&mut self, o: ObjectId) {
        let mut se = std::mem::take(&mut self.script_engine);
        se.add_object_to_cache(self, o);
        self.script_engine = se;
    }
    fn run_object_script(&mut self, script: &[u8], o: ObjectId) {
        let mut se = std::mem::take(&mut self.script_engine);
        let r = se.run_object_script(self, script, Some(o));
        self.script_engine = se;
        if let Err(e) = r {
            self.cuts.team_live_errors.push(format!("ScriptEngine::runObjectScript: {e:?}"));
        }
    }
    fn find_script(&mut self, name: &[u8]) -> (bool, bool) {
        let se = std::mem::take(&mut self.script_engine);
        let r = se.find_script(self, name).map(|r| crate::logic::world::script_engine::ScriptEngine::script(self, r).is_some_and(|s| !s.actions.is_empty()));
        self.script_engine = se;
        (r.is_some(), r.unwrap_or(false))
    }
    fn execute_action(&mut self, script: &[u8], team: TeamRef) {
        let mut se = std::mem::take(&mut self.script_engine);
        let r = match se.find_script(self, script) {
            Some(s) => se.friend_execute_action(self, s, true, i32::try_from(team).ok()),
            None => Ok(()),
        };
        self.script_engine = se;
        if let Err(e) = r {
            self.cuts.team_live_errors.push(format!("ScriptEngine::friend_executeAction: {e:?}"));
        }
    }
    fn clear_team_flags(&mut self) {
        self.script_engine.clear_team_flags();
    }
    fn skirmish_enemy_player(&mut self) -> PlayerRef {
        let se = std::mem::take(&mut self.script_engine);
        let r = se.get_skirmish_enemy_player(self);
        self.script_engine = se;
        r.unwrap_or(-1)
    }
    fn player_exists(&mut self, p: PlayerRef) -> bool {
        self.players.get_nth(p).is_some()
    }
    fn money(&mut self, p: PlayerRef) -> u32 {
        self.players.get_nth(p).map_or(0, |x| x.money.count_money())
    }
    fn can_build_units(&mut self, p: PlayerRef) -> bool {
        self.players.get_nth(p).is_some_and(|x| x.can_build_units)
    }
    fn set_can_build_units(&mut self, p: PlayerRef, b: bool) {
        if let Some(x) = self.players.get_nth_mut(p) {
            x.can_build_units = b;
        }
    }
    fn can_build_base(&mut self, p: PlayerRef) -> bool {
        self.players.get_nth(p).is_some_and(|x| x.can_build_base)
    }
    fn side(&mut self, p: PlayerRef) -> Vec<u8> {
        self.players.get_nth(p).map(|x| x.side.clone()).unwrap_or_default()
    }
    fn attacked_frame(&mut self, p: PlayerRef) -> u32 {
        self.players.get_nth(p).map_or(0, |x| x.attacked_frame)
    }
    fn science_purchase_points(&mut self, p: PlayerRef) -> i32 {
        self.players.get_nth(p).map_or(0, |x| x.science_purchase_points)
    }
    fn is_capable_of_purchasing_science(&mut self, p: PlayerRef, s: ScienceType) -> bool {
        crate::logic::world::player_ops::player_is_capable_of_purchasing_science(self, p, s).unwrap_or(false)
    }
    fn attempt_to_purchase_science(&mut self, p: PlayerRef, s: ScienceType) -> bool {
        crate::logic::world::player_ops::player_attempt_to_purchase_science(self, p, s).unwrap_or(false)
    }
    fn science_internal_name(&mut self, s: ScienceType) -> Vec<u8> {
        self.statics.science_store.get_internal_name_for_science(&self.names, s)
    }
    fn player_name(&mut self, p: PlayerRef) -> Vec<u8> {
        self.players.get_nth(p).map(|x| x.player_name.clone()).unwrap_or_default()
    }
    fn has_upgrade_in_production(&mut self, p: PlayerRef, u: UpgradeRef) -> bool {
        let Some(m) = self.statics.upgrade_center.find_upgrade_by_key(crate::common::name_key_generator::NameKeyType(u)).map(|t| t.upgrade_mask()) else { return false };
        self.players.get_nth(p).is_some_and(|x| x.upgrades_in_progress.test_for_all(&m))
    }
    fn has_upgrade_complete(&mut self, p: PlayerRef, u: UpgradeRef) -> bool {
        let Some(m) = self.statics.upgrade_center.find_upgrade_by_key(crate::common::name_key_generator::NameKeyType(u)).map(|t| t.upgrade_mask()) else { return false };
        self.players.get_nth(p).is_some_and(|x| x.upgrades_completed.test_for_all(&m))
    }
    fn default_team(&mut self, p: PlayerRef) -> TeamRef {
        self.players.get_nth(p).and_then(|x| x.default_team).unwrap_or(0)
    }
    fn relationship_to_team(&mut self, p: PlayerRef, team: TeamRef) -> Relationship {
        crate::logic::world::player_ops::player_get_relationship(self, p, (team != 0).then_some(team)).unwrap_or(Relationship::Neutral)
    }
    fn player_teams(&mut self, p: PlayerRef) -> Vec<ProtoRef> {
        self.players.get_nth(p).map(|x| x.team_prototypes.clone()).unwrap_or_default()
    }
    fn build_list(&mut self, p: PlayerRef) -> &mut Vec<BuildListInfo> {
        &mut self.players.get_nth_mut(p).expect("the AI's player").build_list
    }
    fn add_to_build_list(&mut self, p: PlayerRef, o: ObjectId) {
        if let Err(e) = crate::logic::world::player_ops::player_add_to_build_list(self, p, o) {
            self.cuts.team_live_errors.push(format!("Player::addToBuildList: {e:?}"));
        }
    }
    fn add_to_priority_build_list(&mut self, p: PlayerRef, name: &[u8], loc: &Coord3D, angle: f32) {
        if let Err(e) = crate::logic::world::player_ops::player_add_to_priority_build_list(self, p, name, *loc, angle) {
            self.cuts.team_live_errors.push(format!("Player::addToPriorityBuildList: {e:?}"));
        }
    }
    fn find_template(&mut self, name: &[u8]) -> Option<TemplateRef> {
        self.statics.thing_factory.find_template(name).map(|t| u32::from(t.data.get_template_id()))
    }
    fn first_template(&mut self) -> Option<TemplateRef> {
        self.statics.thing_factory.templates().next().map(|t| u32::from(t.data.get_template_id()))
    }
    fn next_template(&mut self, t: TemplateRef) -> Option<TemplateRef> {
        let mut it = self.statics.thing_factory.templates().map(|x| u32::from(x.data.get_template_id()));
        it.position(|x| x == t)?;
        it.next()
    }
    fn template_name(&mut self, t: TemplateRef) -> Vec<u8> {
        self.apl_tmpl(t).map(|x| x.get_name().to_vec()).unwrap_or_default()
    }
    fn template_is_kind_of(&mut self, t: TemplateRef, k: KindOfType) -> bool {
        self.apl_tmpl(t).is_some_and(|x| x.is_kind_of(k))
    }
    fn template_cost(&mut self, t: TemplateRef, p: PlayerRef) -> i32 {
        crate::logic::world::build_assistant::BuildAssistantHost::tmpl_calc_cost_to_build(self, t, p)
    }
    fn template_equivalent(&mut self, a: TemplateRef, b: TemplateRef) -> bool {
        crate::logic::world::build_assistant::BuildAssistantHost::tmpl_is_equivalent_to(self, a, b)
    }
    fn template_bounding_radius(&mut self, t: TemplateRef) -> f32 {
        self.apl_tmpl(t).map_or(0.0, |x| x.get_template_geometry_info().get_bounding_circle_radius())
    }
    fn template_placement_angle(&mut self, t: TemplateRef) -> f32 {
        self.apl_tmpl(t).map_or(0.0, |x| x.get_placement_view_angle())
    }
    fn objects(&mut self) -> Vec<ObjectId> {
        self.objects.ids_in_order()
    }
    fn object_exists(&mut self, o: ObjectId) -> bool {
        self.find_object(o).is_some()
    }
    fn object_template(&mut self, o: ObjectId) -> TemplateRef {
        self.find_object(o).map_or(0, |x| u32::from(x.template().get_template_id()))
    }
    fn object_is_kind_of(&mut self, o: ObjectId, k: KindOfType) -> bool {
        self.find_object(o).is_some_and(|x| x.is_kind_of(k))
    }
    fn position(&mut self, o: ObjectId) -> Coord3D {
        self.find_object(o).map(|x| x.get_position()).unwrap_or_default()
    }
    fn set_position(&mut self, o: ObjectId, pos: &Coord3D) {
        crate::logic::world::build_assistant::BuildAssistantHost::obj_set_position(self, o, *pos);
    }
    fn bounding_radius(&mut self, o: ObjectId) -> f32 {
        self.find_object(o).map_or(0.0, |x| x.geometry_info.get_bounding_circle_radius())
    }
    fn controlling_player(&mut self, o: ObjectId) -> Option<PlayerRef> {
        self.get_controlling_player(o)
    }
    fn object_team(&mut self, o: ObjectId) -> TeamRef {
        self.find_object(o).map_or(0, |x| x.team)
    }
    fn set_team(&mut self, o: ObjectId, team: TeamRef) {
        World::set_team(self, o, (team != 0).then_some(team));
    }
    fn status_test(&mut self, o: ObjectId, s: crate::common::object_status_types::ObjectStatusTypes) -> bool {
        self.find_object(o).is_some_and(|x| x.test_status(s))
    }
    fn clear_construction_status(&mut self, o: ObjectId) {
        use crate::common::object_status_types::{OBJECT_STATUS_RECONSTRUCTING, OBJECT_STATUS_UNDER_CONSTRUCTION};
        self.clear_status(o, crate::logic::world::object_state::mask_of(OBJECT_STATUS_UNDER_CONSTRUCTION));
        self.clear_status(o, crate::logic::world::object_state::mask_of(OBJECT_STATUS_RECONSTRUCTING));
    }
    fn builder_id(&mut self, o: ObjectId) -> ObjectId {
        self.find_object(o).map_or(INVALID_ID, |x| x.builder_id)
    }
    fn is_significantly_above_terrain(&mut self, o: ObjectId) -> bool {
        World::is_significantly_above_terrain(self, o)
    }
    fn update_obj_values_from_map_properties(&mut self, o: ObjectId, name: &[u8], script: &[u8], health: i32, unsellable: bool) {
        self.apl_map_properties(o, name, script, health, unsellable);
    }
    fn update_upgrade_modules(&mut self, o: ObjectId) {
        World::update_upgrade_modules(self, o);
    }
    fn has_ai(&mut self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.ai.is_some())
    }
    fn ai_is_idle(&mut self, o: ObjectId) -> bool {
        self.ai_ask(o, ModuleCall::AiIsIdle).is_some_and(|r| r.as_bool())
    }
    fn ai_goal_position(&mut self, o: ObjectId) -> Coord3D {
        self.ai_ask(o, ModuleCall::AiGetGoalPosition).map(|r| r.as_coord()).unwrap_or_default()
    }
    fn ai_command(&mut self, o: ObjectId, cmd: PlayerAiCommand) {
        self.apl_ai_command(o, cmd);
    }
    fn has_supply_truck_ai(&mut self, o: ObjectId) -> bool {
        self.ai_peek(o).is_some_and(|a| matches!(a.subclass, crate::logic::ai::ai_update::AiSubclass::SupplyTruck(_) | crate::logic::ai::ai_update::AiSubclass::Chinook(_) | crate::logic::ai::ai_update::AiSubclass::Worker(_)))
    }
    fn preferred_dock(&mut self, o: ObjectId) -> ObjectId {
        self.ai_ask(o, ModuleCall::GetSupplyTruckPreferredDockId).map_or(INVALID_ID, |r| r.as_id())
    }
    fn ferrying_supplies(&mut self, o: ObjectId) -> bool {
        let Some(index) = self.find_object(o).and_then(|x| x.ai) else { return false };
        match self.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: o, index }) {
            Some(crate::logic::world::update_body::UpdateBody::AiUpdate(m)) => m.module.as_ref().and_then(|m| m.supply_machine.as_ref()).is_some_and(|sm| {
                use crate::logic::ai::supply_truck_ai_update::{ST_DOCKING, ST_WANTING};
                matches!(sm.get_current_state_id(), ST_WANTING | ST_DOCKING)
            }),
            _ => false,
        }
    }
    fn forced_into_wanting_state(&mut self, o: ObjectId) -> bool {
        use crate::logic::ai::ai_update::AiSubclass;
        match self.ai_peek(o).map(|a| &a.subclass) {
            Some(AiSubclass::SupplyTruck(s)) => s.force_pending,
            Some(AiSubclass::Chinook(c)) => c.supply.force_pending,
            Some(AiSubclass::Worker(w)) => w.supply.force_pending,
            _ => false,
        }
    }
    fn set_force_wanting_state(&mut self, o: ObjectId, b: bool) {
        self.ai_ask(o, ModuleCall::SupplyTruckSetForceWantingState(b));
    }
    fn has_dozer_ai(&mut self, o: ObjectId) -> bool {
        self.ai_peek(o).is_some_and(|a| matches!(a.subclass, crate::logic::ai::ai_update::AiSubclass::Dozer(_) | crate::logic::ai::ai_update::AiSubclass::Worker(_)))
    }
    fn dozer_any_task_pending(&mut self, o: ObjectId) -> bool {
        self.with_ai_lent(o, crate::logic::ai::dozer_ai_update::is_any_task_pending).unwrap_or(false)
    }
    fn dozer_build_task_pending(&mut self, o: ObjectId) -> bool {
        self.with_ai_lent(o, |w| crate::logic::ai::dozer_ai_update::is_task_pending(w, crate::logic::ai::dozer_ai_update::DOZER_TASK_BUILD)).unwrap_or(false)
    }
    fn has_production(&mut self, o: ObjectId) -> bool {
        self.get_production_update_interface(o).is_some()
    }
    fn production_count(&mut self, o: ObjectId) -> i32 {
        match self.get_production_update_interface(o) {
            Some(i) => self.module_call(o, i, ModuleCall::ProductionGetCount).as_int(0),
            None => 0,
        }
    }
    fn queue_create_unit(&mut self, o: ObjectId, t: TemplateRef) -> bool {
        self.apl_queue_create_unit(o, t)
    }
    fn queue_upgrade(&mut self, o: ObjectId, u: UpgradeRef) -> bool {
        match self.get_production_update_interface(o) {
            Some(i) => self.module_call(o, i, ModuleCall::ProductionQueueUpgrade { upgrade: Some(u) }).as_bool(),
            None => false,
        }
    }
    fn damage_state(&mut self, o: ObjectId) -> Option<i32> {
        let b = self.get_body_module(o)?;
        Some(self.module_call(o, b, ModuleCall::GetDamageState).as_int(0))
    }
    fn last_damage(&mut self, o: ObjectId) -> Option<(bool, u32)> {
        let no_effect = self.peek_last_damage(o)?.out.no_effect;
        let b = self.get_body_module(o)?;
        Some((no_effect, self.module_call(o, b, ModuleCall::GetLastDamageTimestamp).as_int(0) as u32))
    }
    fn has_supply_center_dock(&mut self, o: ObjectId) -> bool {
        let n = self.find_object(o).map_or(0, |x| x.modules.len());
        (0..n).any(|i| matches!(self.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: o, index: i as crate::logic::world::object::ModuleIndex }), Some(crate::logic::world::update_body::UpdateBody::SupplyCenterDockUpdate(_))))
    }
    fn warehouse_boxes(&mut self, o: ObjectId) -> Option<i32> {
        let n = self.find_object(o)?.modules.len();
        (0..n).find(|&i| matches!(self.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: o, index: i as crate::logic::world::object::ModuleIndex }), Some(crate::logic::world::update_body::UpdateBody::SupplyWarehouseDockUpdate(_))))
        .map(|i| self.module_call(o, i as crate::logic::world::object::ModuleIndex, ModuleCall::SupplyWarehouseGetBoxesStored).as_int(0))
    }
    fn rebuild_hole(&mut self, o: ObjectId) -> Option<(ObjectId, ObjectId)> {
        let n = self.find_object(o)?.modules.len();
        (0..n).find_map(|i| match self.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: o, index: i as crate::logic::world::object::ModuleIndex }) {
            Some(crate::logic::world::update_body::UpdateBody::RebuildHoleBehavior(b)) => Some((b.get_spawner_id(), b.get_reconstructed_building_id())),
            _ => None,
        })
    }
    fn exit_natural_rally_point(&mut self, o: ObjectId) -> Option<Option<Coord3D>> {
        let e = self.get_object_exit_interface(o)?;
        Some(match self.module_call(o, e, ModuleCall::GetNaturalRallyPointWithOffset(true)) {
            crate::logic::world::module_call::ModuleReply::Coord(c) => Some(c),
            _ => None,
        })
    }
    fn command_set_upgrades(&mut self, o: ObjectId) -> Option<Vec<Vec<u8>>> {
        let set = crate::logic::world::build_assistant::BuildAssistantHost::find_command_set(self, o)? as usize;
        let mut out = Vec::new();
        for i in 0..crate::logic::world::build_assistant::MAX_COMMANDS_PER_SET as usize {
            let Some(b) = self.command_set_get_command_button(set, i) else { continue };
            let Some(k) = self.cuts.command_buttons.get(b).and_then(|b| b.upgrade) else { continue };
            if let Some(u) = self.statics.upgrade_center.find_upgrade_by_key(crate::common::name_key_generator::NameKeyType(k)) {
                out.push(u.upgrade_name().to_vec());
            }
        }
        Some(out)
    }
    fn team_proto(&mut self, team: TeamRef) -> ProtoRef {
        self.teams.team(team).map_or(0, |t| t.proto)
    }
    fn team_name(&mut self, team: TeamRef) -> Vec<u8> {
        self.tq().team_name(team)
    }
    fn team_members(&mut self, team: TeamRef) -> Vec<ObjectId> {
        self.teams.members(team)
    }
    fn team_has_any_units(&mut self, team: TeamRef) -> bool {
        crate::logic::world::team_ops::team_has_any_units(self, team)
    }
    fn team_has_any_objects(&mut self, team: TeamRef) -> bool {
        crate::logic::world::team_ops::team_has_any_objects(self, team)
    }
    fn team_is_idle(&mut self, team: TeamRef) -> bool {
        crate::logic::world::team_ops::team_is_idle(self, team)
    }
    fn team_set_active(&mut self, team: TeamRef) {
        if let Err(e) = crate::logic::world::team_ops::team_set_active(self, team) {
            self.cuts.team_live_errors.push(format!("Team::setActive: {e:?}"));
        }
    }
    fn team_transfer_units_to(&mut self, team: TeamRef, to: TeamRef) {
        if let Err(e) = crate::logic::world::team_ops::team_transfer_units_to(self, team, (to != 0).then_some(to)) {
            self.cuts.team_live_errors.push(format!("Team::transferUnitsTo: {e:?}"));
        }
    }
    fn team_delete(&mut self, team: TeamRef) {
        if let Err(e) = crate::logic::world::team_ops::team_delete_team(self, team, false) {
            self.cuts.team_live_errors.push(format!("Team::deleteTeam: {e:?}"));
        }
    }
    fn team_try_to_recruit(&mut self, team: TeamRef, t: TemplateRef, origin: &Coord3D, radius: f32) -> Option<ObjectId> {
        crate::logic::world::team_ops::team_try_to_recruit(self, team, t, *origin, radius).ok().flatten()
    }
    fn team_count_objects_by_template(&mut self, team: TeamRef, t: TemplateRef) -> i32 {
        let mut count = [0];
        crate::logic::world::team_ops::team_count_objects_by_thing_template(self, team, &[Some(t)], false, &mut count, true);
        count[0]
    }
    fn team_estimate_position(&mut self, team: TeamRef) -> Option<Coord3D> {
        crate::logic::world::team_ops::team_get_estimate_team_position(self, team)
    }
    fn team_guard_position(&mut self, team: TeamRef, pos: &Coord3D) {
        let g = crate::logic::world::game_logic_update::GameLogicHost::u_create_group(self);
        crate::logic::world::team_ops::team_get_team_as_ai_group(self, team, Some(g));
        self.with_group(g, |gr, w| gr.group_guard(w, Some(crate::logic::ai::ai_group::GroupAiCommand::GuardPosition { pos: *pos, mode: crate::logic::ai::ai_update_types::GUARDMODE_NORMAL }), crate::logic::ai::ai_update_types::CMD_FROM_SCRIPT));
    }
    fn proto_name(&mut self, proto: ProtoRef) -> Vec<u8> {
        self.teams.prototype(proto).map(|x| x.name.clone()).unwrap_or_default()
    }
    fn proto_info(&mut self, proto: ProtoRef) -> TeamTemplateInfo {
        self.teams.prototype(proto).map(|x| x.team_template.clone()).expect("a team prototype the AI holds")
    }
    fn proto_is_singleton(&mut self, proto: ProtoRef) -> bool {
        self.teams.prototype(proto).is_some_and(|x| x.get_is_singleton())
    }
    fn proto_controlling_player(&mut self, proto: ProtoRef) -> PlayerRef {
        self.teams.prototype(proto).and_then(|x| x.owning_player).unwrap_or(-1)
    }
    fn proto_instances(&mut self, proto: ProtoRef) -> Vec<TeamRef> {
        self.teams.instances(proto)
    }
    fn proto_evaluate_production_condition(&mut self, proto: ProtoRef) -> bool {
        crate::logic::world::team_ops::proto_evaluate_production_condition(self, proto).unwrap_or(false)
    }
    fn find_team(&mut self, name: &[u8]) -> Option<TeamRef> {
        crate::logic::world::team_ops::factory_find_team(self, name).ok().flatten()
    }
    fn create_inactive_team(&mut self, name: &[u8]) -> TeamRef {
        match crate::logic::world::team_ops::factory_create_inactive_team(self, name) {
            Ok(t) => t,
            Err(e) => {
                self.cuts.team_live_errors.push(format!("TeamFactory::createInactiveTeam: {e:?}"));
                0
            }
        }
    }
    fn team_by_id(&mut self, id: u32) -> TeamRef {
        crate::logic::world::team_ops::factory_find_team_by_id(self, id).unwrap_or(0)
    }
    fn build_object_now(&mut self, builder: Option<ObjectId>, t: TemplateRef, pos: &Coord3D, angle: f32, p: PlayerRef) -> Option<ObjectId> {
        match crate::logic::world::build_assistant::build_assistant_build_object_now(self, builder, t, *pos, angle, p) {
            Ok(o) => o,
            Err(e) => {
                self.cuts.team_live_errors.push(format!("BuildAssistant::buildObjectNow: {e:?}"));
                None
            }
        }
    }
    fn is_location_legal_to_build(&mut self, pos: &Coord3D, t: TemplateRef, angle: f32, options: u32, builder: Option<ObjectId>, p: PlayerRef) -> bool {
        crate::logic::world::build_assistant::build_assistant_is_location_legal_to_build(self, *pos, t, angle, options, builder, Some(p)) == crate::logic::world::build_assistant::LegalBuildCode::Ok
    }
    fn is_possible_to_make_unit(&mut self, factory: ObjectId, t: TemplateRef) -> bool {
        crate::logic::world::build_assistant::build_assistant_is_possible_to_make_unit(self, factory, t).unwrap_or(false)
    }
    fn remove_all_bibs(&mut self) {
        // (the terrain visual's bibs are the client's)
    }
    fn closest_object(&mut self, q: ClosestQuery, p: PlayerRef, center: &Coord3D, radius: f32) -> Option<ObjectId> {
        self.apl_closest(q, p, center, radius)
    }
    fn quick_does_path_exist(&mut self, o: ObjectId, to: &Coord3D) -> bool {
        crate::logic::world::partition::host::PartitionHost::quick_does_path_exist(self, o, to).unwrap_or(false)
    }
    fn adjust_to_possible_destination(&mut self, o: ObjectId, pos: &mut Coord3D) {
        self.pathfinder_adjust_to_possible_destination(o, pos);
    }
    fn ground_height(&mut self, x: f32, y: f32) -> f32 {
        self.terrain.ground_height(x, y, false).0
    }
    fn extent(&mut self) -> Region2D {
        let r = self.terrain.extent().unwrap_or_default();
        Region2D { lo: crate::common::base_type::Coord2D { x: r.lo.x, y: r.lo.y }, hi: crate::common::base_type::Coord2D { x: r.hi.x, y: r.hi.y } }
    }
    fn find_upgrade(&mut self, name: &[u8]) -> Option<UpgradeRef> {
        let statics = self.statics.clone();
        statics.upgrade_center.find_upgrade(&mut self.names, name).ok().flatten().map(|t| t.upgrade_name_key().0)
    }
    fn upgrade_is_object_type(&mut self, u: UpgradeRef) -> bool {
        self.statics.upgrade_center.find_upgrade_by_key(crate::common::name_key_generator::NameKeyType(u)).is_some_and(|t| t.upgrade_type() == crate::common::upgrade::UpgradeType::Object)
    }
    fn upgrade_name(&mut self, u: UpgradeRef) -> Vec<u8> {
        self.statics.upgrade_center.find_upgrade_by_key(crate::common::name_key_generator::NameKeyType(u)).map(|t| t.upgrade_name().to_vec()).unwrap_or_default()
    }
    fn can_afford_upgrade(&mut self, p: PlayerRef, u: UpgradeRef) -> bool {
        // (`UpgradeCenter::canAffordUpgrade`: `countMoney() < calcCostToBuild(player)`, the `Int` cost compared as unsigned)
        let Some(cost) = self.statics.upgrade_center.find_upgrade_by_key(crate::common::name_key_generator::NameKeyType(u)).map(|t| t.cost_to_build()) else { return false };
        self.players.get_nth(p).is_some_and(|x| x.money.count_money() >= cost as u32)
    }
    fn has_sufficient_power(&mut self, p: PlayerRef) -> bool {
        let now = crate::common::state_machine::StateContext::frame(self);
        self.players.get_nth(p).is_some_and(|x| x.energy.has_sufficient_power(now))
    }
    fn is_disabled_unmanned(&mut self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.is_disabled_by_type(crate::common::disabled_types::DISABLED_UNMANNED))
    }
    fn clear_builder(&mut self, o: ObjectId) {
        World::set_builder(self, o, INVALID_ID);
    }
    fn can_make_unit(&mut self, builder: ObjectId, t: Option<TemplateRef>) -> bool {
        match t {
            Some(t) => matches!(crate::logic::world::build_assistant::build_assistant_can_make_unit(self, builder, t), Ok(crate::logic::world::build_assistant::CanMakeType::Ok)),
            None => false,
        }
    }
    fn player_count(&mut self) -> i32 {
        self.players.player_count()
    }
    fn player_is_human(&mut self, p: PlayerRef) -> bool {
        self.players.get_nth(p).is_some_and(|x| x.player_type == crate::common::game_common::PlayerType::Human)
    }
    fn player_has_any_units(&mut self, p: PlayerRef) -> bool {
        crate::logic::world::player_ops::player_has_any_units(self, p).unwrap_or(false)
    }
    fn player_has_any_build_facility(&mut self, p: PlayerRef) -> bool {
        crate::logic::world::player_ops::player_has_any_build_facility(self, p).unwrap_or(false)
    }
    fn player_has_any_objects(&mut self, p: PlayerRef) -> bool {
        crate::logic::world::player_ops::player_has_any_objects(self, p).unwrap_or(false)
    }
    fn player_is_skirmish_ai(&mut self, p: PlayerRef) -> bool {
        crate::logic::world::player_ops::player_is_skirmish_ai_player(self, p).unwrap_or(false)
    }
    fn player_current_enemy(&mut self, p: PlayerRef) -> Option<PlayerRef> {
        crate::logic::world::player_ops::player_get_current_enemy(self, p).ok().flatten()
    }
    fn mp_start_index(&mut self, p: PlayerRef) -> i32 {
        self.players.get_nth(p).map_or(0, |x| x.mp_start_index)
    }
    fn closest_waypoint_on_path(&mut self, pos: &Coord3D, label: &[u8]) -> Option<Coord3D> {
        let label = crate::common::ascii_string::AsciiString::try_from_bytes(label).ok()?;
        let h = self.terrain.closest_waypoint_on_path(pos, &label)?;
        self.terrain.waypoint(h).map(|w| *w.location())
    }
    fn maximum_pathfind_extent(&mut self) -> Region2D {
        let r = self.terrain.maximum_pathfind_extent().unwrap_or_default();
        Region2D { lo: crate::common::base_type::Coord2D { x: r.lo.x, y: r.lo.y }, hi: crate::common::base_type::Coord2D { x: r.hi.x, y: r.hi.y } }
    }
    fn find_broken_bridge(&mut self, unit: ObjectId, to: &Coord3D) -> Option<ObjectId> {
        let r = self.with_ai_self(unit, |w| w.ai_pf(|pf, w, o, loco| pf.find_broken_bridge(w, loco.valid_surfaces, &o.pos, to)))?;
        r.0.then_some(r.1)
    }
    fn undo_and_destroy_structure(&mut self, o: ObjectId) {
        if let Some(p) = self.get_controlling_player(o) {
            crate::logic::world::player_ops::player_on_structure_undone(self, p, Some(o));
        }
        self.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::RemoveObject(o));
        self.destroy_object(o);
    }
}
