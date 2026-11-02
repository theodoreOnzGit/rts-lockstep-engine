// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp and the AI update modules (the calls the AI's states make
//   out of themselves, answered by the world)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I9b-2: the world as `AiStateHost`, the host of the AI's state machines, answered for
//! [`World::ai_self`] (batch I9b-1).
//!
//! The AI's state and machines are lent to the host while they run: the world keeps the stacks they are lent on
//! ([`AiStacks`], `World::ai_stacks`). The host's methods were written from the trait's own signatures; those the world
//! answers have their bodies here: the objects (kinds, status, health, containment, geometry, position, orientation,
//! layer, team, relationship, producer and builder, model conditions, death), the players (money, objects, the local
//! player), the teams (members, name, the common target), the waypoints and triggers, the terrain, the pathfinder (batch
//! I9a), the AI data and the logic's random values. **The rest are cut points**: each `&mut self` one is logged as
//! [`CutCall::Host`] with its name and answers its type's default; a `&self` one answers the default without a log. They
//! are the client (drawables, audio, the in-game UI, particles, FX), the other modules an AI talks to (weapons and turrets,
//! contain, dock, exit, parking place, production, physics beyond I9b-1, special powers, the tunnel system), the AI
//! groups and `TheAI` (batch I9c), and the AI update module's own snapshot (batch I9b-3).
//!
//! `p1w13-sonnet`, integration batch I9b-2, 2026-11-01.

#![allow(clippy::needless_return)]

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::Coord3D;
use crate::logic::world::partition::{FindPositionOptions, RANDOM_START_ANGLE};
use crate::common::disabled_types::DisabledType;
use crate::common::game_common::Relationship;
use crate::common::game_type::{ObjectId, PathfindLayer, WeaponSlotType, INVALID_ID};
use crate::common::kind_of::KindOfType;
use crate::common::model_state::ModelConditionFlagType;
use crate::common::object_status_types::ObjectStatusTypes;
use crate::common::state_machine::{StateExitType, StateId, StateMachine, StateReturn};
use crate::common::xfer::Snapshot;
use crate::common::xfer::{Xfer, XferResult};
use crate::logic::ai::ai_attack_states::AttackFrame;
use crate::logic::ai::ai_states::{AiState, AiStateHost, DistanceKind, PassengerCommand};
use crate::logic::ai::ai_update::AiUpdate;
use crate::logic::ai::ai_update_types::{AiStateType, CanAttackResultValue, CommandSourceType};
use crate::logic::ai::path::Path;
use crate::logic::locomotor::{LocomotorHost, LocomotorSet, LocomotorSetError};
use crate::logic::module::update_module::UpdateSleepTime;
use crate::logic::weapon_set_state::WeaponChoiceCriteria;
use crate::logic::weapon_types::WeaponStatus;
use crate::logic::world::game_logic::World;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::module_call::{ModuleCall as MC, ModuleReply};
use crate::logic::ai::ai_face_exit_states::DOOR_NONE_AVAILABLE;

/// The stacks the AI's state and machines are lent on while they run (upstream's objects are reached by pointer; here
/// the module lends them to the world for the run and takes them back). Empty between runs: a copy of the world (`Clone`)
/// copies no lent state, and `Debug` shows the stacks' depths.
#[derive(Default)]
pub struct AiStacks {
    pub ai: Vec<AiUpdate>,
    pub machine: Vec<StateMachine<AiState<World>>>,
    pub supply: Vec<Option<StateMachine<crate::logic::ai::supply_truck_ai_update::SupplyState<World>>>>,
    pub worker: Vec<Option<StateMachine<crate::logic::ai::worker_ai_update::WorkerState<World>>>>,
    pub dozer: Vec<Option<StateMachine<crate::logic::ai::dozer_ai_update::DozerPrimaryState<World>>>>,
    pub deliver: Vec<Option<StateMachine<crate::logic::ai::deliver_payload_ai_update::DeliverState<World>>>>,
    pub attack: Vec<AttackFrame>,
    /// Batch I9b-6: the turrets' fields while their machines run (`TurretHost::turret_stack`).
    pub turret: Vec<crate::logic::ai::turret::TurretData>,
}

impl Clone for AiStacks {
    fn clone(&self) -> Self {
        AiStacks::default()
    }
}

impl std::fmt::Debug for AiStacks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AiStacks {{ ai: {}, machine: {}, attack: {} }}", self.ai.len(), self.machine.len(), self.attack.len())
    }
}

impl World {
    /// Batch I9h: `obj->getContain()` asked `call`; `None` when there is no such object or it has no contain.
    fn ai_contain_call(&mut self, obj: ObjectId, call: MC) -> Option<ModuleReply> {
        let c = self.get_contain(obj)?;
        Some(self.module_call(obj, c, call))
    }

    /// Batch I9h: the AI's own object's contain asked `call` (`getObject()->getContain()`).
    fn ai_own_contain_call(&mut self, call: MC) -> Option<ModuleReply> {
        self.ai_contain_call(self.ai_self, call)
    }

    /// Batch I9h: `obj->getContain()->getContainedItemsList()`, in order (empty: no contain).
    fn ai_contained_items(&mut self, obj: ObjectId) -> Vec<ObjectId> {
        match self.ai_contain_call(obj, MC::GetContainedItems) {
            Some(ModuleReply::Ids(ids)) => ids,
            _ => Vec::new(),
        }
    }

    /// Batch I9i: `obj->getDockUpdateInterface()` asked `call`; `None` when there is no such object or it has no dock.
    fn ai_dock_call(&mut self, obj: ObjectId, call: MC) -> Option<ModuleReply> {
        let d = self.get_dock_update_interface(obj)?;
        Some(self.module_call(obj, d, call))
    }

    /// Batch I9i: the AI's controlling player's `getResourceGatheringManager()->findBestSupplyCenter(owner)` (`center`) or
    /// `findBestSupplyWarehouse(owner)`; `INVALID_ID` for none, for no manager, and (recorded) for its undefined behaviour.
    fn ai_resource_query(&mut self, center: bool) -> ObjectId {
        let us = self.ai_self;
        let Some(p) = self.get_controlling_player(us) else { return INVALID_ID };
        let Some(mut rgm) = self.players.get_nth_mut(p).and_then(|pl| pl.resource_gathering_manager.take()) else { return INVALID_ID };
        let r = if center { rgm.find_best_supply_center(self, Some(us)) } else { rgm.find_best_supply_warehouse(self, Some(us)) };
        if let Some(pl) = self.players.get_nth_mut(p) {
            pl.resource_gathering_manager = Some(rgm);
        }
        match r {
            Ok(found) => found.unwrap_or(INVALID_ID),
            Err(e) => {
                self.cuts.team_live_errors.push(format!("ResourceGatheringManager::findBestSupply{}: {e:?}", if center { "Center" } else { "Warehouse" }));
                INVALID_ID
            }
        }
    }

    /// Logs one of the AI states' calls into a unit the world does not hold.
    fn ais_cut(&mut self, call: &'static str) {
        let id = self.ai_self;
        self.cut(CutCall::Host { call, args: vec![i64::from(id)], text: Vec::new() });
    }

    /// Integration batch I9j: `ThePartitionManager->findPositionAround(pos, &options, &result)` for the AI's sites, the partition
    /// manager's with `cuts.live_partition` (none of them sets `flags`: `FPF_NONE`), otherwise the cut `call` answering none.
    fn ai_find_position_around(&mut self, call: &'static str, pos: &Coord3D, options: FindPositionOptions) -> Option<Coord3D> {
        if !self.cuts.live_partition {
            self.ais_cut(call);
            return None;
        }
        self.with_partition(|pm, w| pm.find_position_around(w, pos, &options))
    }
}

impl AiStateHost for World {
    fn ai_stack(&mut self) -> &mut Vec<AiUpdate> {
        &mut self.ai_stacks.ai
    }
    fn take_own_calls(&mut self) -> Vec<crate::logic::ai::ai_states::OwnCall> {
        let us = self.ai_self;
        let (mine, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.ai_own_calls).into_iter().partition(|(id, _)| *id == us);
        self.ai_own_calls = others;
        mine.into_iter().map(|(_, c)| c).collect()
    }
    fn own_call_failed(&mut self, e: crate::common::state_machine::StateMachineError) {
        self.cuts.team_live_errors.push(format!("AIUpdateInterface (a call made while it ran): {e:?}"));
    }
    fn contain_passenger_allowed_to_fire(&mut self) -> bool {
        self.ai_own_contain_call(MC::IsPassengerAllowedToFire(INVALID_ID)).is_some_and(|r| r.as_bool())
    }
    fn object_has_ai(&self, id: ObjectId) -> bool {
        self.find_object(id).is_some_and(|o| o.ai.is_some())
    }
    fn passenger_ai_command(&mut self, passenger: ObjectId, cmd: PassengerCommand) {
        let call = match cmd {
            PassengerCommand::AttackObject { victim, max_shots, src } => MC::AiAttackObject { victim, max_shots, command_source: src },
            PassengerCommand::ForceAttackObject { victim, max_shots, src } => MC::AiForceAttackObject { victim, max_shots, command_source: src },
            PassengerCommand::AttackPosition { pos, max_shots, src } => MC::AiAttackPosition { pos: Some(pos), max_shots, command_source: src },
        };
        self.ai_ask(passenger, call);
    }
    fn clear_model_condition_flags(&mut self, clear: &[ModelConditionFlagType]) {
        let id = self.ai_self;
        for c in clear {
            World::clear_model_condition_state(self, id, *c);
        }
    }
    fn play_per_unit_sound(&mut self, _name: &[u8]) {
        self.ais_cut("play_per_unit_sound");
    }
    fn set_drawable_animation_frame(&mut self, _frame: i32) {
        self.ais_cut("set_drawable_animation_frame");
    }
    fn object_health(&mut self, id: ObjectId) -> Option<(f32, f32)> {
        World::body_health(self, id)
    }
    fn object_is_contained(&mut self, id: ObjectId) -> bool {
        self.is_contained(id)
    }
    fn member_ai_last_command_source(&mut self, _id: ObjectId) -> CommandSourceType {
        self.ais_cut("member_ai_last_command_source");
        Default::default()
    }
    fn member_ai_set_allowed_to_chase(&mut self, _id: ObjectId, _allowed: bool) {
        self.ais_cut("member_ai_set_allowed_to_chase");
    }
    fn member_ai_state_type(&mut self, _id: ObjectId) -> AiStateType {
        self.ais_cut("member_ai_state_type");
        Default::default()
    }
    fn member_ai_is_moving(&mut self, _id: ObjectId) -> bool {
        self.ais_cut("member_ai_is_moving");
        Default::default()
    }
    fn member_ai_goal_object(&mut self, _id: ObjectId) -> ObjectId {
        self.ais_cut("member_ai_goal_object");
        INVALID_ID
    }
    fn member_ai_command(&mut self, _id: ObjectId, _cmd: crate::logic::ai::assault_transport_ai_update::MemberCommand) {
        self.ais_cut("member_ai_command");
    }
    fn waypoint_by_name(&mut self, name: &[u8]) -> Option<u32> {
        AsciiString::try_from_bytes(name).ok().and_then(|n| self.terrain.waypoint_by_name(&n)).and_then(|h| self.terrain.waypoint(h)).map(|w| w.id() as u32)
    }
    fn waypoint_id(&mut self, way: u32) -> u32 {
        way
    }
    fn railed_set_dock_open(&mut self, _open: bool) {
        self.ais_cut("railed_set_dock_open");
    }
    fn railed_dock_present(&mut self) -> bool {
        self.ais_cut("railed_dock_present");
        Default::default()
    }
    fn railed_dock_is_loading_or_unloading(&mut self) -> bool {
        self.ais_cut("railed_dock_is_loading_or_unloading");
        Default::default()
    }
    fn railed_dock_unload_all(&mut self) {
        self.ais_cut("railed_dock_unload_all");
    }
    fn team_name(&mut self, team: u32) -> Vec<u8> {
        let teams = &self.teams;
        teams.team(team).and_then(|x| teams.prototype(x.proto)).map(|p| p.name.clone()).unwrap_or_default()
    }
    fn team_by_name(&mut self, _name: &[u8]) -> Option<u32> {
        self.ais_cut("team_by_name");
        Default::default()
    }
    fn command_button_name(&mut self, _button: u32) -> Vec<u8> {
        self.ais_cut("command_button_name");
        Default::default()
    }
    fn command_button_by_name(&mut self, _name: &[u8]) -> Option<u32> {
        self.ais_cut("command_button_by_name");
        Default::default()
    }
    fn set_drawable_animation_loop_duration(&mut self, _frames: u32) {
        self.ais_cut("set_drawable_animation_loop_duration");
    }
    fn player_has_money(&mut self) -> bool {
        self.get_controlling_player(self.ai_self).and_then(|p| self.players.get_nth(p)).is_some_and(|p| p.money.count_money() > 0)
    }
    fn veterancy_level(&mut self) -> Option<crate::common::game_common::VeterancyLevel> {
        self.find_object(self.ai_self).map(|_| self.get_veterancy_level(self.ai_self))
    }
    fn money_deposit(&mut self, amount: u32) {
        if let Some(p) = self.get_controlling_player(self.ai_self) {
            let _ = crate::logic::world::player_objects::player_money_deposit(self, p, amount, true);
        }
    }
    fn score_add_money_earned(&mut self, _amount: u32) {
        self.ais_cut("score_add_money_earned");
    }
    fn add_experience_points(&mut self, _xp: u32) {
        self.ais_cut("add_experience_points");
    }
    fn is_locally_controlled(&mut self, id: ObjectId) -> bool {
        self.get_controlling_player(id).is_some_and(|p| crate::logic::world::player_ops::player_is_local_player(self, p))
    }
    fn hack_cash_floating_text(&mut self, _amount: u32) {
        self.ais_cut("hack_cash_floating_text");
    }
    fn ai_machine_stack(&mut self) -> &mut Vec<StateMachine<AiState<Self>>> where Self: Sized {
        &mut self.ai_stacks.machine
    }
    fn supply_find_best_center(&mut self) -> ObjectId {
        self.ai_resource_query(true)
    }
    fn supply_find_best_warehouse(&mut self) -> ObjectId {
        self.ai_resource_query(false)
    }
    fn player_find_closest_by_kind_of(&mut self, _kind: crate::common::kind_of::KindOfType) -> ObjectId {
        self.ais_cut("player_find_closest_by_kind_of");
        INVALID_ID
    }
    fn distance_squared_to_object_2d(&mut self, other: ObjectId) -> f32 {
        let (p, q) = (LocomotorHost::position(self), self.find_object(other).map_or_else(Coord3D::default, |o| o.get_position()));
        (p.x - q.x) * (p.x - q.x) + (p.y - q.y) * (p.y - q.y)
    }
    fn dock_supply_kind(&mut self, dock: ObjectId) -> crate::logic::ai::supply_truck_ai_update::SupplyDockKind {
        use crate::logic::ai::supply_truck_ai_update::SupplyDockKind;
        if self.find_module(dock, b"SupplyWarehouseDockUpdate").is_some() {
            SupplyDockKind::Warehouse
        } else if self.find_module(dock, b"SupplyCenterDockUpdate").is_some() {
            SupplyDockKind::Center
        } else {
            SupplyDockKind::None
        }
    }
    fn drawable_update_supply_status(&mut self, _max_boxes: i32, _boxes: i32) {
        self.ais_cut("drawable_update_supply_status");
    }
    fn play_supplies_depleted_voice(&mut self) {
        self.ais_cut("play_supplies_depleted_voice");
    }
    fn object_weapon_bonus_condition(&mut self, id: ObjectId) -> u32 {
        self.find_object(id).map_or(0, |o| o.weapon_bonus_condition)
    }
    fn position_projectile_for_launch(&mut self, _launcher: ObjectId, _wslot: i32, _specific_barrel_to_use: i32) {
        self.ais_cut("position_projectile_for_launch");
    }
    fn weapon_template_speed(&mut self, _template: u32) -> f32 {
        self.ais_cut("weapon_template_speed");
        0.0
    }
    fn weapon_template_should_projectile_collide_with(&mut self, _template: u32, _launcher: ObjectId, _other: ObjectId, _victim: ObjectId) -> bool {
        self.ais_cut("weapon_template_should_projectile_collide_with");
        Default::default()
    }
    fn weapon_template_die_on_detonate(&mut self, _template: u32) -> bool {
        self.ais_cut("weapon_template_die_on_detonate");
        Default::default()
    }
    fn weapon_store_handle_projectile_detonation(&mut self, _template: u32, _bonus: u32, _inflict_damage: bool) {
        self.ais_cut("weapon_store_handle_projectile_detonation");
    }
    fn weapon_template_name(&mut self, _template: u32) -> Vec<u8> {
        self.ais_cut("weapon_template_name");
        Default::default()
    }
    fn weapon_template_by_name(&mut self, _name: &[u8]) -> Option<u32> {
        self.ais_cut("weapon_template_by_name");
        Default::default()
    }
    fn particle_system_create_attached(&mut self, _template: u32) -> u32 {
        self.ais_cut("particle_system_create_attached");
        Default::default()
    }
    fn particle_system_destroy(&mut self, _id: u32) {
        self.ais_cut("particle_system_destroy");
    }
    fn particle_template_name(&mut self, _template: u32) -> Vec<u8> {
        self.ais_cut("particle_template_name");
        Default::default()
    }
    fn particle_template_by_name(&mut self, _name: &[u8]) -> Option<u32> {
        self.ais_cut("particle_template_by_name");
        Default::default()
    }
    fn object_garrison_hit_killable(&mut self, _id: ObjectId) -> bool {
        self.ais_cut("object_garrison_hit_killable");
        Default::default()
    }
    fn object_is_kind_of_multi(&mut self, id: ObjectId, must_be_set: &crate::common::kind_of::KindOfMaskType, must_be_clear: &crate::common::kind_of::KindOfMaskType) -> bool {
        self.find_object(id).is_some_and(|o| o.template().is_kind_of_multi(must_be_set, must_be_clear))
    }
    fn object_score_the_kill(&mut self, _killer: ObjectId, _victim: ObjectId) {
        self.ais_cut("object_score_the_kill");
    }
    fn fx_do_obj(&mut self, fx: Option<u32>, primary: ObjectId) {
        // (batch I9b-5: the FX list by its name, `cuts.ai_host.fx_names`)
        let text = fx.and_then(|i| self.cuts.ai_host.fx_names.get(i as usize)).cloned().unwrap_or_default();
        self.cut(CutCall::Host { call: "fx_do_obj", args: vec![i64::from(primary)], text });
    }
    fn body_max_health(&mut self) -> f32 {
        let id = self.ai_self;
        World::body_health(self, id).map_or(0.0, |h| h.1)
    }
    fn attempt_damage(&mut self, damage_info: &mut crate::logic::damage::DamageInfo) {
        let id = self.ai_self;
        self.attempt_damage(id, damage_info);
    }
    fn set_drawable_hidden(&mut self, _hidden: bool) {
        self.ais_cut("set_drawable_hidden");
    }
    fn distance_squared_to_object(&mut self, other: ObjectId, kind: DistanceKind) -> f32 {
        self.ai_distance_squared(self.ai_self, other, kind)
    }
    fn distance_squared_to_position_2d(&mut self, pos: &Coord3D) -> f32 {
        let p = LocomotorHost::position(self);
        (p.x - pos.x) * (p.x - pos.x) + (p.y - pos.y) * (p.y - pos.y)
    }
    fn calculate_countermeasure_to_divert_to(&mut self, _victim: ObjectId) -> ObjectId {
        self.ais_cut("calculate_countermeasure_to_divert_to");
        INVALID_ID
    }
    fn contain_is_special_overlord_style_container(&mut self) -> bool {
        self.ai_own_contain_call(MC::IsSpecialOverlordStyleContainer).is_some_and(|r| r.as_bool())
    }
    fn contain_passenger_allowed_to_fire_id(&mut self, passenger: ObjectId) -> bool {
        self.ai_own_contain_call(MC::IsPassengerAllowedToFire(passenger)).is_some_and(|r| r.as_bool())
    }
    fn contain_rider(&mut self) -> ObjectId {
        self.ai_own_contain_call(MC::GetRider).map_or(INVALID_ID, |r| r.as_id())
    }
    fn parking_place_exists(&mut self, _airfield: ObjectId) -> bool {
        self.ais_cut("parking_place_exists");
        Default::default()
    }
    fn parking_place_set_healee(&mut self, _airfield: ObjectId, _add: bool) {
        self.ais_cut("parking_place_set_healee");
    }
    fn member_ai_current_victim(&mut self, _id: ObjectId) -> ObjectId {
        self.ais_cut("member_ai_current_victim");
        INVALID_ID
    }
    fn set_disabled_held(&mut self, held: bool) {
        let id = self.ai_self;
        if held {
            self.set_disabled(id, crate::common::disabled_types::DISABLED_HELD);
        } else {
            self.clear_disabled(id, crate::common::disabled_types::DISABLED_HELD);
        }
    }
    fn highest_layer_for_healthy_destination(&mut self, pos: &Coord3D) -> crate::common::game_type::PathfindLayer {
        self.terrain.layer_for_destination_uncounted(&*self, pos).unwrap_or(crate::common::game_type::LAYER_GROUND)
    }
    fn pf_adjust_to_landing_destination(&mut self, pos: &mut Coord3D) {
        let mut p = *pos;
        if self.ai_pf(|pf, w, o, _| pf.adjust_to_landing_destination(w, o, &mut p)).is_some() {
            *pos = p;
        }
    }
    fn body_health(&mut self) -> f32 {
        let id = self.ai_self;
        World::body_health(self, id).map_or(0.0, |h| h.0)
    }
    fn action_can_enter_object_combat_drop(&mut self, target: ObjectId, src: CommandSourceType) -> bool {
        let o = self.ai_self;
        let r = crate::logic::action_manager::can_enter_object(self, Some(o), Some(target), crate::logic::world::ai_combat_world::source(src), crate::logic::action_manager::CanEnterType::CombatDropInto);
        self.ai_action_answer("ActionManager::canEnterObject", r)
    }
    fn drawable_pristine_bone_positions(&mut self, _prefix: &[u8], _max: usize) -> Vec<Coord3D> {
        self.ais_cut("drawable_pristine_bone_positions");
        Default::default()
    }
    fn drawable_pristine_bone_transforms(&mut self, _prefix: &[u8], _max: usize) -> Vec<crate::ww::matrix3d::Matrix3D> {
        self.ais_cut("drawable_pristine_bone_transforms");
        Default::default()
    }
    fn own_convert_bone_transform_to_world(&mut self, _mtx: &crate::ww::matrix3d::Matrix3D) -> crate::ww::matrix3d::Matrix3D {
        self.ais_cut("own_convert_bone_transform_to_world");
        Default::default()
    }
    fn own_convert_bone_pos_to_world_pos(&mut self, _pos: &Coord3D) -> Coord3D {
        self.ais_cut("own_convert_bone_pos_to_world_pos");
        Coord3D::default()
    }
    fn new_rope_drawable(&mut self) -> Option<u32> {
        self.ais_cut("new_rope_drawable");
        Default::default()
    }
    fn rope_call(&mut self, _drawable: u32, _call: crate::logic::ai::chinook_ai_update::RopeCall) {
        self.ais_cut("rope_call");
    }
    fn own_exit_reserve_door(&mut self, exiter: ObjectId) -> i32 {
        let us = self.ai_self;
        let template = self.find_object(exiter).map(|o| o.template().get_template_id());
        match self.get_object_exit_interface(us) {
            Some(exit) => self.module_call(us, exit, MC::ReserveDoorForExit { template, specific: exiter }).as_int(DOOR_NONE_AVAILABLE),
            None => DOOR_NONE_AVAILABLE,
        }
    }
    fn own_exit_object_via_door(&mut self, exiter: ObjectId, door: i32) {
        let us = self.ai_self;
        if let Some(exit) = self.get_object_exit_interface(us) {
            self.module_call(us, exit, MC::ExitObjectViaDoor { obj: exiter, door });
        }
    }
    fn object_set_transform_matrix(&mut self, id: ObjectId, mtx: &crate::ww::matrix3d::Matrix3D) {
        World::set_transform_matrix(self, id, mtx);
    }
    fn local_player_shroud_clear(&mut self) -> bool {
        self.ais_cut("local_player_shroud_clear");
        Default::default()
    }
    fn client_random_value_real(&mut self, _lo: f32, _hi: f32) -> f32 {
        self.ais_cut("client_random_value_real");
        0.0
    }
    fn rotor_wash(&mut self, _pos: &Coord3D) {
        self.ais_cut("rotor_wash");
    }
    fn pp_has_reserved_space(&mut self, _airfield: ObjectId, _id: ObjectId) -> bool {
        self.ais_cut("pp_has_reserved_space");
        Default::default()
    }
    fn pp_reserve_space(&mut self, _airfield: ObjectId, _id: ObjectId, _parking_offset: f32, _want_info: bool) -> Option<crate::logic::ai::jet_ai_update::PpInfo> {
        self.ais_cut("pp_reserve_space");
        Default::default()
    }
    fn pp_release_space(&mut self, _airfield: ObjectId, _id: ObjectId) {
        self.ais_cut("pp_release_space");
    }
    fn pp_reserve_runway(&mut self, _airfield: ObjectId, _id: ObjectId, _for_landing: bool) -> bool {
        self.ais_cut("pp_reserve_runway");
        Default::default()
    }
    fn pp_release_runway(&mut self, _airfield: ObjectId, _id: ObjectId) {
        self.ais_cut("pp_release_runway");
    }
    fn pp_calc_pp_info(&mut self, _airfield: ObjectId, _id: ObjectId) -> crate::logic::ai::jet_ai_update::PpInfo {
        self.ais_cut("pp_calc_pp_info");
        Default::default()
    }
    fn pp_runway_count(&mut self, _airfield: ObjectId) -> i32 {
        self.ais_cut("pp_runway_count");
        Default::default()
    }
    fn pp_runway_reservation(&mut self, _airfield: ObjectId, _r: i32) -> ObjectId {
        self.ais_cut("pp_runway_reservation");
        INVALID_ID
    }
    fn pp_transfer_runway_reservation(&mut self, _airfield: ObjectId, _id: ObjectId) {
        self.ais_cut("pp_transfer_runway_reservation");
    }
    fn pp_approach_height(&mut self, _airfield: ObjectId) -> f32 {
        self.ais_cut("pp_approach_height");
        0.0
    }
    fn pp_landing_deck_height_offset(&mut self, _airfield: ObjectId) -> f32 {
        self.ais_cut("pp_landing_deck_height_offset");
        0.0
    }
    fn pp_calc_best_parking_assignment(&mut self, _airfield: ObjectId, _id: ObjectId) -> Option<Coord3D> {
        self.ais_cut("pp_calc_best_parking_assignment");
        Default::default()
    }
    fn pp_taxi_locations(&mut self, _airfield: ObjectId, _id: ObjectId) -> Option<Vec<Coord3D>> {
        self.ais_cut("pp_taxi_locations");
        Default::default()
    }
    fn pp_creation_locations(&mut self, _airfield: ObjectId, _id: ObjectId) -> Option<Vec<Coord3D>> {
        self.ais_cut("pp_creation_locations");
        Default::default()
    }
    fn jet_airfield_candidate(&mut self) -> Option<ObjectId> {
        self.ais_cut("jet_airfield_candidate");
        Default::default()
    }
    fn log_find_suitable_airfield(&mut self, _jet: ObjectId, _found: ObjectId) {
        self.ais_cut("log_find_suitable_airfield");
    }
    fn weapon_returns_to_base_to_reload(&mut self, slot: WeaponSlotType) -> bool {
        self.ai_weapon_clip(self.ai_self, slot).is_some_and(|c| c.0)
    }
    fn weapon_remaining_ammo(&mut self, slot: WeaponSlotType) -> i32 {
        self.ai_weapon_clip(self.ai_self, slot).map_or(0, |c| c.1)
    }
    fn weapon_clip_size(&mut self, slot: WeaponSlotType) -> i32 {
        self.ai_weapon_clip(self.ai_self, slot).map_or(0, |c| c.2)
    }
    fn weapon_clip_reload_time(&mut self, slot: WeaponSlotType) -> i32 {
        self.ai_weapon_clip(self.ai_self, slot).map_or(0, |c| c.3)
    }
    fn weapon_set_clip_percent_full(&mut self, slot: WeaponSlotType, percent: f32, allow_reduction: bool) {
        self.ai_weapon_set_clip_percent_full(self.ai_self, slot, percent, allow_reduction);
    }
    fn has_countermeasures(&mut self) -> bool {
        self.ais_cut("has_countermeasures");
        Default::default()
    }
    fn reload_countermeasures(&mut self) {
        self.ais_cut("reload_countermeasures");
    }
    fn play_per_unit_sound_for(&mut self, _name: &[u8], _object: ObjectId) -> u32 {
        self.ais_cut("play_per_unit_sound_for");
        Default::default()
    }
    fn play_wheel_screech(&mut self, _pos: &Coord3D) {
        self.ais_cut("play_wheel_screech");
    }
    fn play_lockon_tick(&mut self, _object: ObjectId) {
        self.ais_cut("play_lockon_tick");
    }
    fn audio_is_currently_playing(&mut self, _handle: u32) -> bool {
        self.ais_cut("audio_is_currently_playing");
        Default::default()
    }
    fn drawable_set_instance_matrix(&mut self, _z: Option<f32>) {
        self.ais_cut("drawable_set_instance_matrix");
    }
    fn drawable_enable_ambient_sound(&mut self, _enable: bool) {
        self.ais_cut("drawable_enable_ambient_sound");
    }
    fn drawable_stop_ambient_sound(&mut self) {
        self.ais_cut("drawable_stop_ambient_sound");
    }
    fn drawable_set_orientation(&mut self, _drawable: u32, _angle: f32) {
        self.ais_cut("drawable_set_orientation");
    }
    fn lockon_set_hidden(&mut self, _hidden: bool) {
        self.ais_cut("lockon_set_hidden");
    }
    fn destroy_drawable(&mut self, _drawable: u32) {
        self.ais_cut("destroy_drawable");
    }
    fn other_ai_current_state_id(&mut self, id: ObjectId) -> StateId {
        self.ai_machine_query(id).map_or(crate::logic::ai::ai_update_types::AI_IDLE, |m| m.get_current_state_id())
    }
    fn supply_machine_stack(&mut self) -> &mut Vec<Option<StateMachine<crate::logic::ai::supply_truck_ai_update::SupplyState<Self>>>> where Self: Sized {
        &mut self.ai_stacks.supply
    }
    fn worker_machine_stack(&mut self) -> &mut Vec<Option<StateMachine<crate::logic::ai::worker_ai_update::WorkerState<Self>>>> where Self: Sized {
        &mut self.ai_stacks.worker
    }
    fn object_set_status(&mut self, id: ObjectId, status: crate::common::object_status_types::ObjectStatusTypes) {
        World::set_status(self, id, crate::logic::world::object_state::mask_of(status), true);
    }
    fn dozer_machine_stack(&mut self) -> &mut Vec<Option<StateMachine<crate::logic::ai::dozer_ai_update::DozerPrimaryState<Self>>>> where Self: Sized {
        &mut self.ai_stacks.dozer
    }
    fn ai_data_dozer_bored_radius_modifier(&mut self) -> f32 {
        self.statics.ai_data.data.ai_dozer_bored_radius_modifier
    }
    fn set_weapon_set_flag(&mut self, flag: crate::logic::weapon_types::WeaponSetType, set: bool) {
        let o = self.ai_self;
        if set {
            World::set_weapon_set_flag(self, o, flag);
        } else {
            World::clear_weapon_set_flag(self, o, flag);
        }
    }
    fn object_major_radius(&mut self, id: ObjectId) -> f32 {
        self.find_object(id).map_or(0.0, |o| o.geometry_info.get_major_radius())
    }
    fn object_bounding_sphere_radius(&mut self, id: ObjectId) -> f32 {
        self.find_object(id).map_or(0.0, |o| o.geometry_info.get_bounding_sphere_radius())
    }
    fn bounding_sphere_radius(&mut self) -> f32 {
        self.find_object(self.ai_self).map_or(0.0, |o| o.geometry_info.get_bounding_sphere_radius())
    }
    fn find_position_around_options(
        &mut self,
        pos: &Coord3D,
        min_radius: f32,
        max_radius: f32,
        source: bool,
        start_angle: Option<f32>,
        max_z_delta: f32,
        ignore: ObjectId,
    ) -> Option<Coord3D> {
        let options = FindPositionOptions {
            min_radius,
            max_radius,
            start_angle: start_angle.unwrap_or(RANDOM_START_ANGLE),
            max_z_delta,
            ignore_object: (ignore != INVALID_ID).then_some(ignore),
            source_to_path_to_dest: source.then_some(self.ai_self),
            ..FindPositionOptions::default()
        };
        self.ai_find_position_around("find_position_around_options", pos, options)
    }
    fn distance_squared_to_position(&mut self, pos: &Coord3D, kind: DistanceKind) -> f32 {
        self.ai_distance_squared_pos(self.ai_self, pos, kind)
    }
    fn bridge_tower_ids(&mut self, _bridge: ObjectId) -> Option<[ObjectId; 4]> {
        self.ais_cut("bridge_tower_ids");
        Default::default()
    }
    fn bridge_tower_bridge_id(&mut self, _tower: ObjectId) -> Option<ObjectId> {
        self.ais_cut("bridge_tower_bridge_id");
        Default::default()
    }
    fn bridge_scaffold_in_motion(&mut self, _bridge: ObjectId) -> Option<bool> {
        self.ais_cut("bridge_scaffold_in_motion");
        Default::default()
    }
    fn bridge_scaffolding(&mut self, _bridge: ObjectId, _create: bool) {
        self.ais_cut("bridge_scaffolding");
    }
    fn object_set_builder(&mut self, id: ObjectId, builder: ObjectId) {
        if let Some(o) = self.find_object_mut(id) {
            o.builder_id = builder;
        }
    }
    fn object_set_producer(&mut self, id: ObjectId, producer: ObjectId) {
        if let Some(o) = self.find_object_mut(id) {
            o.producer_id = producer;
        }
    }
    fn object_builder_id(&mut self, id: ObjectId) -> ObjectId {
        self.find_object(id).map_or(INVALID_ID, |o| o.builder_id)
    }
    fn object_sole_healing_benefactor(&mut self, id: ObjectId) -> ObjectId {
        self.get_sole_healing_benefactor(id)
    }
    fn action_can_repair_object(&mut self, target: ObjectId, src: CommandSourceType) -> bool {
        let o = self.ai_self;
        let r = crate::logic::action_manager::can_repair_object(self, Some(o), Some(target), crate::logic::world::ai_combat_world::source(src));
        self.ai_action_answer("ActionManager::canRepairObject", r)
    }
    fn action_can_resume_construction_of(&mut self, target: ObjectId, src: CommandSourceType) -> bool {
        let o = self.ai_self;
        crate::logic::action_manager::can_resume_construction_of(self, Some(o), Some(target), crate::logic::world::ai_combat_world::source(src))
    }
    fn object_same_controlling_player(&mut self, id: ObjectId) -> bool {
        self.get_controlling_player(self.ai_self) == self.get_controlling_player(id)
    }
    fn object_time_to_build(&mut self, id: ObjectId) -> i32 {
        let Some(p) = self.get_controlling_player(self.ai_self) else { return 0 };
        let Some(t) = self.find_object(id).map(|o| u32::from(o.template().get_template_id())) else { return 0 };
        self.calc_time_to_build(t, p)
    }
    fn object_construction_percent(&mut self, id: ObjectId) -> f32 {
        self.find_object(id).map_or(0.0, |o| o.construction_percent)
    }
    fn object_set_construction_percent(&mut self, id: ObjectId, percent: f32) {
        if let Some(o) = self.find_object_mut(id) {
            o.construction_percent = percent;
        }
    }
    fn object_max_health(&mut self, id: ObjectId) -> f32 {
        World::body_health(self, id).map_or(0.0, |h| h.1)
    }
    fn object_internal_change_health(&mut self, id: ObjectId, delta: f32) {
        if let Some(b) = self.get_body_module(id) {
            self.module_call(id, b, crate::logic::world::module_call::ModuleCall::InternalChangeHealth(delta));
        }
    }
    fn object_evaluate_visual_condition(&mut self, id: ObjectId) {
        if let Some(b) = self.get_body_module(id) {
            self.module_call(id, b, crate::logic::world::module_call::ModuleCall::EvaluateVisualCondition);
        }
    }
    fn object_clear_status(&mut self, id: ObjectId, status: crate::common::object_status_types::ObjectStatusTypes) {
        World::set_status(self, id, crate::logic::world::object_state::mask_of(status), false);
    }
    fn object_clear_model_condition_flags(&mut self, id: ObjectId, conditions: &[ModelConditionFlagType]) {
        for c in conditions {
            World::clear_model_condition_state(self, id, *c);
        }
    }
    fn object_clear_and_set_model_condition_flags(&mut self, id: ObjectId, clear: &[ModelConditionFlagType], set: &[ModelConditionFlagType]) {
        for c in clear {
            World::clear_model_condition_state(self, id, *c);
        }
        for c in set {
            World::set_model_condition_state(self, id, *c);
        }
    }
    fn object_clear_model_condition_state(&mut self, id: ObjectId, condition: ModelConditionFlagType) {
        World::clear_model_condition_state(self, id, condition);
    }
    fn player_on_structure_construction_complete(&mut self, structure: ObjectId, is_rebuild: bool) {
        let me = self.ai_self;
        let Some(p) = self.get_controlling_player(me) else { return };
        if let Err(e) = crate::logic::world::player_objects::player_on_structure_construction_complete(self, p, Some(me), structure, is_rebuild) {
            self.cuts.team_live_errors.push(format!("Player::onStructureConstructionComplete: {e:?}"));
        }
    }
    fn academy_record_production(&mut self, _structure: ObjectId) {
        self.ais_cut("academy_record_production");
    }
    fn object_on_build_complete(&mut self, id: ObjectId) {
        for n in 0..crate::logic::world::build_assistant::BuildAssistantHost::obj_create_module_count(self, id) {
            crate::logic::world::build_assistant::BuildAssistantHost::create_module_on_build_complete(self, id, n);
        }
    }
    fn object_handle_partition_cell_maintenance(&mut self, id: ObjectId) {
        self.handle_partition_cell_maintenance(id);
    }
    fn object_update_upgrade_modules(&mut self, id: ObjectId) {
        self.update_upgrade_modules(id);
    }
    fn dozer_construction_complete_ui(&mut self, _structure: ObjectId) {
        self.ais_cut("dozer_construction_complete_ui");
    }
    fn in_game_ui_message(&mut self, _label: &[u8]) {
        self.ais_cut("in_game_ui_message");
    }
    fn in_game_ui_idle_worker(&mut self, _add: bool, _player_number: i32) {
        self.ais_cut("in_game_ui_idle_worker");
    }
    fn player_index(&mut self) -> Option<i32> {
        self.get_controlling_player(self.ai_self)
    }
    fn object_attempt_healing_from_sole_benefactor(&mut self, _id: ObjectId, _amount: f32, _healer: ObjectId, _frames: u32) -> bool {
        self.ais_cut("object_attempt_healing_from_sole_benefactor");
        Default::default()
    }
    fn partition_repair_candidates(&mut self, pos: &Coord3D, range: f32) -> Vec<ObjectId> {
        self.ai_repair_candidates(self.ai_self, pos, range)
    }
    fn partition_closest_mine(&mut self, range: f32) -> ObjectId {
        self.ai_closest_mine(self.ai_self, range)
    }
    fn play_under_construction_sound(&mut self, _site: ObjectId) -> u32 {
        self.ais_cut("play_under_construction_sound");
        Default::default()
    }
    fn build_assistant_can_make_unit(&mut self, template: u32) -> bool {
        let me = self.ai_self;
        matches!(crate::logic::world::build_assistant::build_assistant_can_make_unit(self, me, template), Ok(crate::logic::world::build_assistant::CanMakeType::Ok))
    }
    fn build_assistant_is_location_legal_to_build(&mut self, template: u32, pos: &Coord3D, angle: f32, options: u32) -> bool {
        let me = self.ai_self;
        let p = self.get_controlling_player(me);
        crate::logic::world::build_assistant::build_assistant_is_location_legal_to_build(self, *pos, template, angle, options, Some(me), p) == crate::logic::world::build_assistant::LegalBuildCode::Ok
    }
    fn thing_factory_new_structure(&mut self, template: u32, is_rebuild: bool) -> ObjectId {
        use crate::common::object_status_types::{OBJECT_STATUS_RECONSTRUCTING, OBJECT_STATUS_UNDER_CONSTRUCTION};
        let Some(p) = self.get_controlling_player(self.ai_self) else { return INVALID_ID };
        let status: &[ObjectStatusTypes] = if is_rebuild { &[OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_RECONSTRUCTING] } else { &[OBJECT_STATUS_UNDER_CONSTRUCTION] };
        crate::logic::world::build_assistant::BuildAssistantHost::thing_factory_new_object(self, template, p, status)
    }
    fn template_cost_to_build(&mut self, template: u32) -> i32 {
        let Some(p) = self.get_controlling_player(self.ai_self) else { return 0 };
        crate::logic::world::build_assistant::BuildAssistantHost::tmpl_calc_cost_to_build(self, template, p)
    }
    fn player_money_withdraw(&mut self, amount: i32) -> bool {
        // (false only without the player's money: upstream does not look at what was withdrawn)
        let Some(p) = self.get_controlling_player(self.ai_self) else { return false };
        crate::logic::world::player_objects::player_money_withdraw(self, p, amount as u32, true).is_ok()
    }
    fn new_structure_set_position(&mut self, id: ObjectId, pos: &Coord3D) {
        crate::logic::world::build_assistant::BuildAssistantHost::obj_set_position(self, id, *pos);
    }
    fn new_structure_set_orientation(&mut self, id: ObjectId, angle: f32) {
        crate::logic::world::build_assistant::BuildAssistantHost::obj_set_orientation(self, id, angle);
    }
    fn terrain_flatten(&mut self, id: ObjectId) {
        if !self.terrain_flatten_object(id) {
            self.ais_cut("terrain_flatten");
        }
    }
    fn pf_add_object_to_pathfind_map(&mut self, id: ObjectId) {
        self.pathfinder_add_object_to_map(id);
    }
    fn player_on_structure_created(&mut self, _structure: ObjectId) {
        // `Player::onStructureCreated` is empty upstream
    }
    fn deliver_machine_stack(&mut self) -> &mut Vec<Option<StateMachine<crate::logic::ai::deliver_payload_ai_update::DeliverState<Self>>>> where Self: Sized {
        &mut self.ai_stacks.deliver
    }
    fn delivery_decal_create(&mut self, _template: &crate::logic::radius_decal::RadiusDecalTemplate, _pos: &Coord3D, _radius: f32, _has_player: bool) {
        self.ais_cut("delivery_decal_create");
    }
    fn delivery_decal_update(&mut self) {
        self.ais_cut("delivery_decal_update");
    }
    fn delivery_decal_clear(&mut self) {
        self.ais_cut("delivery_decal_clear");
    }
    fn play_per_unit_sound_at(&mut self, _name: &[u8], _pos: &Coord3D) {
        self.ais_cut("play_per_unit_sound_at");
    }
    fn set_weapon_lock(&mut self, slot: i32, lock: i32) {
        let o = self.ai_self;
        crate::logic::world::object_weapons::set_weapon_lock(self, o, slot, lock);
    }
    fn fx_do_pos(&mut self, _fx: &crate::common::damage_fx::FxListName, _pos: &Coord3D) {
        self.ais_cut("fx_do_pos");
    }
    fn has_drawable(&mut self) -> bool {
        World::has_drawable(self, self.ai_self)
    }
    fn drawable_show_sub_object(&mut self, _name: &[u8], _show: bool) {
        self.ais_cut("drawable_show_sub_object");
    }
    fn drawable_update_sub_objects(&mut self) {
        self.ais_cut("drawable_update_sub_objects");
    }
    fn drawable_pristine_bone_position(&mut self, _prefix: &[u8], _start_index: i32) -> Option<Coord3D> {
        self.ais_cut("drawable_pristine_bone_position");
        Default::default()
    }
    fn drawable_convert_bone_pos_to_world_pos(&mut self, _pos: &Coord3D) -> Coord3D {
        self.ais_cut("drawable_convert_bone_pos_to_world_pos");
        Coord3D::default()
    }
    fn object_destroy(&mut self, id: ObjectId) {
        self.destroy_object(id);
    }
    fn object_set_position(&mut self, id: ObjectId, pos: &Coord3D) {
        World::set_position(self, id, pos);
    }
    fn object_contain_set_override_destination(&mut self, id: ObjectId, pos: &Coord3D) {
        self.ai_contain_call(id, MC::SetOverrideDestination(*pos));
    }
    fn object_aim_update_module(&mut self, _id: ObjectId, _module: crate::logic::ai::deliver_payload_ai_update::ItemModule, _pos: &Coord3D) {
        self.ais_cut("object_aim_update_module");
    }
    fn object_apply_force(&mut self, _id: ObjectId, _force: &Coord3D) {
        self.ais_cut("object_apply_force");
    }
    fn thing_template_by_name(&mut self, name: &[u8]) -> Option<u32> {
        World::thing_factory_find_template(self, name).map(u32::from)
    }
    fn payload_new(&mut self, _name: &[u8]) -> ObjectId {
        self.ais_cut("payload_new");
        INVALID_ID
    }
    fn payload_set_producer(&mut self, _id: ObjectId) {
        self.ais_cut("payload_set_producer");
    }
    fn payload_set_position(&mut self, _id: ObjectId, _pos: &Coord3D) {
        self.ais_cut("payload_set_position");
    }
    fn payload_position(&mut self, _id: ObjectId) -> Coord3D {
        self.ais_cut("payload_position");
        Coord3D::default()
    }
    fn payload_set_orientation(&mut self, _id: ObjectId, _angle: f32) {
        self.ais_cut("payload_set_orientation");
    }
    fn payload_mass(&mut self, _id: ObjectId) -> f32 {
        self.ais_cut("payload_mass");
        0.0
    }
    fn payload_apply_motive_force(&mut self, _id: ObjectId, _force: &Coord3D) {
        self.ais_cut("payload_apply_motive_force");
    }
    fn payload_is_projectile(&mut self, _id: ObjectId) -> bool {
        self.ais_cut("payload_is_projectile");
        Default::default()
    }
    fn payload_projectile_fire(&mut self, _id: ObjectId, _pos: &Coord3D, _weapon: u32, _exhaust: Option<u32>) {
        self.ais_cut("payload_projectile_fire");
    }
    fn payload_center_of_mass_offset(&mut self, _id: ObjectId) -> f32 {
        self.ais_cut("payload_center_of_mass_offset");
        0.0
    }
    fn payload_set_pitch_rate(&mut self, _id: ObjectId, _rate: f32) {
        self.ais_cut("payload_set_pitch_rate");
    }
    fn payload_has_ai(&mut self, _id: ObjectId) -> bool {
        self.ais_cut("payload_has_ai");
        Default::default()
    }
    fn payload_ai_move_to_position(&mut self, _id: ObjectId, _pos: &Coord3D) {
        self.ais_cut("payload_ai_move_to_position");
    }
    fn weapon_template_projectile_exhaust(&mut self, _template: u32, _level: crate::common::game_common::VeterancyLevel) -> Option<u32> {
        self.ais_cut("weapon_template_projectile_exhaust");
        Default::default()
    }
    fn terrain_extent_including_border(&mut self) -> (Coord3D, Coord3D) {
        // W3DTerrainLogic::getExtentIncludingBorder writes only x and y; both callers read only x and y, so z is 0 here
        let r = self.terrain_get_extent_including_border();
        (Coord3D { x: r.lo.x, y: r.lo.y, z: 0.0 }, Coord3D { x: r.hi.x, y: r.hi.y, z: 0.0 })
    }
    fn partition_register(&mut self) {
        self.ais_cut("partition_register");
    }
    fn partition_unregister(&mut self) {
        self.ais_cut("partition_unregister");
    }
    fn physics_turning(&mut self) -> i32 {
        self.ai_physics_call(crate::logic::world::module_call::ModuleCall::PhysicsGetTurning).map_or(0, |r| r.as_int(0))
    }
    fn unit_direction_vector_3d(&mut self) -> Coord3D {
        self.find_object(self.ai_self).map(|o| o.thing.unit_direction_vector_3d()).unwrap_or_default()
    }
    fn pf_goal_position(&mut self) -> Option<Coord3D> {
        let mut p = Coord3D::default();
        (self.ai_pf(|pf, w, o, _| pf.goal_position(w, o, &mut p)) == Some(true)).then_some(p)
    }
    fn pf_snap_position(&mut self, pos: &mut Coord3D) {
        let id = self.ai_self;
        self.pathfinder_snap_position(id, pos);
    }
    fn pf_update_goal(&mut self, pos: &Coord3D, layer: PathfindLayer) {
        let id = self.ai_self;
        self.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::UpdateGoal(id, *pos, layer));
    }
    fn pf_queue_for_path(&mut self) {
        let id = self.ai_self;
        if self.pf_ready().is_some() {
            self.with_pathfinder(|pf, _| pf.queue_for_path(id));
        }
    }
    fn turret_update(&mut self, which: usize) -> UpdateSleepTime {
        // (batch I9b-6: the turret's AI, with the AI lent: `AIUpdateInterface::update` lends it around the turrets)
        let id = self.ai_self;
        match self.with_turret(id, which, |t, w| t.update_turret_ai(w)) {
            Some(Ok(sleep)) => sleep,
            Some(Err(e)) => {
                self.cuts.team_live_errors.push(format!("TurretAI({which})::updateTurretAI: {e:?}"));
                crate::logic::module::update_module::UPDATE_SLEEP_NONE
            }
            None => crate::logic::module::update_module::UPDATE_SLEEP_FOREVER,
        }
    }
    fn find_closest_repulsor(&mut self, range: f32) -> ObjectId {
        // (batch I9c-3: with `live_ai`, `TheAI`'s own query)
        if self.cuts.live_ai {
            let id = self.ai_self;
            return self.ai_find_closest_repulsor_live(id, range);
        }
        self.ais_cut("find_closest_repulsor");
        INVALID_ID
    }
    fn object_vision_range(&self) -> f32 {
        self.get_vision_range(self.ai_self)
    }
    fn clear_and_set_model_condition_flags(&mut self, clear: &[ModelConditionFlagType], set: ModelConditionFlagType) {
        let id = self.ai_self;
        for c in clear {
            World::clear_model_condition_state(self, id, *c);
        }
        World::set_model_condition_state(self, id, set);
    }
    fn notify_of_object_creation_or_destruction(&mut self) {
        self.ais_cut("notify_of_object_creation_or_destruction");
    }
    fn is_mobile(&self) -> bool {
        World::is_mobile(self, self.ai_self)
    }
    fn partition_cell_size(&self) -> f32 {
        self.statics.global_data.the_global_data().map_or(0.0, |g| g.partition_cell_size)
    }
    fn is_significantly_above_terrain(&mut self) -> bool {
        let id = self.ai_self;
        World::is_significantly_above_terrain(self, id)
    }
    fn terrain_extent(&self) -> (Coord3D, Coord3D) {
        self.terrain.extent().map_or((Coord3D::default(), Coord3D::default()), |r| (r.lo, r.hi))
    }
    fn terrain_find_closest_edge_point(&mut self, pos: &Coord3D) -> Coord3D {
        self.terrain.find_closest_edge_point(pos).unwrap_or(*pos)
    }
    fn team_member_ids(&mut self, team: u32) -> Vec<ObjectId> {
        self.teams.members(team)
    }
    fn action_can_get_healed_at(&mut self, depot: ObjectId, src: CommandSourceType) -> bool {
        let o = self.ai_self;
        crate::logic::action_manager::can_get_healed_at(self, Some(o), Some(depot), crate::logic::world::ai_combat_world::source(src))
    }
    fn action_can_get_repaired_at(&mut self, depot: ObjectId, src: CommandSourceType) -> bool {
        let o = self.ai_self;
        let r = crate::logic::action_manager::can_get_repaired_at(self, Some(o), Some(depot), crate::logic::world::ai_combat_world::source(src));
        self.ai_action_answer("ActionManager::canGetRepairedAt", r)
    }
    fn action_can_enter_object(&mut self, target: ObjectId, src: CommandSourceType) -> bool {
        let o = self.ai_self;
        let r = crate::logic::action_manager::can_enter_object(self, Some(o), Some(target), crate::logic::world::ai_combat_world::source(src), crate::logic::action_manager::CanEnterType::DontCheckCapacity);
        self.ai_action_answer("ActionManager::canEnterObject", r)
    }
    fn object_is_disabled_by_type(&self, id: ObjectId, t: DisabledType) -> bool {
        self.find_object(id).is_some_and(|o| o.is_disabled_by_type(t))
    }
    fn object_ai_has_path(&self, id: ObjectId) -> bool {
        self.ai_peek(id).is_some_and(|a| a.path.is_some())
    }
    fn pf_move_away_from_path(&mut self, unit: ObjectId, other: ObjectId, other_has_path: bool) -> Option<Path> {
        // `getMoveAwayFromPath(obj, unit, unit's AI's path, obj2, obj2's AI's path)` (none without the unit's path)
        let avoid = self.ai_peek(unit)?.path.clone()?;
        let avoid2 = if other_has_path { self.ai_peek(other).and_then(|a| a.path.clone()) } else { None };
        let unit_facts = self.pf_object_facts(unit);
        self.ai_pf(|pf, w, o, _| pf.get_move_away_from_path(w, o, unit_facts.as_ref(), &avoid, avoid2.as_ref())).flatten()
    }
    fn pf_move_allies(&mut self, path: &Path) {
        self.ai_pf_mut(|pf, w, o, _| pf.move_allies(w, o, path));
    }
    fn is_path_available(&mut self, destination: &Coord3D) -> bool {
        self.ai_pf(|pf, w, o, loco| pf.client_safe_quick_does_path_exist(w, loco.valid_surfaces, &o.pos, destination)).unwrap_or(false)
    }
    fn weapon_set_max_shot_count(&mut self, n: i32) {
        self.ai_weapon_set_max_shot_count(self.ai_self, n);
    }
    fn weapon_continue_attack_range(&self) -> f32 {
        self.objects.find(self.ai_self).and_then(|o| o.weapon_set.get_cur_weapon()).and_then(|w| self.statics.weapon_store.templates().get(w.template())).map_or(0.0, |t| t.continue_attack_range)
    }
    fn weapon_is_contact(&self) -> bool {
        self.objects.find(self.ai_self).and_then(|o| o.weapon_set.get_cur_weapon()).and_then(|w| self.statics.weapon_store.templates().get(w.template())).is_some_and(|t| t.is_contact_weapon())
    }
    fn partition_closest_attack_target(&mut self, pos: &Coord3D, range: f32, src: CommandSourceType) -> ObjectId {
        self.ai_closest_attack_target(self.ai_self, pos, range, src, None)
    }
    fn partition_closest_attack_target_of_player(&mut self, pos: &Coord3D, range: f32, src: CommandSourceType, player: Option<crate::logic::world::ids::PlayerIndex>) -> ObjectId {
        self.ai_closest_attack_target(self.ai_self, pos, range, src, Some(player))
    }
    fn object_controlling_player(&mut self, id: ObjectId) -> Option<crate::logic::world::ids::PlayerIndex> {
        self.get_controlling_player(id)
    }
    fn find_position_around(&mut self, pos: &Coord3D, min_radius: f32, max_radius: f32) -> Option<Coord3D> {
        let options = FindPositionOptions { min_radius, max_radius, source_to_path_to_dest: Some(self.ai_self), ..FindPositionOptions::default() };
        self.ai_find_position_around("find_position_around", pos, options)
    }
    fn has_contain(&self) -> bool {
        self.get_contain(self.ai_self).is_some()
    }
    fn contain_idle_passengers(&mut self, src: CommandSourceType) {
        let us = self.ai_self;
        for rider in self.ai_contained_items(us) {
            self.ai_ask(rider, MC::AiIdleFrom { command_source: src });
        }
    }
    fn contain_remove_all_contained(&mut self) {
        self.ai_own_contain_call(MC::RemoveAllContained { expose_stealth_units: false });
    }
    fn contain_mark_all_passengers_detected(&mut self) {
        self.ai_own_contain_call(MC::MarkAllPassengersDetected);
    }
    fn contain_order_all_passengers_to_exit(&mut self, src: CommandSourceType, instantly: bool) {
        self.ai_own_contain_call(MC::OrderAllPassengersToExit { command_source: src, instantly });
    }
    fn go_prone(&mut self) {
        // (batch I9b-6: `privateGoProne(&parms->m_damage)`: the object's `ProneUpdate`, if it has one, with the damage the
        // command carries; a go-prone command without one (no `aiGoProne` running) is the default `DamageInfo`)
        let id = self.ai_self;
        if let Some(idx) = self.find_module(id, b"ProneUpdate") {
            let info = self.ai_prone_damage.clone().unwrap_or_default();
            self.module_call(id, idx, crate::logic::world::module_call::ModuleCall::ProneGoProne(info));
        }
    }
    fn polygon_center(&mut self, _polygon: i32) -> Coord3D {
        self.ais_cut("polygon_center");
        Coord3D::default()
    }
    fn command_set_button_matches(&mut self, _button: u32) -> (u32, i32) {
        self.ais_cut("command_set_button_matches");
        Default::default()
    }
    fn waypoint_location(&mut self, way: u32) -> Coord3D {
        self.terrain.waypoint_by_id(way).and_then(|h| self.terrain.waypoint(h)).map(|w| *w.location()).unwrap_or_default()
    }
    fn waypoint_link0(&mut self, way: u32) -> Option<u32> {
        let w = self.terrain.waypoint_by_id(way).and_then(|h| self.terrain.waypoint(h))?;
        let l = w.link(0).ok().flatten()?;
        self.terrain.waypoint(l).map(|x| x.id() as u32)
    }
    fn pf_get_aircraft_path(&mut self, destination: &Coord3D) -> Option<Path> {
        self.ai_pf(|pf, w, o, _| pf.get_aircraft_path(w, o, destination))
    }
    fn terrain_max_pathfind_extent(&mut self) -> (Coord3D, Coord3D) {
        self.terrain.maximum_pathfind_extent().map_or((Coord3D::default(), Coord3D::default()), |r| (r.lo, r.hi))
    }
    fn pf_move_allies_away_from_destination(&mut self, destination: &Coord3D) {
        self.ai_pf_mut(|pf, w, o, _| pf.move_allies_away_from_destination(w, o, destination));
    }
    fn pf_set_ignore_obstacle_id(&mut self, id: ObjectId) {
        if let Some(pf) = self.pathfinder.as_mut() {
            pf.ignore_obstacle_id = id;
        }
    }
    fn pf_patch_path(&mut self, path: Option<&Path>, blocked: bool) -> Option<Path> {
        self.ai_pf(|pf, w, o, loco| pf.patch_path(w, o, loco, path, blocked)).flatten()
    }
    fn pf_find_path(&mut self, destination: &Coord3D) -> Option<Path> {
        self.ai_pf(|pf, w, o, loco| pf.find_path(w, Some(o), loco, &o.pos, destination)).flatten()
    }
    fn pf_find_closest_path(&mut self, destination: &Coord3D, blocked: bool, path_cost_multiplier: f32, _move_allies: bool) -> Option<Path> {
        // (`findClosestPath`'s `moveAllies` is not used upstream)
        let mut to = *destination;
        self.ai_pf(|pf, w, o, loco| pf.find_closest_path(w, Some(o), loco, &o.pos, &mut to, blocked, path_cost_multiplier)).flatten()
    }
    fn pf_find_safe_path(&mut self, repulsor_pos1: &Coord3D, repulsor_pos2: &Coord3D, radius: f32) -> Option<Path> {
        self.ai_pf(|pf, w, o, loco| pf.find_safe_path(w, o, loco, &o.pos, repulsor_pos1, repulsor_pos2, radius)).flatten()
    }
    fn pf_find_attack_path(&mut self, victim: ObjectId, victim_pos: &Coord3D) -> Option<Path> {
        let v = self.pf_object_facts(victim);
        self.ai_pf(|pf, w, o, loco| pf.find_attack_path(w, o, loco, &o.pos, v.as_ref(), victim_pos)).flatten()
    }
    fn pf_adjust_to_possible_destination(&mut self, pos: &mut Coord3D) {
        let id = self.ai_self;
        self.pathfinder_adjust_to_possible_destination(id, pos);
    }
    fn pf_adjust_destination(&mut self, pos: &mut Coord3D) -> bool {
        let id = self.ai_self;
        self.pathfinder_adjust_destination(id, pos).unwrap_or(false)
    }
    fn pf_quick_path_exists_for_ui(&mut self, destination: &Coord3D) -> bool {
        self.ai_pf(|pf, w, o, loco| pf.client_safe_quick_does_path_exist(w, loco.valid_surfaces, &o.pos, destination)).unwrap_or(false)
    }
    fn pf_is_attack_view_blocked(&mut self, victim: ObjectId, victim_pos: &Coord3D) -> bool {
        let v = self.pf_object_facts(victim);
        self.ai_pf(|pf, w, o, _| pf.is_attack_view_blocked_by_obstacle(w, o, &o.pos, v.as_ref(), victim_pos)).unwrap_or(false)
    }
    fn ai_data_repulsed_distance(&self) -> f32 {
        self.statics.ai_data.data.repulsed_distance
    }
    fn weapon_is_within_attack_range_of_object(&mut self, victim: ObjectId) -> bool {
        self.ai_weapon_within_range_of_object(self.ai_self, victim)
    }
    fn weapon_is_within_attack_range_of_position(&mut self, pos: &Coord3D) -> bool {
        self.ai_weapon_within_range_of_position(self.ai_self, pos)
    }
    fn weapon_compute_approach_target(&mut self, victim: ObjectId, pos: &Coord3D) -> Coord3D {
        self.ai_weapon_approach_target(self.ai_self, victim, pos)
    }
    fn weapon_is_goal_pos_within_attack_range(&mut self, goal: &Coord3D, victim: ObjectId, victim_pos: &Coord3D) -> bool {
        self.ai_weapon_goal_within_range(self.ai_self, goal, victim, victim_pos)
    }
    fn object_is_significantly_above_terrain(&mut self, id: ObjectId) -> bool {
        World::is_significantly_above_terrain(self, id)
    }
    fn object_is_kind_of(&self, id: ObjectId, kind: KindOfType) -> bool {
        self.find_object(id).is_some_and(|o| o.is_kind_of(kind))
    }
    fn bridge_attack_points(&mut self, _bridge: ObjectId) -> (Coord3D, Coord3D) {
        self.ais_cut("bridge_attack_points");
        Default::default()
    }
    fn distance_squared_bounding_sphere_3d(&mut self, pos: &Coord3D) -> f32 {
        self.ai_distance_squared_pos(self.ai_self, pos, DistanceKind::BoundingSphere3d)
    }
    fn pf_cell_at(&mut self, layer: PathfindLayer, x: i32, y: i32) -> Option<(u8, bool)> {
        let pf = self.pf_ready()?;
        let c = &pf.store.cells[pf.get_cell(layer, x, y)? as usize];
        Some((c.get_type(), c.get_pinched()))
    }
    fn pf_snap_closest_goal_position(&mut self, pos: &mut Coord3D) {
        let mut p = *pos;
        if self.ai_pf(|pf, w, o, _| pf.snap_closest_goal_position(w, o, &mut p)).is_some() {
            *pos = p;
        }
    }
    fn pf_remove_goal(&mut self) {
        let id = self.ai_self;
        if self.pf_ready().is_some() {
            self.with_pathfinder(|pf, w| pf.remove_goal(w, id));
        }
    }
    fn start_move_sound(&mut self, _damaged: bool) -> Option<u32> {
        self.ais_cut("start_move_sound");
        Default::default()
    }
    fn remove_audio_event(&mut self, _handle: u32) {
        self.ais_cut("remove_audio_event");
    }
    fn formation_id(&self) -> i32 {
        self.objects.find(self.ai_self).map_or(0, |o| o.formation_id)
    }
    fn group_speed(&mut self) -> Option<f32> {
        self.ais_cut("group_speed");
        Default::default()
    }
    fn object_max_height_above_position(&self, id: ObjectId) -> f32 {
        self.find_object(id).map_or(0.0, |o| o.geometry_info.get_max_height_above_position())
    }
    fn object_has_physics(&self, id: ObjectId) -> bool {
        self.find_object(id).is_some_and(|o| o.physics.is_some())
    }
    fn object_velocity_magnitude(&mut self, id: ObjectId) -> f32 {
        let Some(ph) = self.find_object(id).and_then(|o| o.physics) else { return 0.0 };
        self.module_call(id, ph, crate::logic::world::module_call::ModuleCall::PhysicsGetVelocityMagnitude).as_real(0.0)
    }
    fn object_unit_direction_vector_3d(&mut self, id: ObjectId) -> Coord3D {
        self.find_object(id).map(|o| o.thing.unit_direction_vector_3d()).unwrap_or_default()
    }
    fn team_set_active(&mut self) {
        let t = self.find_object(self.ai_self).map(|o| o.team);
        if let Some(t) = t {
            let _ = crate::logic::world::team_ops::team_set_active(self, t);
        }
    }
    fn destroy_object(&mut self) {
        let id = self.ai_self;
        self.destroy_object(id);
    }
    fn waypoint_num_links(&mut self, way: u32) -> i32 {
        self.terrain.waypoint_by_id(way).and_then(|h| self.terrain.waypoint(h)).map_or(0, |w| w.num_links())
    }
    fn waypoint_link(&mut self, way: u32, which: i32) -> Option<u32> {
        let w = self.terrain.waypoint_by_id(way).and_then(|h| self.terrain.waypoint(h))?;
        let l = w.link(which).ok().flatten()?;
        self.terrain.waypoint(l).map(|x| x.id() as u32)
    }
    fn pf_is_point_on_wall(&mut self, pos: &Coord3D) -> bool {
        self.pf_wall.is_point_on_wall(pos)
    }
    fn pf_wall_height(&self) -> f32 {
        self.cuts.wall_height
    }
    fn group_center(&mut self) -> Coord3D {
        self.ais_cut("group_center");
        Coord3D::default()
    }
    fn team_current_waypoint(&mut self) -> Option<u32> {
        self.ais_cut("team_current_waypoint");
        Default::default()
    }
    fn team_set_current_waypoint(&mut self, _way: Option<u32>) {
        self.ais_cut("team_set_current_waypoint");
    }
    fn is_skirmish_ai_player(&mut self) -> bool {
        self.ais_cut("is_skirmish_ai_player");
        Default::default()
    }
    fn team_group_center_and_count(&mut self) -> (Coord3D, i32) {
        self.ais_cut("team_group_center_and_count");
        Default::default()
    }
    fn ai_data_skirmish_group_fudge_value(&self) -> f32 {
        self.statics.ai_data.data.skirmish_group_fudge_value
    }
    fn attack_stack(&mut self) -> &mut Vec<AttackFrame> {
        &mut self.ai_stacks.attack
    }
    fn object_contained_by(&self, id: ObjectId) -> ObjectId {
        self.find_object(id).map_or(INVALID_ID, |o| o.contained_by)
    }
    fn ai_data_crushes_infantry(&self) -> bool {
        self.statics.ai_data.data.ai_crushes_infantry
    }
    fn can_crush_or_squish(&mut self, victim: ObjectId) -> bool {
        World::can_crush_or_squish(self, self.ai_self, Some(victim), crate::logic::world::object_damage::CrushSquishTestType::CrushOrSquish)
    }
    fn current_weapon_slot(&self) -> Option<WeaponSlotType> {
        let o = self.objects.find(self.ai_self)?;
        if !o.weapon_set.has_any_weapon() {
            return None;
        }
        match o.weapon_set.get_cur_weapon_slot() {
            0 => Some(WeaponSlotType::Primary),
            1 => Some(WeaponSlotType::Secondary),
            2 => Some(WeaponSlotType::Tertiary),
            _ => None,
        }
    }
    fn weapon_slot_present(&self, slot: WeaponSlotType) -> bool {
        self.objects.find(self.ai_self).is_some_and(|o| o.weapon_set.get_weapon_in_weapon_slot(slot as i32).is_some())
    }
    fn object_is_airborne_target(&mut self, id: ObjectId) -> bool {
        crate::logic::weapon_host::WeaponHost::is_airborne_target(self, id)
    }
    fn is_able_to_attack(&mut self) -> bool {
        let id = self.ai_self;
        World::is_able_to_attack(self, id)
    }
    fn is_out_of_ammo(&self) -> bool {
        let now = crate::common::state_machine::StateContext::frame(self);
        self.objects.find(self.ai_self).is_some_and(|o| o.weapon_set.is_out_of_ammo(now))
    }
    fn is_cur_weapon_locked(&self) -> bool {
        self.objects.find(self.ai_self).is_some_and(|o| o.weapon_set.is_cur_weapon_locked())
    }
    fn has_any_weapon(&self) -> bool {
        self.objects.find(self.ai_self).is_some_and(|o| o.weapon_set.has_any_weapon())
    }
    fn turret_holds_current_weapon(&self, which: usize) -> bool {
        let slot = AiStateHost::current_weapon_slot(self);
        self.turret_peek(self.ai_self, which).is_some_and(|d| slot.is_some_and(|s| crate::logic::ai::turret::is_weapon_slot_on_turret(d, s)))
    }
    fn turret_set_target_object(&mut self, which: usize, o: Option<ObjectId>, force_attacking: bool) {
        let id = self.ai_self;
        if let Some(Err(e)) = self.with_turret(id, which, |t, w| t.set_turret_target_object(w, o, force_attacking)) {
            self.cuts.team_live_errors.push(format!("TurretAI({which})::setTurretTargetObject: {e:?}"));
        }
    }
    fn turret_set_target_position(&mut self, which: usize, pos: Option<&Coord3D>) {
        let id = self.ai_self;
        if let Some(Err(e)) = self.with_turret(id, which, |t, w| t.set_turret_target_position(w, pos)) {
            self.cuts.team_live_errors.push(format!("TurretAI({which})::setTurretTargetPosition: {e:?}"));
        }
    }
    fn turret_turn_rate(&self, which: usize) -> f32 {
        self.turret_peek(self.ai_self, which).map_or(0.0, |d| d.data.turn_rate)
    }
    fn choose_best_weapon_for_target(&mut self, target: Option<ObjectId>, criteria: WeaponChoiceCriteria, src: CommandSourceType) -> bool {
        let o = self.ai_self;
        self.ai_choose_best_weapon(o, target, criteria, src)
    }
    fn adjust_model_condition_for_weapon_status(&mut self) {
        let o = self.ai_self;
        crate::logic::world::object_weapons::adjust_model_condition_for_weapon_status(self, o);
    }
    fn clear_leech_range_mode_for_all_weapons(&mut self) {
        let o = self.ai_self;
        crate::logic::world::object_weapons::clear_leech_range_mode_for_all_weapons(self, o);
    }
    fn object_is_effectively_dead(&self, id: ObjectId) -> bool {
        self.find_object(id).is_some_and(|o| o.is_effectively_dead())
    }
    fn object_is_destroyed(&mut self, id: ObjectId) -> bool {
        self.find_object(id).is_none_or(|o| o.is_destroyed())
    }
    fn object_team(&self, id: ObjectId) -> Option<u32> {
        self.find_object(id).map(|o| o.team)
    }
    fn object_test_status(&self, id: ObjectId, bit: ObjectStatusTypes) -> bool {
        self.find_object(id).is_some_and(|o| o.test_status(bit))
    }
    fn object_has_contain(&self, id: ObjectId) -> bool {
        self.get_contain(id).is_some()
    }
    fn object_relationship(&mut self, that: ObjectId) -> Relationship {
        self.get_relationship(self.ai_self, that)
    }
    fn set_status_bits(&mut self, bits: &[ObjectStatusTypes], set: bool) {
        let id = self.ai_self;
        for b in bits {
            World::set_status(self, id, crate::logic::world::object_state::mask_of(*b), set);
        }
    }
    fn set_team_target_object(&mut self, target: ObjectId) {
        let t = self.find_object(self.ai_self).map(|o| o.team);
        if let Some(x) = t.and_then(|t| self.teams.team_mut(t)) {
            x.common_attack_target = target;
        }
    }
    fn contain_is_garrisonable(&mut self) -> bool {
        let id = self.ai_self;
        let Some(c) = self.get_contain(id) else { return false };
        self.module_call(id, c, crate::logic::world::module_call::ModuleCall::IsGarrisonable).as_bool()
    }
    fn contain_count(&mut self) -> u32 {
        let id = self.ai_self;
        let Some(c) = self.get_contain(id) else { return 0 };
        self.module_call(id, c, crate::logic::world::module_call::ModuleCall::GetContainCount).as_int(0) as u32
    }
    fn contain_is_enclosing_container_for_me(&mut self) -> bool {
        let us = self.ai_self;
        let container = self.find_object(us).map_or(INVALID_ID, |o| o.contained_by);
        self.ai_contain_call(container, MC::IsEnclosingContainerFor(us)).is_some_and(|r| r.as_bool())
    }
    fn contain_attempt_best_fire_point_at_object(&mut self, victim: ObjectId) -> bool {
        let us = self.ai_self;
        let container = self.find_object(us).map_or(INVALID_ID, |o| o.contained_by);
        self.ai_contain_call(container, MC::AttemptBestFirePointPosition { source: us, victim, pos: None }).is_some_and(|r| r.as_bool())
    }
    fn contain_attempt_best_fire_point_at_position(&mut self, pos: &Coord3D) -> bool {
        let us = self.ai_self;
        let container = self.find_object(us).map_or(INVALID_ID, |o| o.contained_by);
        self.ai_contain_call(container, MC::AttemptBestFirePointPosition { source: us, victim: INVALID_ID, pos: Some(*pos) }).is_some_and(|r| r.as_bool())
    }
    fn relative_angle_2d_to_object(&mut self, other: ObjectId) -> f32 {
        self.ai_relative_angle(self.ai_self, other)
    }
    fn relative_angle_2d_to_position(&mut self, pos: &Coord3D) -> f32 {
        self.partition_relative_angle_2d(self.ai_self, pos)
    }
    fn weapon_max_shot_count(&self) -> i32 {
        self.objects.find(self.ai_self).and_then(|o| o.weapon_set.get_cur_weapon()).map_or(0, |w| w.max_shot_count())
    }
    fn weapon_aim_delta(&self) -> f32 {
        self.objects.find(self.ai_self).and_then(|o| o.weapon_set.get_cur_weapon()).and_then(|w| self.statics.weapon_store.templates().get(w.template())).map_or(0.0, |t| t.aim_delta)
    }
    fn weapon_status_now(&self, slot: WeaponSlotType) -> WeaponStatus {
        let now = crate::common::state_machine::StateContext::frame(self);
        self.objects.find(self.ai_self).and_then(|o| o.weapon_set.get_weapon_in_weapon_slot(slot as i32)).map_or(WeaponStatus::ReadyToFire, |w| w.get_status(now))
    }
    fn weapon_has_leech_range_now(&self, slot: WeaponSlotType) -> bool {
        self.objects.find(self.ai_self).and_then(|o| o.weapon_set.get_weapon_in_weapon_slot(slot as i32)).is_some_and(|w| w.has_leech_range())
    }
    fn weapon_fire_at(&mut self, slot: WeaponSlotType, pos: &Coord3D) -> bool {
        self.ai_weapon_fire_at(self.ai_self, slot, pos)
    }
    fn weapon_set_pre_attack_finished(&mut self, slot: WeaponSlotType, frame: u32) {
        self.ai_weapon_set_pre_attack_finished(self.ai_self, slot, frame);
    }
    fn set_firing_condition_for_current_weapon(&mut self) {
        let o = self.ai_self;
        crate::logic::world::object_weapons::set_firing_condition_for_current_weapon(self, o);
    }
    fn pre_fire_current_weapon(&mut self, victim: Option<ObjectId>) {
        self.ai_pre_fire(self.ai_self, victim);
    }
    fn fire_current_weapon_at_object(&mut self, victim: Option<ObjectId>) {
        self.ai_fire(self.ai_self, crate::logic::world::object_weapons::FireAt::Object(victim));
    }
    fn fire_current_weapon_at_position(&mut self, pos: &Coord3D) {
        self.ai_fire(self.ai_self, crate::logic::world::object_weapons::FireAt::Position(Some(*pos)));
    }
    fn release_weapon_lock(&mut self, lock: i32) {
        let o = self.ai_self;
        crate::logic::world::object_weapons::release_weapon_lock(self, o, lock);
    }
    fn units_should_hunt(&self) -> Option<bool> {
        let p = self.get_controlling_player(self.ai_self)?;
        self.players.get_nth(p).map(|x| x.units_should_hunt)
    }
    fn attack_priority(&mut self, info: usize, obj: ObjectId) -> i32 {
        use crate::logic::ai::ai_system::AiQueryHost;
        let template = AiQueryHost::template_of(self, obj);
        AiQueryHost::attack_priority(self, i32::try_from(info).unwrap_or(0), template)
    }
    fn contain_wants_to_enter_or_exit(&mut self, container: ObjectId, wants: i32) {
        let us = self.ai_self;
        self.ai_contain_call(container, MC::OnObjectWantsToEnterOrExit { obj: us, wants });
    }
    fn contain_has_exit_interface(&mut self, container: ObjectId) -> bool {
        self.ai_contain_call(container, MC::HasContainExitInterface).is_some_and(|r| r.as_bool())
    }
    fn exit_is_busy(&mut self, container: ObjectId) -> bool {
        self.ai_contain_call(container, MC::IsExitBusy).is_some_and(|r| r.as_bool())
    }
    fn exit_reserve_door(&mut self, container: ObjectId) -> i32 {
        let us = self.ai_self;
        let template = self.find_object(us).map(|o| o.template().get_template_id());
        self.ai_contain_call(container, MC::ReserveDoorForExit { template, specific: us }).map_or(DOOR_NONE_AVAILABLE, |r| r.as_int(DOOR_NONE_AVAILABLE))
    }
    fn exit_object_via_door(&mut self, container: ObjectId, door: i32) {
        let us = self.ai_self;
        self.ai_contain_call(container, MC::ExitObjectViaDoor { obj: us, door });
    }
    fn object_ai_free_to_exit(&mut self, id: ObjectId) -> Option<i32> {
        let us = self.ai_self;
        self.ai_ask(id, MC::AiGetFreeToExit { exiter: us }).map(|r| r.as_int(0))
    }
    fn object_is_above_terrain(&mut self, id: ObjectId) -> bool {
        self.get_height_above_terrain(id) > 0.0
    }
    fn action_can_attack_object(&mut self, target: ObjectId, src: CommandSourceType) -> CanAttackResultValue {
        self.ai_action_can_attack(self.ai_self, target, src)
    }
    fn object_geometry_radii(&mut self, id: ObjectId) -> (f32, f32, bool) {
        self.find_object(id).map_or((0.0, 0.0, false), |o| (o.geometry_info.get_major_radius(), o.geometry_info.get_minor_radius(), o.geometry_info.get_geom_type() == crate::common::geometry::GeometryType::Sphere))
    }
    fn contain_add_to_contain(&mut self, container: ObjectId) {
        let us = self.ai_self;
        self.ai_contain_call(container, MC::AddToContain(us));
    }
    fn contain_items(&mut self, container: ObjectId) -> Vec<ObjectId> {
        self.ai_contained_items(container)
    }
    fn contain_remove_from_contain(&mut self, container: ObjectId, obj: ObjectId) {
        self.ai_contain_call(container, MC::RemoveFromContain(obj));
    }
    fn contain_is_valid_container_for_me(&mut self, container: ObjectId) -> bool {
        let us = self.ai_self;
        self.ai_contain_call(container, MC::IsValidContainerFor { obj: us, check_capacity: true }).is_some_and(|r| r.as_bool())
    }
    fn score_the_kill(&mut self, _victim: ObjectId) {
        self.ais_cut("score_the_kill");
    }
    fn object_kill(&mut self, id: ObjectId) {
        self.kill(id, crate::logic::damage::DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
    }
    fn kill_self(&mut self) {
        let id = self.ai_self;
        self.kill(id, crate::logic::damage::DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
    }
    fn do_fx_combat_drop_kill(&mut self, _bldg: ObjectId) {
        self.ais_cut("do_fx_combat_drop_kill");
    }
    fn physics_reset_dynamic(&mut self) {
        self.ais_cut("physics_reset_dynamic");
    }
    fn physics_scrub_velocity_z(&mut self, _rate: f32) {
        self.ais_cut("physics_scrub_velocity_z");
    }
    fn object_set_layer(&mut self, layer: PathfindLayer) {
        let id = self.ai_self;
        World::set_layer(self, id, layer);
    }
    fn object_orientation(&mut self, id: ObjectId) -> f32 {
        self.find_object(id).map_or(0.0, |o| o.thing.orientation())
    }
    fn object_position(&mut self, id: ObjectId) -> Coord3D {
        self.find_object(id).map_or_else(Coord3D::default, |o| o.get_position())
    }
    fn object_bounding_circle_radius(&mut self, id: ObjectId) -> f32 {
        self.find_object(id).map_or(0.0, |o| o.geometry_info.get_bounding_circle_radius())
    }
    fn logic_random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.random.game_logic_random_value_real(lo, hi)
    }
    fn find_position_around_no_source(&mut self, pos: &Coord3D, min_radius: f32, max_radius: f32) -> Option<Coord3D> {
        let options = FindPositionOptions { min_radius, max_radius, ..FindPositionOptions::default() };
        self.ai_find_position_around("find_position_around_no_source", pos, options)
    }
    fn player_difficulty(&mut self) -> i32 {
        use crate::logic::world::team_ops::TeamHost;
        let p = self.get_controlling_player(self.ai_self).unwrap_or(0);
        TeamHost::player_difficulty(self, p) as i32
    }
    fn choose_victim_always_uses_normal(&mut self) -> bool {
        self.ais_cut("choose_victim_always_uses_normal");
        Default::default()
    }
    fn object_is_selectable(&mut self, id: ObjectId) -> bool {
        self.is_selectable(id)
    }
    fn closest_object_on_squad(&mut self, _squad: &[ObjectId]) -> ObjectId {
        self.ais_cut("closest_object_on_squad");
        INVALID_ID
    }
    fn object_has_dock(&mut self, id: ObjectId) -> bool {
        self.get_dock_update_interface(id).is_some()
    }
    fn dock_is_open(&mut self, dock: ObjectId) -> bool {
        self.ai_dock_call(dock, MC::DockIsOpen).is_some_and(|r| r.as_bool())
    }
    fn dock_cancel(&mut self, dock: ObjectId) {
        let us = self.ai_self;
        self.ai_dock_call(dock, MC::DockCancelDock { docker: us });
    }
    fn dock_reserve_approach(&mut self, dock: ObjectId, advance: bool) -> Option<(Coord3D, i32)> {
        let us = self.ai_self;
        // (the docking machine's `m_approachPosition`, which `advanceApproachPosition` reads and both write)
        let index = self.ai_stacks.attack.last().map_or(0, |f| f.dock_approach);
        let position = Some(Coord3D::default());
        let call = if advance { MC::DockAdvanceApproachPosition { docker: us, position, index } } else { MC::DockReserveApproachPosition { docker: us, position, index } };
        match self.ai_dock_call(dock, call) {
            Some(ModuleReply::Approach { ok: true, position, index }) => Some((position.unwrap_or_default(), index)),
            _ => None,
        }
    }
    fn dock_is_clear_to_enter(&mut self, dock: ObjectId) -> bool {
        let us = self.ai_self;
        self.ai_dock_call(dock, MC::DockIsClearToEnter { docker: us }).is_some_and(|r| r.as_bool())
    }
    fn dock_is_clear_to_advance(&mut self, dock: ObjectId, index: i32) -> bool {
        let us = self.ai_self;
        self.ai_dock_call(dock, MC::DockIsClearToAdvance { docker: us, index }).is_some_and(|r| r.as_bool())
    }
    fn dock_is_allow_passthrough(&mut self, dock: ObjectId) -> bool {
        self.ai_dock_call(dock, MC::DockIsAllowPassthroughType).is_some_and(|r| r.as_bool())
    }
    fn dock_is_rally_point_after_dock(&mut self, dock: ObjectId) -> bool {
        self.ai_dock_call(dock, MC::DockIsRallyPointAfterDockType).is_some_and(|r| r.as_bool())
    }
    fn dock_position(&mut self, dock: ObjectId, which: u32) -> Coord3D {
        use crate::logic::ai::ai_dock::{DOCK_DOCK, DOCK_ENTER};
        let us = self.ai_self;
        let call = match which {
            DOCK_ENTER => MC::DockGetEnterPosition { docker: us, position: true },
            DOCK_DOCK => MC::DockGetDockPosition { docker: us, position: true },
            _ => MC::DockGetExitPosition { docker: us, position: true },
        };
        self.ai_dock_call(dock, call).map(|r| r.as_coord()).unwrap_or_default()
    }
    fn dock_reached(&mut self, dock: ObjectId, which: u32) {
        use crate::logic::ai::ai_dock::{REACHED_APPROACH, REACHED_DOCK, REACHED_ENTER};
        let us = self.ai_self;
        let call = match which {
            REACHED_APPROACH => MC::DockOnApproachReached { docker: us },
            REACHED_ENTER => MC::DockOnEnterReached { docker: us },
            REACHED_DOCK => MC::DockOnDockReached { docker: us },
            _ => MC::DockOnExitReached { docker: us },
        };
        self.ai_dock_call(dock, call);
    }
    fn dock_action(&mut self, dock: ObjectId, drone: ObjectId) -> bool {
        let us = self.ai_self;
        self.ai_dock_call(dock, MC::DockAction { docker: us, drone }).is_some_and(|r| r.as_bool())
    }
    fn object_rally_point(&mut self, _id: ObjectId) -> Option<Coord3D> {
        self.ais_cut("object_rally_point");
        Default::default()
    }
    fn supply_truck_action_delay(&mut self, _dock: ObjectId) -> Option<u32> {
        // (the docking states ask the host only for an AI that is not a supply truck, a worker or a chinook, whose own delays they
        // read themselves: upstream's `getSupplyTruckAIInterface()` is `NULL` then, and the delay none)
        None
    }
    fn player_object_ids(&mut self) -> Vec<ObjectId> {
        match self.get_controlling_player(self.ai_self) {
            Some(p) => self.player_iterate_objects(p),
            None => Vec::new(),
        }
    }
    fn clearable_last_attacker(&mut self) -> ObjectId {
        self.ai_clearable_last_attacker(self.ai_self)
    }
    fn clear_last_attacker(&mut self) {
        self.ai_clear_last_attacker(self.ai_self);
    }
    fn template_is_enter_guard(&mut self) -> bool {
        self.ais_cut("template_is_enter_guard");
        Default::default()
    }
    fn template_is_hijack_guard(&mut self) -> bool {
        self.ais_cut("template_is_hijack_guard");
        Default::default()
    }
    fn frame_objects_changed_trigger_areas(&mut self) -> u32 {
        self.ais_cut("frame_objects_changed_trigger_areas");
        Default::default()
    }
    fn ai_data_guard_enemy_scan_rate(&mut self) -> u32 {
        self.statics.ai_data.data.guard_enemy_scan_rate
    }
    fn ai_data_guard_enemy_return_scan_rate(&mut self) -> u32 {
        self.statics.ai_data.data.guard_enemy_return_scan_rate
    }
    fn ai_data_guard_chase_unit_frames(&mut self) -> u32 {
        self.statics.ai_data.data.guard_chase_unit_frames
    }
    fn polygon_radius(&mut self, _polygon: i32) -> f32 {
        self.ais_cut("polygon_radius");
        0.0
    }
    fn closest_guard_target(&mut self, _pos: &Coord3D, _range: f32, _filters: u32, _area: Option<i32>) -> ObjectId {
        self.ais_cut("closest_guard_target");
        INVALID_ID
    }
    fn has_team(&mut self) -> bool {
        true
    }
    fn tunnel_system(&mut self) -> bool {
        self.ais_cut("tunnel_system");
        Default::default()
    }
    fn tunnel_container_list(&mut self) -> Vec<ObjectId> {
        self.ais_cut("tunnel_container_list");
        Default::default()
    }
    fn tunnel_cur_nemesis(&mut self) -> ObjectId {
        self.ais_cut("tunnel_cur_nemesis");
        INVALID_ID
    }
    fn tunnel_update_nemesis(&mut self, _target: ObjectId) {
        self.ais_cut("tunnel_update_nemesis");
    }
    fn object_ai_goal_object(&mut self, id: ObjectId) -> Option<ObjectId> {
        self.ai_machine_query(id).map(|m| m.goal_object_id())
    }
    fn object_last_damage(&mut self, id: ObjectId) -> Option<(bool, ObjectId, u32)> {
        self.ai_object_last_damage(id)
    }
    fn exit_object_in_a_hurry(&mut self, container: ObjectId) {
        let us = self.ai_self;
        self.ai_contain_call(container, MC::ExitObjectInAHurry(us));
    }
    fn update_module_xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self.ai_update_base.as_mut() {
            Some(u) => u.xfer(xfer),
            None => Ok(()),
        }
    }
    fn update_module_crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self.ai_update_base.as_mut() {
            Some(u) => u.crc(xfer),
            None => Ok(()),
        }
    }
    fn update_module_load_post_process(&mut self) {
        if let Some(u) = self.ai_update_base.as_mut() {
            u.load_post_process();
        }
    }
    fn polygon_trigger_name(&mut self, _polygon: i32) -> Vec<u8> {
        self.ais_cut("polygon_trigger_name");
        Default::default()
    }
    fn polygon_trigger_by_name(&mut self, name: &[u8]) -> Option<i32> {
        self.terrain.trigger_area_by_name(&AsciiString::try_from_bytes(name).ok()?).map(|i| i as i32)
    }
    fn attack_info_name(&mut self, _info: usize) -> Vec<u8> {
        self.ais_cut("attack_info_name");
        Default::default()
    }
    fn attack_info_by_name(&mut self, _name: &[u8]) -> Option<usize> {
        self.ais_cut("attack_info_by_name");
        Default::default()
    }
    fn waypoint_by_id(&mut self, id: u32) -> Option<u32> {
        self.terrain.waypoint_by_id(id).map(|_| id)
    }
    fn locomotor_set_xfer(&mut self, _set: &mut LocomotorSet, _cur: &mut Option<usize>, _xfer: &mut dyn Xfer) -> Result<(), LocomotorSetError> {
        self.ais_cut("locomotor_set_xfer");
        Ok(())
    }
    fn turret_ai_snapshot(&mut self, which: usize, xfer: &mut dyn Xfer) -> XferResult<()> {
        let id = self.ai_self;
        self.with_turret(id, which, |t, _| t.xfer(xfer)).unwrap_or(Ok(()))
    }
    fn pf_update_pos(&mut self, pos: &Coord3D) {
        let id = self.ai_self;
        self.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::UpdatePos(id, *pos));
    }
    fn own_team_member_ids(&mut self) -> Vec<ObjectId> {
        let t = self.find_object(self.ai_self).map(|o| o.team);
        t.map(|t| self.teams.members(t)).unwrap_or_default()
    }
    fn object_ai_is_idle(&mut self, id: ObjectId) -> bool {
        self.ai_other_is_idle(id)
    }
    fn object_ai_goal_position(&mut self, id: ObjectId) -> Coord3D {
        self.ai_machine_query(id).map(|m| *m.goal_position()).unwrap_or_default()
    }
    fn turret_target_object(&mut self, which: usize, clear_dead_targets: bool) -> Option<ObjectId> {
        let id = self.ai_self;
        self.with_turret(id, which, |t, w| t.friend_get_turret_target(w, clear_dead_targets))
            .and_then(|(kind, obj, _)| if kind == crate::logic::ai::turret::TurretTargetType::Object { obj } else { None })
    }
    fn turret_set_enabled(&mut self, which: usize, enabled: bool) {
        let id = self.ai_self;
        self.with_turret(id, which, |t, w| t.set_turret_enabled(w, enabled));
    }
    fn turret_recenter(&mut self, which: usize) {
        let id = self.ai_self;
        if let Some(Err(e)) = self.with_turret(id, which, |t, w| t.recenter_turret(w)) {
            self.cuts.team_live_errors.push(format!("TurretAI({which})::recenterTurret: {e:?}"));
        }
    }
    fn turret_is_enabled(&mut self, which: usize) -> bool {
        self.turret_peek(self.ai_self, which).is_some_and(|d| d.enabled)
    }
    fn turret_is_in_natural_position(&mut self, which: usize) -> bool {
        let id = self.ai_self;
        self.with_turret(id, which, |t, w| t.is_turret_in_natural_position(w)).unwrap_or(false)
    }
    fn turret_is_weapon_slot_on(&mut self, which: usize, wslot: WeaponSlotType) -> bool {
        self.turret_peek(self.ai_self, which).is_some_and(|d| crate::logic::ai::turret::is_weapon_slot_on_turret(d, wslot))
    }
    fn turret_is_trying_to_aim_at(&mut self, which: usize, victim: ObjectId) -> bool {
        let id = self.ai_self;
        self.with_turret(id, which, |t, w| t.is_trying_to_aim_at_target(w, victim)).unwrap_or(false)
    }
    fn turret_angle(&mut self, which: usize) -> f32 {
        self.turret_peek(self.ai_self, which).map_or(0.0, |d| d.angle)
    }
    fn turret_pitch(&mut self, which: usize) -> f32 {
        self.turret_peek(self.ai_self, which).map_or(0.0, |d| d.pitch)
    }
    fn object_producer_id(&mut self, id: ObjectId) -> ObjectId {
        self.find_object(id).map_or(INVALID_ID, |o| o.producer_id)
    }
    fn notify_firing_tracker_shot_fired(&mut self, slot: WeaponSlotType, victim: ObjectId) {
        let o = self.ai_self;
        crate::logic::world::object_weapons::notify_firing_tracker_shot_fired(self, o, slot as i32, victim);
    }
    fn weapon_is_too_close(&mut self, victim: ObjectId) -> bool {
        self.ai_weapon_too_close(self.ai_self, victim)
    }
    fn object_forward_speed_2d(&mut self, id: ObjectId) -> f32 {
        let Some(ph) = self.find_object(id).and_then(|o| o.physics) else { return 0.0 };
        self.module_call(id, ph, crate::logic::world::module_call::ModuleCall::PhysicsGetForwardSpeed2D).as_real(0.0)
    }
    fn object_unit_direction_vector_2d(&mut self, id: ObjectId) -> Coord3D {
        self.find_object_mut(id).map_or_else(Coord3D::default, |o| o.thing.unit_direction_vector_2d())
    }
    fn object_center_position(&mut self, id: ObjectId, pos: &Coord3D) -> Coord3D {
        let _ = pos;
        self.find_object(id).map_or_else(Coord3D::default, |o| o.get_position())
    }
    fn object_template_is_kind_of(&self, id: ObjectId, kind: KindOfType) -> bool {
        self.find_object(id).is_some_and(|o| o.template().is_kind_of(kind))
    }
    fn untranslated_on_enter(&mut self, _id: StateId) -> StateReturn {
        self.ais_cut("untranslated_on_enter");
        StateReturn::CONTINUE
    }
    fn untranslated_update(&mut self, _id: StateId) -> StateReturn {
        self.ais_cut("untranslated_update");
        StateReturn::CONTINUE
    }
    fn untranslated_on_exit(&mut self, _id: StateId, _status: StateExitType) {
        self.ais_cut("untranslated_on_exit");
    }
}

#[cfg(test)]
mod tests {
    //! The states themselves are checked against the compiled AIStates.cpp by `tests/differential_ai_update.rs` with a
    //! stand-in host; this runs the AI's machine on the world.
    use super::*;
    use crate::common::object_status_types::ObjectStatusMaskType;
    use crate::logic::ai::ai_commands::{AiCommandParms, AICMD_IDLE};
    use crate::logic::ai::ai_states::AiUpdateModule;
    use crate::logic::ai::ai_update_data::AiUpdateModuleData;
    use crate::logic::ai::ai_update_types::{AI_IDLE, CMD_FROM_PLAYER};
    use crate::logic::world::player_host_world::tests::world;
    use std::sync::Arc;

    /// A tank's `AIUpdateInterface` made on the world (`ai_self` the tank): `onObjectCreated` makes its machine, which
    /// starts idle; an update runs it and an idle command keeps it idle. The AI's state and machine are lent to the world
    /// while they run and taken back after (the world's stacks are empty again), and no live error is recorded.
    #[test]
    fn the_ais_machine_runs_on_the_world() {
        let (mut w, _) = world();
        let t = World::thing_factory_find_template(&w, b"Tank").unwrap();
        let tank = World::thing_factory_new_object(&mut w, t, None, ObjectStatusMaskType::default()).unwrap();
        w.with_ai_self(tank, |w| {
            let mut m = AiUpdateModule::<World>::new(w, Arc::new(AiUpdateModuleData::default()));
            m.on_object_created(w).unwrap();
            assert_eq!(m.machine.as_ref().unwrap().get_current_state_id(), AI_IDLE);
            m.update(w).unwrap();
            m.ai_do_command(w, &AiCommandParms::new(AICMD_IDLE, CMD_FROM_PLAYER)).unwrap();
            m.update(w).unwrap();
            assert_eq!(m.machine.as_ref().unwrap().get_current_state_id(), AI_IDLE);
            assert!(m.ai.is_some());
        });
        assert!(w.ai_stacks.ai.is_empty() && w.ai_stacks.machine.is_empty() && w.ai_stacks.attack.is_empty());
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
