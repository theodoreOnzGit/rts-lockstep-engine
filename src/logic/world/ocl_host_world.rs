// SPDX-License-Identifier: GPL-3.0-or-later
//! Integration batch I11: the world as `OclHost`, the host of `ObjectCreationList.cpp`, and the object creation lists run
//! live.
//!
//! Every module's `ObjectCreationList::create(...)` reaches the world as a cut call (`CutCall::OclCreate`,
//! `OclCreateWithSecondary`, `OclCreateWithLifetime`, `OclCreateAt`, `OclCreateOwner`, `OclCreateAtPrimary`, and the
//! weapon's `WeaponOclCreate`). With `cuts.live_ocl` (off by default, so the differential tests keep their tables) the
//! world runs the list: [`World::live_ocl_call`] from [`World::cut`], and [`World::ocl_create_returning`] for the one
//! caller that needs the first object made. The list is the statics' store's by name (a weapon's by its name key, which
//! [`World::weapon_store_post_process`] gives it); a name that is no list makes nothing, as upstream's static
//! `create(NULL, ...)` does.
//!
//! **What the world answers and how.** The objects, their modules (physics, body, contain, the AI, the lifetime, float,
//! radius decal and slaved updates, the special power completion die), the thing factory, the players and teams, the
//! script engine's names, the terrain, the pathfinder and the partition manager are the world's. Two module calls were
//! added for the `DeliverPayload` nugget: `ModuleCall::AiDeliverPayload` and
//! `ModuleCall::AiGetCurLocomotorMaxSpeedForCondition`. **The client's parts are not the logic's:** a drawable's
//! effective hiddenness answers `false` (the logic does not track the client's), its debris draw modules are none (so
//! the debris model and animation names are not set: they change only what is drawn), a fade and a hide are client
//! events, a particle system is a client event for a template the body tables know
//! (`cuts.body.particle_templates`), and an audio event is a client event. `TheGameLODManager->isDebrisSkipped()` is
//! `false` (the logic runs at full detail; a skipped debris would change the logic's object count by machine).
//!
//! **Deviation:** `ThePartitionManager->findPositionAround` into an uninitialised `resultPos` (the spread formation)
//! answers the centre when nothing is found; upstream reads an uninitialised `Coord3D`.
//!
//! `p1w13-sonnet`, integration batch I11, 2026-11-02.

use crate::common::base_type::Coord3D;
use crate::common::damage_fx::FxListName;
use crate::common::disabled_types::DisabledType;
use crate::common::game_common::{CommandSourceType, VeterancyLevel};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ID};
use crate::common::kind_of::KINDOF_STRUCTURE;
use crate::common::name_key_generator::NameKeyType;
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::modules::deliver_payload_data::{DeliverPayloadData, WeaponTemplateRef};
use crate::logic::object_creation_list::{FindPositionOptions, OclHost, UpdateModuleKind};
use crate::logic::radius_decal::RadiusDecalTemplate;
use crate::logic::world::build_assistant::PathfindCellType;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{PlayerIndex, TeamId};
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::team_ops::TemplateRef;
use crate::ww::matrix3d::Matrix3D;

impl World {
    /// The index of the module of object `o` whose class is `class_name`, if any.
    fn ocl_module(&self, o: ObjectId, class_name: &[u8]) -> Option<ModuleIndex> {
        self.find_module(o, class_name)
    }

    /// A call to object `o`'s physics module, if it has one.
    fn ocl_physics(&mut self, o: ObjectId, call: ModuleCall) {
        if let Some(p) = self.objects.find(o).and_then(|x| x.physics) {
            self.module_call(o, p, call);
        }
    }

    /// A call to object `o`'s module of class `class_name`, if it has one.
    fn ocl_module_call(&mut self, o: ObjectId, class_name: &[u8], call: ModuleCall) {
        if let Some(m) = self.ocl_module(o, class_name) {
            self.module_call(o, m, call);
        }
    }

    /// Runs `ObjectCreationList::create(...)` for an OCL cut call with `cuts.live_ocl`: `true` when `call` is one (run or
    /// not: a name that is no list makes nothing). The first object made is returned through `made`.
    pub(crate) fn live_ocl_call(&mut self, call: &CutCall) -> Option<Option<ObjectId>> {
        let some = |id: ObjectId| (id != INVALID_ID).then_some(id);
        let statics = self.statics.clone();
        let store = &statics.object_creation_list_store;
        let r = match call {
            CutCall::OclCreate { object, ocl } => {
                let Some(list) = store.find_object_creation_list(&mut self.names, ocl) else { return Some(None) };
                list.create_object(self, some(*object), None, 0)
            }
            CutCall::OclCreateWithSecondary { primary, secondary, ocl } => {
                let Some(list) = store.find_object_creation_list(&mut self.names, ocl) else { return Some(None) };
                list.create_object(self, some(*primary), some(*secondary), 0)
            }
            CutCall::OclCreateWithLifetime { primary, secondary, ocl, lifetime_frames } => {
                let Some(list) = store.find_object_creation_list(&mut self.names, ocl) else { return Some(None) };
                list.create_object(self, some(*primary), some(*secondary), *lifetime_frames)
            }
            CutCall::OclCreateAt { primary, ocl, primary_pos, secondary_pos, angle } => {
                let Some(list) = store.find_object_creation_list(&mut self.names, ocl) else { return Some(None) };
                list.create_at(self, some(*primary), Some(primary_pos), Some(secondary_pos), *angle, 0)
            }
            CutCall::OclCreateOwner { primary, ocl, primary_pos, secondary_pos, create_owner } => {
                let Some(list) = store.find_object_creation_list(&mut self.names, ocl) else { return Some(None) };
                list.create_with_owner(self, some(*primary), Some(primary_pos), Some(secondary_pos), *create_owner, 0)
            }
            CutCall::OclCreateAtPrimary { primary, ocl, primary_pos, angle } => {
                let Some(list) = store.find_object_creation_list(&mut self.names, ocl) else { return Some(None) };
                list.create_at(self, some(*primary), Some(primary_pos), None, *angle, 0)
            }
            CutCall::WeaponOclCreate { ocl, primary } => {
                let Some(list) = store.ocls.get(&NameKeyType(*ocl)) else { return Some(None) };
                list.create_object(self, some(*primary), None, 0)
            }
            _ => return None,
        };
        match r {
            Ok(first) => Some(first),
            Err(e) => {
                self.cuts.team_live_errors.push(format!("ObjectCreationList::create: {e:?}"));
                Some(None)
            }
        }
    }

    /// `TheWeaponStore->postProcessLoad()`: each weapon template's projectile template and OCLs found by name (an OCL is its
    /// name key in the store, which [`World::live_ocl_call`] looks up). Upstream runs it once the INI is read; integration batch
    /// I11 adds it to the world (it was run only by the weapon module's own tests, so the world's weapons had no projectile and
    /// no fire OCL).
    pub fn weapon_store_post_process(&mut self) {
        let statics = std::sync::Arc::make_mut(&mut self.statics);
        crate::logic::world::game_data::weapon_store_post_process(&mut statics.weapon_store, &statics.thing_factory, &statics.object_creation_list_store, &mut self.names);
    }
}

impl OclHost for World {
    // ---- global state ---------------------------------------------------------------------------------------------
    fn frame(&self) -> u32 {
        World::frame(self)
    }
    fn is_debris_skipped(&mut self) -> bool {
        false
    }
    fn random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.random.game_logic_random_value(lo, hi)
    }
    fn random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.random.game_logic_random_value_real(lo, hi)
    }
    fn destroy_object(&mut self, o: ObjectId) {
        World::destroy_object(self, o)
    }
    fn neutral_player_default_team(&mut self) -> Option<TeamId> {
        self.players.get_nth(0)?;
        let t = self.tq().player_default_team(0);
        (t != 0).then_some(t)
    }
    fn player_default_team(&mut self, player: PlayerIndex) -> Option<TeamId> {
        let t = self.tq().player_default_team(player);
        (t != 0).then_some(t)
    }
    fn player_is_active(&mut self, player: PlayerIndex) -> bool {
        self.tq().player_is_active(player)
    }
    fn transfer_object_name(&mut self, name: &[u8], o: ObjectId) {
        crate::logic::world::script_actions::ScriptActionsHost::sa_transfer_object_name(self, name, o)
    }

    // ---- the thing factory ----------------------------------------------------------------------------------------
    fn debris_template(&mut self) -> Option<TemplateRef> {
        self.thing_factory_find_template(b"GenericDebris").map(u32::from)
    }
    fn find_template(&mut self, name: &[u8]) -> Option<TemplateRef> {
        self.thing_factory_find_template(name).map(u32::from)
    }
    fn new_object(&mut self, tmplate: Option<TemplateRef>, team: Option<TeamId>) -> Option<ObjectId> {
        let t = u16::try_from(tmplate?).ok()?;
        self.thing_factory_new_object(t, team, Default::default()).ok()
    }

    // ---- an object -------------------------------------------------------------------------------------------------
    fn obj_position(&mut self, o: ObjectId) -> Coord3D {
        self.objects.find(o).map(|x| x.get_position()).unwrap_or_default()
    }
    fn obj_orientation(&mut self, o: ObjectId) -> f32 {
        self.objects.find(o).map_or(0.0, |x| x.thing.orientation())
    }
    fn obj_transform(&mut self, o: ObjectId) -> Matrix3D {
        self.objects.find(o).map_or_else(Matrix3D::identity, |x| *x.thing.transform_matrix())
    }
    fn obj_controlling_player(&mut self, o: ObjectId) -> Option<PlayerIndex> {
        self.get_controlling_player(o)
    }
    fn obj_layer(&mut self, o: ObjectId) -> PathfindLayer {
        self.objects.find(o).map_or_else(PathfindLayer::default, |x| x.layer)
    }
    fn obj_is_above_terrain(&mut self, o: ObjectId) -> bool {
        World::is_above_terrain(self, o)
    }
    fn obj_is_significantly_above_terrain(&mut self, o: ObjectId) -> bool {
        World::is_significantly_above_terrain(self, o)
    }
    fn obj_is_off_map(&mut self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.is_off_map())
    }
    fn obj_is_structure(&mut self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.is_kind_of(KINDOF_STRUCTURE))
    }
    fn obj_veterancy_level(&mut self, o: ObjectId) -> VeterancyLevel {
        self.objects.find(o).map_or(VeterancyLevel::Regular, |x| x.get_veterancy_level())
    }
    fn obj_indicator_color(&mut self, o: ObjectId) -> i32 {
        self.get_indicator_color(o)
    }
    fn obj_name(&mut self, o: ObjectId) -> Vec<u8> {
        self.objects.find(o).map(|x| x.name.clone()).unwrap_or_default()
    }
    fn obj_unit_direction_vector_2d(&mut self, o: ObjectId) -> Coord3D {
        self.objects.find_mut(o).map(|x| x.thing.unit_direction_vector_2d()).unwrap_or_default()
    }
    fn obj_set_producer(&mut self, o: ObjectId, producer: Option<ObjectId>) {
        World::set_producer(self, o, producer.unwrap_or(INVALID_ID))
    }
    fn obj_set_position(&mut self, o: ObjectId, pos: &Coord3D) {
        self.set_position(o, pos)
    }
    fn obj_set_orientation(&mut self, o: ObjectId, angle: f32) {
        self.set_orientation(o, angle)
    }
    fn obj_set_transform_matrix(&mut self, o: ObjectId, mtx: &Matrix3D) {
        World::set_transform_matrix(self, o, mtx)
    }
    fn obj_set_layer(&mut self, o: ObjectId, layer: PathfindLayer) {
        self.set_layer(o, layer)
    }
    fn obj_set_script_status(&mut self, o: ObjectId, bit: u8) {
        self.set_script_status(o, bit, true)
    }
    fn obj_set_disabled_until(&mut self, o: ObjectId, disabled: DisabledType, frame: u32) {
        self.set_disabled_until(o, disabled, frame)
    }
    fn obj_set_weapon_lock(&mut self, o: ObjectId, slot: i32, lock: i32) {
        self.set_weapon_lock(o, slot, lock)
    }
    fn obj_go_invulnerable(&mut self, o: ObjectId, frames: u32) {
        self.go_invulnerable(o, frames)
    }
    fn obj_kill(&mut self, o: ObjectId) {
        // `kill(DamageType damageType = DAMAGE_UNRESISTABLE, DeathType deathType = DEATH_NORMAL)`
        self.kill(o, DAMAGE_UNRESISTABLE, DEATH_NORMAL)
    }
    fn obj_attempt_damage(&mut self, o: ObjectId, info: &DamageInfo) {
        let mut info = info.clone();
        self.attempt_damage(o, &mut info)
    }
    fn obj_has_update_module(&mut self, o: ObjectId, kind: UpdateModuleKind) -> bool {
        let name: &[u8] = match kind {
            UpdateModuleKind::RadiusDecal => b"RadiusDecalUpdate",
            UpdateModuleKind::DeliverPayloadAi => b"DeliverPayloadAIUpdate",
            UpdateModuleKind::Lifetime => b"LifetimeUpdate",
            UpdateModuleKind::Float => b"FloatUpdate",
        };
        self.ocl_module(o, name).is_some()
    }
    fn obj_has_special_power_completion_die(&mut self, o: ObjectId) -> bool {
        self.module_with_interface(o, ModuleCall::IsSpecialPowerCompletionDie).is_some()
    }
    fn obj_has_ai(&mut self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.ai.is_some())
    }
    fn obj_has_physics(&mut self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.physics.is_some())
    }
    fn obj_has_body(&mut self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.body.is_some())
    }
    fn obj_has_contain(&mut self, o: ObjectId) -> bool {
        self.get_contain(o).is_some()
    }
    fn obj_has_drawable(&mut self, o: ObjectId) -> bool {
        self.has_drawable(o)
    }
    fn obj_is_trainable(&mut self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.template().is_trainable())
    }
    fn obj_set_veterancy_level(&mut self, o: ObjectId, level: VeterancyLevel) {
        // `setVeterancyLevel(newLevel, provideFeedback = TRUE)`
        self.cut_experience_set_veterancy_level(o, level, true)
    }

    // ---- the modules of an object ---------------------------------------------------------------------------------
    fn ai_attack_position(&mut self, o: ObjectId, pos: &Coord3D, max_shots: i32, cmd: CommandSourceType) {
        if let Some(ai) = self.objects.find(o).and_then(|x| x.ai) {
            self.module_call(o, ai, ModuleCall::AiAttackPosition { pos: Some(*pos), max_shots, command_source: cmd as i32 });
        }
    }
    fn radius_decal_create(&mut self, o: ObjectId, tmpl: &RadiusDecalTemplate, radius: f32, pos: &Coord3D) {
        self.ocl_module_call(o, b"RadiusDecalUpdate", ModuleCall::RadiusDecalCreate { template: tmpl.clone(), radius, pos: *pos });
    }
    fn radius_decal_kill_when_no_longer_attacking(&mut self, o: ObjectId) {
        self.ocl_module_call(o, b"RadiusDecalUpdate", ModuleCall::RadiusDecalKillWhenNoLongerAttacking(true));
    }
    fn deliver_payload(&mut self, o: ObjectId, move_to: &Coord3D, target: &Coord3D, data: &DeliverPayloadData) {
        if let Some(ai) = self.objects.find(o).and_then(|x| x.ai) {
            self.module_call(o, ai, ModuleCall::AiDeliverPayload { move_to: *move_to, target: *target, data: data.clone() });
        }
    }
    fn deliver_payload_max_speed_for_condition(&mut self, o: ObjectId, damage_state: i32) -> f32 {
        let Some(ai) = self.objects.find(o).and_then(|x| x.ai) else { return 0.0 };
        self.module_call(o, ai, ModuleCall::AiGetCurLocomotorMaxSpeedForCondition(damage_state)).as_real(0.0)
    }
    fn deliver_payload_preferred_height(&mut self, o: ObjectId) -> f32 {
        let Some(ai) = self.objects.find(o).and_then(|x| x.ai) else { return 0.0 };
        self.module_call(o, ai, ModuleCall::AiGetCurLocomotorPreferredHeight).as_real(0.0)
    }
    fn body_damage_state(&mut self, o: ObjectId) -> i32 {
        let Some(b) = self.objects.find(o).and_then(|x| x.body) else { return 0 };
        self.module_call(o, b, ModuleCall::GetDamageState).as_int(0)
    }
    fn body_set_initial_health(&mut self, o: ObjectId, percent: i32) {
        if let Some(b) = self.objects.find(o).and_then(|x| x.body) {
            self.module_call(o, b, ModuleCall::SetInitialHealth(percent));
        }
    }
    fn lifetime_set_range(&mut self, o: ObjectId, min_frames: u32, max_frames: u32) {
        self.ocl_module_call(o, b"LifetimeUpdate", ModuleCall::SetLifetimeRange { min_frames, max_frames });
    }
    fn float_set_enabled(&mut self, o: ObjectId) {
        self.ocl_module_call(o, b"FloatUpdate", ModuleCall::FloatSetEnabled(true));
    }
    fn special_power_completion_die_set_creator(&mut self, o: ObjectId, creator: ObjectId) {
        if let Some(d) = self.module_with_interface(o, ModuleCall::IsSpecialPowerCompletionDie) {
            self.module_call(o, d, ModuleCall::SetCreator { creator });
        }
    }
    fn behavior_module_count(&mut self, o: ObjectId) -> usize {
        self.objects.find(o).map_or(0, |x| x.modules.len())
    }
    fn behavior_module_has_slaved_update(&mut self, o: ObjectId, i: usize) -> bool {
        self.module_call(o, i as ModuleIndex, ModuleCall::HasSlavedUpdateInterface).as_bool()
    }
    fn behavior_module_on_enslave(&mut self, o: ObjectId, i: usize, slaver: Option<ObjectId>) {
        self.module_call(o, i as ModuleIndex, ModuleCall::SlavedOnEnslave { slaver: slaver.unwrap_or(INVALID_ID) });
    }

    // ---- physics ----------------------------------------------------------------------------------------------------
    fn physics_velocity(&mut self, o: ObjectId) -> Coord3D {
        let Some(p) = self.objects.find(o).and_then(|x| x.physics) else { return Coord3D::default() };
        self.module_call(o, p, ModuleCall::PhysicsGetVelocity).as_coord()
    }
    fn physics_mass(&mut self, o: ObjectId) -> f32 {
        let Some(p) = self.objects.find(o).and_then(|x| x.physics) else { return 0.0 };
        self.module_call(o, p, ModuleCall::PhysicsGetMass).as_real(0.0)
    }
    fn physics_apply_motive_force(&mut self, o: ObjectId, force: &Coord3D) {
        self.ocl_physics(o, ModuleCall::PhysicsApplyMotiveForce(*force))
    }
    fn physics_apply_force(&mut self, o: ObjectId, force: &Coord3D) {
        self.ocl_physics(o, ModuleCall::PhysicsApplyForce(*force))
    }
    fn physics_set_mass(&mut self, o: ObjectId, mass: f32) {
        self.ocl_physics(o, ModuleCall::PhysicsSetMass(mass))
    }
    fn physics_set_extra_friction(&mut self, o: ObjectId, f: f32) {
        self.ocl_physics(o, ModuleCall::PhysicsSetExtraFriction(f))
    }
    fn physics_set_extra_bounciness(&mut self, o: ObjectId, b: f32) {
        self.ocl_physics(o, ModuleCall::PhysicsSetExtraBounciness(b))
    }
    fn physics_set_allow_bouncing(&mut self, o: ObjectId, b: bool) {
        self.ocl_physics(o, ModuleCall::PhysicsSetAllowBouncing(b))
    }
    fn physics_set_allow_to_fall(&mut self, o: ObjectId, b: bool) {
        self.ocl_physics(o, ModuleCall::PhysicsSetAllowToFall(b))
    }
    fn physics_set_bounce_sound(&mut self, o: ObjectId, name: &[u8]) {
        self.ocl_physics(o, ModuleCall::PhysicsSetBounceSound(Some(name.to_vec())))
    }
    fn physics_set_ignore_collisions_with(&mut self, o: ObjectId, other: Option<ObjectId>) {
        self.ocl_physics(o, ModuleCall::PhysicsSetIgnoreCollisionsWith(other.unwrap_or(INVALID_ID)))
    }
    fn physics_set_angles(&mut self, o: ObjectId, yaw: f32, pitch: f32, roll: f32) {
        self.ocl_physics(o, ModuleCall::PhysicsSetAngles { yaw, pitch, roll })
    }
    fn physics_set_yaw_rate(&mut self, o: ObjectId, r: f32) {
        self.ocl_physics(o, ModuleCall::PhysicsSetYawRate(r))
    }
    fn physics_set_roll_rate(&mut self, o: ObjectId, r: f32) {
        self.ocl_physics(o, ModuleCall::PhysicsSetRollRate(r))
    }
    fn physics_set_pitch_rate(&mut self, o: ObjectId, r: f32) {
        self.ocl_physics(o, ModuleCall::PhysicsSetPitchRate(r))
    }

    // ---- containment ------------------------------------------------------------------------------------------------
    fn contain_is_valid_container_for(&mut self, container: ObjectId, obj: ObjectId, check_capacity: bool) -> bool {
        let Some(c) = self.get_contain(container) else { return false };
        self.module_call(container, c, ModuleCall::IsValidContainerFor { obj, check_capacity }).as_bool()
    }
    fn contain_add(&mut self, container: ObjectId, obj: ObjectId) {
        if let Some(c) = self.get_contain(container) {
            self.module_call(container, c, ModuleCall::AddToContain(obj));
        }
    }

    // ---- the drawable (the client's) --------------------------------------------------------------------------------
    fn drawable_is_effectively_hidden(&mut self, _o: ObjectId) -> bool {
        false
    }
    fn drawable_set_hidden(&mut self, o: ObjectId, hidden: bool) {
        self.emit(ObjectClientEvent::DrawableSetHidden { object_id: o, hidden });
    }
    fn drawable_fade_in(&mut self, o: ObjectId, frames: u32) {
        self.emit(ObjectClientEvent::DrawableFade { object_id: o, fade_in: true, frames });
    }
    fn drawable_fade_out(&mut self, o: ObjectId, frames: u32) {
        self.emit(ObjectClientEvent::DrawableFade { object_id: o, fade_in: false, frames });
    }
    fn drawable_debris_interface_count(&mut self, _o: ObjectId) -> usize {
        0
    }
    fn debris_set_model_name(&mut self, _o: ObjectId, _i: usize, _name: &[u8], _color: i32, _shadow_type: u32) {}
    fn debris_set_anim_names(&mut self, _o: ObjectId, _i: usize, _initial: &[u8], _flying: &[u8], _fin: &[u8], _final_fx: &FxListName) {}

    // ---- the world --------------------------------------------------------------------------------------------------
    fn terrain_ground_height(&mut self, x: f32, y: f32) -> f32 {
        self.terrain_get_ground_height(x, y)
    }
    fn terrain_highest_layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer {
        self.terrain_get_highest_layer_for_destination(pos)
    }
    fn terrain_layer_height(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32 {
        self.terrain_get_layer_height(x, y, layer)
    }
    fn terrain_flatten(&mut self, o: ObjectId) {
        if !self.terrain_flatten_object(o) {
            self.cut(CutCall::Host { call: "ocl_terrain_flatten", args: vec![i64::from(o)], text: Vec::new() });
        }
    }
    fn terrain_is_underwater(&mut self, x: f32, y: f32) -> Option<(f32, f32)> {
        let water_z = self.terrain_is_underwater_water_z(x, y)?;
        let terrain_z = self.terrain_get_ground_height(x, y);
        (water_z > terrain_z).then_some((water_z, terrain_z))
    }
    fn pathfinder_cell_type(&mut self, layer: PathfindLayer, x: i32, y: i32) -> Option<PathfindCellType> {
        crate::logic::world::build_assistant::BuildAssistantHost::pathfinder_cell_type(self, layer, x, y)
    }
    fn pathfinder_add_object(&mut self, o: ObjectId) {
        self.pathfinder_add_object_to_map(o)
    }
    fn partition_find_position_around(&mut self, center: &Coord3D, options: &FindPositionOptions) -> Coord3D {
        let mut pos = *center;
        let found = if self.cuts.live_partition {
            let o = crate::logic::world::partition::FindPositionOptions {
                flags: options.flags,
                min_radius: options.min_radius,
                max_radius: options.max_radius,
                start_angle: options.start_angle,
                max_z_delta: options.max_z_delta,
                ..Default::default()
            };
            self.with_partition(|pm, w| pm.find_position_around(w, center, &o))
        } else {
            self.contain_partition_find_position_around(&mut pos, options.min_radius, options.max_radius, options.flags as i32).then_some(pos)
        };
        found.unwrap_or(*center)
    }
    fn weapon_create_and_fire_temp(&mut self, weapon: WeaponTemplateRef, source: ObjectId, pos: &Coord3D) {
        if let Err(e) = crate::logic::weapon_fire::create_and_fire_temp_weapon_at_pos(self, Some(weapon as usize), Some(source), pos) {
            self.cuts.team_live_errors.push(format!("WeaponStore::createAndFireTempWeapon (OCL): {e:?}"));
        }
    }
    fn particle_system_find_template(&mut self, name: &[u8]) -> Option<u32> {
        self.cuts.body.particle_templates.iter().position(|n| n.as_slice() == name).map(|i| i as u32)
    }
    fn particle_system_create_attached(&mut self, tmpl: u32, o: ObjectId) {
        if let Some(name) = self.cuts.body.particle_templates.iter().nth(tmpl as usize).cloned() {
            self.emit(ObjectClientEvent::ParticleSystemCreateAttached { template: name, object_id: o });
        }
    }
    fn audio_add_event(&mut self, name: &[u8], object: ObjectId) {
        self.emit(ObjectClientEvent::AudioAddEvent { name: name.to_vec(), object_id: object, position: None, player_index: None });
    }
}
