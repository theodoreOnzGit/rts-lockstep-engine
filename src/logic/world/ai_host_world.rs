// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp, Locomotor.cpp, AI/AIPathfind.cpp (`Path`):
//   the calls `AIUpdateInterface`, `Locomotor` and `Path` make out of themselves, answered by the world
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I9b-1: the world as an AI's host: `LocomotorGlobals`, `LocomotorHost`, `PathHost` and `AiHost`.
//!
//! These hosts are an object's view ("this" object: its position, its physics, its body): the world answers them for
//! [`World::ai_self`], the object whose AI is running, set by [`World::with_ai_self`]. The AI update module itself joins the
//! world in batch I9b-3; until then a caller (a test) runs AI code on the world through `with_ai_self`.
//!
//! - **The world:** the object (position, orientation, transform, layer, kinds, status, geometry, model conditions,
//!   containment, effective death, its height above the terrain), its body's damage state and last damage, its
//!   `PhysicsBehavior` (through module calls: the mass counts the contained items'), the terrain (ground, layer and
//!   unclipped layer heights, water, the highest layer for a destination), the partition manager's ground or structure
//!   height (`live_partition`), the pathfinder (I9a: surfaces, movement positions and lines, cells, `updateLayer`; only
//!   when it answers, `World::pf_ready`: else every position is valid, every surface there, no cell), the
//!   locomotor store (`statics.locomotor_store`), the AI data (`statics.ai_data`), the team's common target, the weapons'
//!   `getAbleToAttackSpecificObject`, the player's upgrades and type, the weapon bonus conditions, the logic's random
//!   values and the AI module's wake frame.
//! - **Cut points** ([`AiHostCuts`]): the targeters an AI keeps (`addTargeter` on the target's AI, batch I9b-3), its
//!   group's speed (batch I9c), `TheAI->findClosestEnemy` and the adjusted vision range (`TheAI`, batch I9c), the stealth
//!   granted by a special power, a container's `isPassengerAllowedToFire`, and the horde updates.
//!
//! **Signature changes:** `LocomotorHost`'s physics getters (`is_stunned`, `velocity`, `velocity_magnitude`,
//! `forward_speed_2d`, `forward_speed_3d`, `mass`, `is_motive`) take `&mut self`: the world answers them through the
//! physics module's calls, and `getMass` asks the contain module for the contained items' mass. `AiHost::team_target_object`
//! takes `&mut self`: `Team::getTeamTargetObject` forgets a target that is no longer fair. The locomotor's
//! `calcDirectionToApplyThrust` and `calcLiftToUseAtPt` take the host as `&mut H`; the differential hosts follow.
//!
//! `p1w13-sonnet`, integration batch I9b-1, 2026-11-01.

use std::collections::BTreeMap;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{DisabledType, DISABLED_HELD};
use crate::common::game_common::PlayerType;
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID, LAYER_GROUND};
use crate::common::kind_of::KindOfType;
use crate::common::model_state::ModelConditionFlagType;
use crate::common::name_key_generator::NameKeyType;
use crate::common::object_status_types::ObjectStatusTypes;
use crate::logic::ai::ai_update::AiHost;
use crate::logic::ai::ai_update_types::{CanAttackResultValue, ATTACKRESULT_INVALID_SHOT, ATTACKRESULT_NOT_POSSIBLE, ATTACKRESULT_POSSIBLE, ATTACKRESULT_POSSIBLE_AFTER_MOVING};
use crate::logic::ai::path::PathHost;
use crate::logic::damage::DamageType;
use crate::logic::locomotor::{Locomotor, LocomotorGlobals, LocomotorHost, LocomotorSet, LocomotorSurfaceTypeMask, PhysicsTurningType};
use crate::logic::module::body_module::BodyDamageType;
use crate::logic::module::update_module::{UpdateModule, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::terrain_logic::Underwater;
use crate::common::game_common::AbleToAttackType;
use crate::logic::ai::ai_update_types::CommandSourceType;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::weapon_host::CanAttackResult as WeaponCanAttackResult;
use crate::ww::matrix3d::Matrix3D;

/// The AI host's answers from the units the world does not hold yet (see the module docs).
#[derive(Clone, Debug, Default)]
pub struct AiHostCuts {
    /// `TheAI->findClosestEnemy(obj, range, qualifiers, info, area)`, by object (none listed: `NULL`).
    pub closest_enemy: BTreeMap<ObjectId, ObjectId>,
    /// `TheAI->getAdjustedVisionRangeForObject(obj, factors)`, by object (none listed: the object's vision range).
    pub adjusted_vision_range: BTreeMap<ObjectId, f32>,
    /// The stealth update's `isGrantedBySpecialPower()` (none listed: no stealth update).
    pub stealth_granted_by_special_power: BTreeMap<ObjectId, bool>,
    /// The container's `isPassengerAllowedToFire()`, by container (none listed: false).
    pub passenger_allowed_to_fire: BTreeMap<ObjectId, bool>,
    /// The object's horde updates (`isTrueHordeMember()`, `isAllowedNationalism()`), by object.
    pub horde_updates: BTreeMap<ObjectId, Vec<(bool, bool)>>,
    /// The FX lists the AIs' data name (batch I9b-5), by the index an AI keeps for one: the FX is the client's, and its cut
    /// points log the name.
    pub fx_names: Vec<Vec<u8>>,
    /// Batch I9c-3: the largest weapon range of the object whose adjusted vision range is being computed (see
    /// `world::ai_query_world`).
    pub largest_weapon_range: Option<(ObjectId, f32)>,
}

impl World {
    /// The index an AI keeps for the FX list `name` (`None` for `NULL`): its place in `cuts.ai_host.fx_names`, added once.
    pub fn ai_fx_handle(&mut self, name: &crate::common::damage_fx::FxListName) -> Option<u32> {
        let name = name.as_ref()?;
        let names = &mut self.cuts.ai_host.fx_names;
        let i = names.iter().position(|n| n == name).unwrap_or_else(|| {
            names.push(name.clone());
            names.len() - 1
        });
        Some(i as u32)
    }

    /// Runs `f` with `id` as the object the AI hosts answer for ([`World::ai_self`]), then puts the previous one back.
    pub fn with_ai_self<T>(&mut self, id: ObjectId, f: impl FnOnce(&mut World) -> T) -> T {
        let prev = std::mem::replace(&mut self.ai_self, id);
        let r = f(self);
        self.ai_self = prev;
        r
    }

    /// The physics module of `this` object, if it has one.
    fn ai_physics(&self) -> Option<crate::logic::world::object::ModuleIndex> {
        self.objects.find(self.ai_self).and_then(|o| o.physics)
    }

    /// A call to `this` object's physics module (`None` without one).
    pub(crate) fn ai_physics_call(&mut self, call: ModuleCall) -> Option<crate::logic::world::module_call::ModuleReply> {
        let p = self.ai_physics()?;
        Some(self.module_call(self.ai_self, p, call))
    }

    fn ai_host_cut(&mut self, call: &'static str, args: Vec<i64>) {
        self.cut(CutCall::Host { call, args, text: Vec::new() });
    }
}

impl LocomotorGlobals for World {
    fn gravity(&self) -> f32 {
        self.object_globals().gravity
    }
    fn movement_penalty_damage_state(&self) -> BodyDamageType {
        self.statics.global_data.the_global_data().map_or(crate::logic::module::body_module::BODY_REALLYDAMAGED, |g| g.movement_penalty_damage_state)
    }
}

impl LocomotorHost for World {
    fn has_physics(&self) -> bool {
        self.ai_physics().is_some()
    }
    fn position(&self) -> Coord3D {
        self.objects.find(self.ai_self).map_or_else(Coord3D::default, |o| o.get_position())
    }
    fn set_position(&mut self, pos: &Coord3D) {
        World::set_position(self, self.ai_self, pos);
    }
    fn orientation(&self) -> f32 {
        self.objects.find(self.ai_self).map_or(0.0, |o| o.thing.orientation())
    }
    fn set_orientation(&mut self, angle: f32) {
        World::set_orientation(self, self.ai_self, angle);
    }
    fn unit_direction_vector_2d(&mut self) -> Coord3D {
        let id = self.ai_self;
        self.objects.find_mut(id).map_or_else(Coord3D::default, |o| o.thing.unit_direction_vector_2d())
    }
    fn transform_matrix(&self) -> Matrix3D {
        self.objects.find(self.ai_self).map_or_else(Matrix3D::identity, |o| *o.thing.transform_matrix())
    }
    fn set_transform_matrix(&mut self, mtx: &Matrix3D) {
        World::set_transform_matrix(self, self.ai_self, mtx);
    }
    fn layer(&self) -> PathfindLayer {
        self.objects.find(self.ai_self).map_or(LAYER_GROUND, |o| o.layer)
    }
    fn is_kind_of(&self, kind: KindOfType) -> bool {
        self.objects.find(self.ai_self).is_some_and(|o| o.is_kind_of(kind))
    }
    fn status_test(&self, status: ObjectStatusTypes) -> bool {
        self.objects.find(self.ai_self).is_some_and(|o| o.test_status(status))
    }
    fn set_status(&mut self, status: ObjectStatusTypes, set: bool) {
        World::set_status(self, self.ai_self, crate::logic::world::object_state::mask_of(status), set);
    }
    fn carrier_deck_height(&self) -> f32 {
        // `getCarrierDeckHeight()` asks the containing flight deck by a module call; the world answers it by `&mut` (the
        // module call), so here the containing object's height alone (a flight deck's own answer is batch I9b-3's)
        0.0
    }
    fn major_radius(&self) -> f32 {
        self.objects.find(self.ai_self).map_or(0.0, |o| o.geometry_info.get_major_radius())
    }
    fn bounding_circle_radius(&self) -> f32 {
        self.objects.find(self.ai_self).map_or(0.0, |o| o.geometry_info.get_bounding_circle_radius())
    }
    fn is_disabled_held(&self) -> bool {
        self.objects.find(self.ai_self).is_some_and(|o| o.is_disabled_by_type(DISABLED_HELD))
    }
    fn set_model_condition_state(&mut self, condition: ModelConditionFlagType) {
        World::set_model_condition_state(self, self.ai_self, condition);
    }
    fn clear_model_condition_state(&mut self, condition: ModelConditionFlagType) {
        World::clear_model_condition_state(self, self.ai_self, condition);
    }
    fn is_above_terrain(&mut self) -> bool {
        self.get_height_above_terrain(self.ai_self) > 0.0
    }
    fn damage_state(&self) -> BodyDamageType {
        self.peek_damage_state(self.ai_self)
    }
    fn is_stunned(&mut self) -> bool {
        self.ai_physics_call(ModuleCall::PhysicsGetIsStunned).is_some_and(|r| r.as_bool())
    }
    fn apply_motive_force(&mut self, force: &Coord3D) {
        self.ai_physics_call(ModuleCall::PhysicsApplyMotiveForce(*force));
    }
    fn scrub_velocity_2d(&mut self, desired_speed: f32) {
        self.ai_physics_call(ModuleCall::PhysicsScrubVelocity2D(desired_speed));
    }
    fn set_turning(&mut self, turning: PhysicsTurningType) {
        self.ai_physics_call(ModuleCall::PhysicsSetTurning(turning as i32));
    }
    fn velocity(&mut self) -> Coord3D {
        self.ai_physics_call(ModuleCall::PhysicsGetVelocity).map_or_else(Coord3D::default, |r| r.as_coord())
    }
    fn velocity_magnitude(&mut self) -> f32 {
        self.ai_physics_call(ModuleCall::PhysicsGetVelocityMagnitude).map_or(0.0, |r| r.as_real(0.0))
    }
    fn forward_speed_2d(&mut self) -> f32 {
        self.ai_physics_call(ModuleCall::PhysicsGetForwardSpeed2D).map_or(0.0, |r| r.as_real(0.0))
    }
    fn forward_speed_3d(&mut self) -> f32 {
        self.ai_physics_call(ModuleCall::PhysicsGetForwardSpeed3D).map_or(0.0, |r| r.as_real(0.0))
    }
    fn mass(&mut self) -> f32 {
        self.ai_physics_call(ModuleCall::PhysicsGetMass).map_or(0.0, |r| r.as_real(0.0))
    }
    fn is_motive(&mut self) -> bool {
        self.ai_physics_call(ModuleCall::PhysicsIsMotive).is_some_and(|r| r.as_bool())
    }
    fn set_extra_friction(&mut self, friction: f32) {
        self.ai_physics_call(ModuleCall::PhysicsSetExtraFriction(friction));
    }
    fn set_allow_airborne_friction(&mut self, allow: bool) {
        self.ai_physics_call(ModuleCall::PhysicsSetAllowAirborneFriction(allow));
    }
    fn set_stick_to_ground(&mut self, stick: bool) {
        self.ai_physics_call(ModuleCall::PhysicsSetStickToGround(stick));
    }
    fn frame(&self) -> u32 {
        World::frame(self)
    }
    fn ground_height(&mut self, x: f32, y: f32) -> f32 {
        self.terrain.ground_height(x, y, false).0
    }
    fn layer_height(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        crate::logic::world::object_transform::layer_height(&self.terrain, self.wall_height(), &self.pf_wall, x, y, layer)
    }
    /// `getLayerHeight(x, y, layer, NULL, false)`; a vertical deck (an undefined input) answers the ground height.
    fn layer_height_unclipped(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        let (wall_height, wall) = (self.wall_height(), &self.pf_wall);
        match self.terrain.layer_height_with(|| wall_height, |p| wall.is_point_on_wall(p), x, y, layer, false, false) {
            Ok((h, _)) => h,
            Err(_) => self.terrain.ground_height(x, y, false).0,
        }
    }
    fn underwater(&mut self, x: f32, y: f32) -> Underwater {
        match self.terrain.is_underwater(x, y) {
            Ok(u) => u,
            Err(_) => Underwater { underwater: false, water_z: None, terrain_z: self.terrain.ground_height(x, y, false).0 },
        }
    }
    fn highest_layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        self.terrain_get_highest_layer_for_destination(pos)
    }
    fn ground_or_structure_height(&mut self, x: f32, y: f32) -> f32 {
        if !self.cuts.live_partition {
            return self.terrain.ground_height(x, y, false).0;
        }
        self.with_partition(|pm, w| pm.get_ground_or_structure_height(w, x, y))
    }
    /// `TheAI->pathfinder()->validMovementTerrain(layer, loco, pos)`: true without the pathfinder (as a stand-in host's).
    fn valid_movement_terrain(&mut self, layer: PathfindLayer, loco: &Locomotor, pos: &Coord3D) -> bool {
        let Some(pf) = self.pf_ready() else { return true };
        pf.valid_movement_terrain(layer, loco.get_legal_surfaces(), pos)
    }
}

impl PathHost for World {
    fn is_line_passable(&mut self, obj: ObjectId, surfaces: LocomotorSurfaceTypeMask, layer: PathfindLayer, start: &Coord3D, end: &Coord3D, blocked: bool, allow_pinched: bool) -> bool {
        let facts = self.pf_object_facts(obj);
        let Some(pf) = self.pf_ready() else { return true };
        pf.is_line_passable(self, facts.as_ref(), surfaces, layer, start, end, blocked, allow_pinched)
    }
    fn is_ground_path_passable(&mut self, crusher: bool, start: &Coord3D, layer: PathfindLayer, end: &Coord3D, diameter: i32) -> bool {
        let Some(pf) = self.pf_ready() else { return true };
        pf.is_ground_path_passable(self, crusher, start, layer, end, diameter)
    }
    fn cell_type_and_pinched(&mut self, layer: PathfindLayer, pos: &Coord3D) -> Option<(i32, bool)> {
        let pf = self.pf_ready()?;
        let c = &pf.store.cells[pf.get_cell_at(layer, pos)? as usize];
        Some((i32::from(c.get_type()), c.get_pinched()))
    }
    fn layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        crate::logic::world::build_assistant::BuildAssistantHost::terrain_layer_for_destination(self, *pos)
    }
    fn object_layer(&mut self, obj: ObjectId) -> PathfindLayer {
        self.objects.find(obj).map_or(LAYER_GROUND, |o| o.layer)
    }
}

impl AiHost for World {
    fn object_id(&self) -> ObjectId {
        self.ai_self
    }
    fn is_disabled_by_type(&self, t: DisabledType) -> bool {
        self.objects.find(self.ai_self).is_some_and(|o| o.is_disabled_by_type(t))
    }
    fn crusher_level(&self) -> i32 {
        i32::from(self.get_crusher_level(self.ai_self))
    }
    fn physics_allow_to_fall(&self) -> Option<bool> {
        let p = self.ai_physics()?;
        match self.peek_module_body(ModuleHandle { object: self.ai_self, index: p }) {
            Some(crate::logic::world::update_body::UpdateBody::PhysicsBehavior(b)) => Some(b.get_allow_to_fall()),
            _ => None,
        }
    }
    fn height_above_terrain(&mut self) -> f32 {
        self.get_height_above_terrain(self.ai_self)
    }
    fn set_effectively_dead(&mut self, dead: bool) {
        World::set_effectively_dead(self, self.ai_self, dead);
    }
    /// The AI module's `getWakeFrame()` (`UPDATE_SLEEP_NONE` without an AI module).
    fn wake_frame(&self) -> UpdateSleepTime {
        let Some(o) = self.objects.find(self.ai_self) else { return UPDATE_SLEEP_NONE };
        let Some(u) = o.ai.and_then(|i| o.modules.get(i as usize)).and_then(|m| m.update_index) else { return UPDATE_SLEEP_NONE };
        o.behaviors.get(u as usize).map_or(UPDATE_SLEEP_NONE, |b| b.update.get_wake_frame(World::frame(self)))
    }
    /// The AI module's `setWakeFrame(obj, wakeDelay)`.
    fn set_wake_frame(&mut self, wake_delay: UpdateSleepTime) {
        let id = self.ai_self;
        let Some(index) = self.objects.find(id).and_then(|o| o.ai) else { return };
        let Some(u) = self.update_handle_of_module(ModuleHandle { object: id, index }) else { return };
        let frame = UpdateModule::set_wake_frame(World::frame(self), wake_delay);
        let _ = World::awaken_update_module(self, id, u, frame);
    }
    fn group_recompute_speed(&mut self) {
        // (integration batch I16: `getGroup()->recomputeGroupSpeed()`, `m_dirty = true`, AI.h; it was a cut)
        if let Some(g) = self.find_object(self.ai_self).and_then(|o| o.group) {
            self.with_group(g, |gr, _| gr.dirty = true);
        }
    }
    fn pf_surfaces_for_position(&mut self, layer: PathfindLayer, pos: &Coord3D) -> LocomotorSurfaceTypeMask {
        use crate::logic::locomotor::{LOCOMOTORSURFACE_AIR, LOCOMOTORSURFACE_CLIFF, LOCOMOTORSURFACE_GROUND, LOCOMOTORSURFACE_RUBBLE, LOCOMOTORSURFACE_WATER};
        let all = LOCOMOTORSURFACE_GROUND | LOCOMOTORSURFACE_WATER | LOCOMOTORSURFACE_CLIFF | LOCOMOTORSURFACE_AIR | LOCOMOTORSURFACE_RUBBLE;
        self.pf_ready().map_or(all, |pf| pf.surfaces_for_position(layer, pos))
    }
    fn pf_update_layer(&mut self, layer: PathfindLayer) {
        let id = self.ai_self;
        if self.pf_ready().is_some() {
            self.with_pathfinder(|pf, w| pf.update_layer(w, id, layer));
        }
    }
    fn pf_valid_movement_position(&mut self, is_crusher: bool, layer: PathfindLayer, surfaces: LocomotorSurfaceTypeMask, pos: &Coord3D) -> bool {
        let Some(pf) = self.pf_ready() else { return true };
        pf.valid_movement_position(is_crusher, surfaces, pf.get_cell_at(layer, pos))
    }
    /// `ThePartitionManager->getDistanceSquared(obj, dest, FROM_CENTER_3D)`.
    fn distance_squared_3d(&mut self, dest: &Coord3D) -> f32 {
        let p = LocomotorHost::position(self);
        let (dx, dy, dz) = (p.x - dest.x, p.y - dest.y, p.z - dest.z);
        dx * dx + dy * dy + dz * dz
    }
    fn add_locomotor(&mut self, set: &mut LocomotorSet, template: NameKeyType) {
        let frame = World::frame(self);
        if let Err(e) = set.add_locomotor(&self.statics.locomotor_store, template, &mut self.random, frame) {
            self.cuts.team_live_errors.push(format!("LocomotorSet::addLocomotor: {e:?}"));
        }
    }
    fn object_exists(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some()
    }
    fn ai_add_targeter(&mut self, target: ObjectId, targeter: ObjectId, add: bool) {
        self.ai_add_targeter_live(target, targeter, add);
    }
    fn is_effectively_dead(&self) -> bool {
        self.objects.find(self.ai_self).is_some_and(|o| o.is_effectively_dead())
    }
    fn controlling_player_type(&self) -> Option<PlayerType> {
        let p = self.get_controlling_player(self.ai_self)?;
        self.players.get_nth(p).map(|x| x.player_type)
    }
    fn team_attack_common_target(&self) -> bool {
        let Some(team) = self.objects.find(self.ai_self).map(|o| o.team) else { return false };
        self.teams.team(team).and_then(|t| self.teams.prototype(t.proto)).is_some_and(|p| p.team_template.attack_common_target)
    }
    fn team_target_object(&mut self) -> ObjectId {
        let Some(team) = self.objects.find(self.ai_self).map(|o| o.team) else { return INVALID_ID };
        crate::logic::world::team_ops::team_get_team_target_object(self, team).ok().flatten().unwrap_or(INVALID_ID)
    }
    fn able_to_attack_specific_object(&mut self, t: AbleToAttackType, target: ObjectId, command_source: CommandSourceType) -> CanAttackResultValue {
        let id = self.ai_self;
        match crate::logic::world::object_weapons::get_able_to_attack_specific_object(self, id, t, target, crate::common::game_common::CommandSourceType::from_i32(command_source), -1) {
            Ok(WeaponCanAttackResult::Possible) => ATTACKRESULT_POSSIBLE,
            Ok(WeaponCanAttackResult::PossibleAfterMoving) => ATTACKRESULT_POSSIBLE_AFTER_MOVING,
            Ok(WeaponCanAttackResult::InvalidShot) => ATTACKRESULT_INVALID_SHOT,
            Ok(WeaponCanAttackResult::NotPossible) | Err(_) => ATTACKRESULT_NOT_POSSIBLE,
        }
    }
    fn stealth_granted_by_special_power(&self) -> Option<bool> {
        // ~~the table~~ (batch I58: with `live_ai`, the object's stealth update, `getStealth()`: its first `StealthUpdate`; it was always
        // the table, so a unit stealthed by a special power never left its stealth when it attacked)
        if self.cuts.live_ai {
            let o = self.objects.find(self.ai_self)?;
            return (0..o.modules.len()).find_map(|i| match self.peek_module_body(ModuleHandle { object: self.ai_self, index: i as u16 }) {
                Some(crate::logic::world::update_body::UpdateBody::StealthUpdate(m)) => Some(m.is_granted_by_special_power()),
                _ => None,
            });
        }
        self.cuts.ai_host.stealth_granted_by_special_power.get(&self.ai_self).copied()
    }
    fn contained_by(&self) -> ObjectId {
        self.objects.find(self.ai_self).map_or(INVALID_ID, |o| o.contained_by)
    }
    fn container_passenger_allowed_to_fire(&mut self) -> bool {
        let c = AiHost::contained_by(self);
        // ~~the table~~ (batch I58: with `live_ai`, the container's contain module, `isPassengerAllowedToFire()` with its default
        // argument; it was always the table, false, so no garrisoned or transported unit's AI ever fired)
        if self.cuts.live_ai {
            let Some(contain) = self.get_contain(c) else { return false };
            return self.module_call(c, contain, ModuleCall::IsPassengerAllowedToFire(INVALID_ID)).as_bool();
        }
        self.cuts.ai_host.passenger_allowed_to_fire.get(&c).copied().unwrap_or(false)
    }
    fn container_bounding_circle_radius(&self) -> f32 {
        let c = AiHost::contained_by(self);
        self.objects.find(c).map_or(0.0, |o| o.geometry_info.get_bounding_circle_radius())
    }
    fn last_damage_type_and_source(&self) -> (DamageType, ObjectId) {
        self.peek_last_damage(self.ai_self).map_or((0, INVALID_ID), |d| (d.input.damage_type, d.input.source_id))
    }
    fn ai_data_force_idle_frames(&self) -> u32 {
        self.statics.ai_data.data.force_idle_frames_count
    }
    fn ai_data_attack_uses_line_of_sight(&self) -> bool {
        self.statics.ai_data.data.attack_uses_line_of_sight
    }
    fn ai_data_attack_ignore_insignificant_buildings(&self) -> bool {
        self.statics.ai_data.data.attack_ignore_insignificant_buildings
    }
    fn adjusted_vision_range_for_object(&mut self, factors: i32) -> f32 {
        let id = self.ai_self;
        // (batch I9c-3: with `live_ai`, `TheAI`'s own query)
        if self.cuts.live_ai {
            return self.ai_adjusted_vision_range(id, factors);
        }
        self.ai_host_cut("AI::getAdjustedVisionRangeForObject", vec![i64::from(id), i64::from(factors)]);
        match self.cuts.ai_host.adjusted_vision_range.get(&id) {
            Some(r) => *r,
            None => self.get_vision_range(id),
        }
    }
    fn find_closest_enemy(&mut self, range: f32, qualifiers: u32, info: Option<usize>, area: Option<i32>) -> ObjectId {
        let id = self.ai_self;
        // (batch I9c-3: with `live_ai`, `TheAI`'s own query; ~~the area filter is not live yet~~ **since batch I21** the area is
        // upstream's optional `PartitionFilterPolygonTrigger(area)`)
        if self.cuts.live_ai {
            return self.ai_find_closest_enemy_live_in(id, range, qualifiers, info, area);
        }
        self.ai_host_cut("AI::findClosestEnemy", vec![i64::from(id), i64::from(range.to_bits()), i64::from(qualifiers)]);
        self.cuts.ai_host.closest_enemy.get(&id).copied().unwrap_or(INVALID_ID)
    }
    fn current_weapon_anti_mask(&self) -> Option<i32> {
        let w = self.objects.find(self.ai_self)?.weapon_set.get_cur_weapon()?;
        self.statics.weapon_store.templates().get(w.template()).map(|t| t.anti_mask as i32)
    }
    fn player_has_upgrade_complete(&self, name: &str) -> bool {
        let Some(p) = self.get_controlling_player(self.ai_self) else { return false };
        let Some(u) = self.statics.upgrade_center.templates().find(|u| u.upgrade_name() == name.as_bytes()) else { return false };
        World::player_has_upgrade_complete(self, p, &u.upgrade_mask())
    }
    fn contain_has_objects_wanting_to_enter_or_exit(&mut self) -> Option<bool> {
        let id = self.ai_self;
        let c = self.get_contain(id)?;
        Some(self.module_call(id, c, ModuleCall::HasObjectsWantingToEnterOrExit).as_bool())
    }
    fn horde_update_interfaces(&self) -> Vec<(bool, bool)> {
        // ~~the table~~ (batch I58: with `live_ai`, each `HordeUpdate` of the object, in module order: `(isInHorde(),
        // isAllowedNationalism())`; it was always the table, empty, so no horde unit ever got its horde or nationalism bonus)
        if self.cuts.live_ai {
            let Some(o) = self.objects.find(self.ai_self) else { return Vec::new() };
            return (0..o.modules.len())
                .filter_map(|i| match self.peek_module_body(ModuleHandle { object: self.ai_self, index: i as u16 }) {
                    Some(crate::logic::world::update_body::UpdateBody::HordeUpdate(m)) => Some((m.is_in_horde(), m.is_allowed_nationalism())),
                    _ => None,
                })
                .collect();
        }
        self.cuts.ai_host.horde_updates.get(&self.ai_self).cloned().unwrap_or_default()
    }
    fn set_weapon_bonus_condition(&mut self, c: i32) {
        World::set_weapon_bonus_condition(self, self.ai_self, c);
    }
    fn clear_weapon_bonus_condition(&mut self, c: i32) {
        World::clear_weapon_bonus_condition(self, self.ai_self, c);
    }
    fn logic_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.random.game_logic_random_value(lo, hi)
    }
}

#[cfg(test)]
mod tests {
    //! The AI code itself is checked against the compiled C++ by `tests/differential_ai_update.rs` and
    //! `tests/differential_locomotor.rs` with stand-in hosts; these check the world answering for the object whose AI runs.
    use super::*;
    use crate::common::object_status_types::ObjectStatusMaskType;
    use crate::logic::ai::ai_update::AiUpdate;
    use crate::logic::ai::ai_update_data::AiUpdateModuleData;
    use crate::logic::locomotor::LOCOMOTORSURFACE_GROUND;
    use crate::logic::world::pathfinder_host_world::PfOp;
    use crate::logic::world::player_host_world::tests::world;
    use std::sync::Arc;

    fn make(w: &mut World, name: &[u8], x: f32, y: f32) -> ObjectId {
        let t = World::thing_factory_find_template(w, name).unwrap();
        let id = World::thing_factory_new_object(w, t, None, ObjectStatusMaskType::default()).unwrap();
        World::set_position(w, id, &Coord3D { x, y, z: 0.0 });
        id
    }

    /// A tank at (50, 50) and a depot at (200, 200) on the pathfinder's map. Inside `with_ai_self(tank)` the hosts answer
    /// for the tank (its id, its position; `set_position` moves it), and outside it no object is "this" again. The
    /// pathfinder answers the movement positions: a ground unit cannot stand in the depot's cell and can in the open.
    /// `AiUpdate::new` (the ported constructor) runs on the world; with no locomotor (the fixture has none) its set's
    /// surfaces are none, so `isValidLocomotorPosition` is false even in the open.
    #[test]
    fn the_world_answers_for_the_object_whose_ai_runs() {
        let (mut w, _) = world();
        w.cuts.live_pathfinder = true;
        let tank = make(&mut w, b"Tank", 50.0, 50.0);
        make(&mut w, b"Depot", 200.0, 200.0);
        w.pathfinder_op(PfOp::NewMap);
        let (open, depot) = (Coord3D { x: 55.0, y: 55.0, z: 0.0 }, Coord3D { x: 205.0, y: 205.0, z: 0.0 });
        w.with_ai_self(tank, |w| {
            assert_eq!(AiHost::object_id(w), tank);
            assert_eq!(LocomotorHost::position(w), Coord3D { x: 50.0, y: 50.0, z: 0.0 });
            assert_eq!(AiHost::distance_squared_3d(w, &Coord3D { x: 53.0, y: 54.0, z: 0.0 }), 25.0);
            assert!(AiHost::pf_valid_movement_position(w, false, LAYER_GROUND, LOCOMOTORSURFACE_GROUND, &open));
            assert!(!AiHost::pf_valid_movement_position(w, false, LAYER_GROUND, LOCOMOTORSURFACE_GROUND, &depot));
            let ai = AiUpdate::new(w, Arc::new(AiUpdateModuleData::default()));
            assert!(!ai.is_valid_locomotor_position(w, &open));
            assert_eq!(AiHost::team_target_object(w), INVALID_ID);
            LocomotorHost::set_position(w, &Coord3D { x: 60.0, y: 70.0, z: 0.0 });
        });
        assert_eq!(w.ai_self, INVALID_ID);
        assert_eq!(w.find_object(tank).unwrap().get_position(), Coord3D { x: 60.0, y: 70.0, z: 0.0 });
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
