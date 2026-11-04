// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/BuildAssistant.cpp (the calls `BuildAssistant` makes out of itself)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I6: the world as `BuildAssistantHost`, and `TheBuildAssistant` in the world
//! ([`World::build_assistant`]). `GameLogic::update`'s `TheBuildAssistant->update()` (batch I4's cut point), the
//! player host's `sellObject` and the object code's (both batch I3's cut point `CutCall::BuildAssistantSellObject`) are
//! now the assistant's own.
//!
//! - **The world:** the global data, the frame, the game logic's random values, the objects (their templates, kinds,
//!   statuses, geometry, position, orientation, players, relationships, construction percent, model conditions,
//!   producer), their modules (the AI's `isBusy`, `aiIdle` and `construct`; the container's `onSelling`; the production
//!   queue's `canQueueCreateUnit` and `cancelAndRefundAllProduction`; the parking place's `killAllParkedUnits`; the
//!   create modules' `onBuildComplete`), the thing factory and templates, the players (`canBuild`,
//!   `canBuildMoreOfType`, `calcCostToBuild`, money, `onStructureConstructionComplete`, `onUnitCreated`), the terrain
//!   logic, and the partition manager (with `cuts.live_partition`).
//! - **Cut points** ([`CutCall::Host`]; answers in tables): the AI's quick path and its move "even if sleeping" (the AI
//!   update comes in with batch I9), the control bar's command sets and buttons (`cuts.command_sets`,
//!   `cuts.command_buttons`, whose new `thing_template` field answers `getThingTemplate()`), ~~the production queue's
//!   special power construction button~~ (**Corrected 2026-11-03 (batch I51):** the module's own with
//!   `cuts.live_build_assistant`, the table without), the terrain visual (`getRestrictConstruction`, the faction bib, the trees and
//!   props), the drawable's animation loop, the audio, and the pathfinder's cells (`cuts.contain.cell_types`).
//!
//! `p1w13-sonnet`, integration batch I6, 2026-10-31.

use crate::common::base_type::{Coord3D, Region3D};
use crate::common::game_common::{PlayerType, Relationship};
use crate::common::game_type::PathfindLayer;
use crate::common::geometry::GeometryInfo;
use crate::common::kind_of::{KindOfMaskType, KindOfType, KINDOF_STRUCTURE, KINDOF_SUPPLY_SOURCE};
use crate::common::model_state::ModelConditionFlagType;
use crate::common::object_status_types::ObjectStatusTypes;
use crate::logic::world::build_assistant::{
    build_assistant_sell_object, build_assistant_update, BuildAssistantHost, BuildGlobals, CanMakeType, PathfindCellType,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{ObjectId, PlayerIndex, INVALID_ID};
use crate::logic::world::module_call::{ModuleCall, ModuleReply};
use crate::logic::world::object::ModuleIndex;
use crate::logic::world::object_cuts::CutCall;
use crate::common::game_common::CellShroudStatus;
use crate::logic::world::partition::{DistanceCalculationType, PartitionFilter};
use crate::logic::world::player_objects::{
    player_calc_cost_to_build, player_can_build, player_can_build_more_of_type, player_money_deposit, player_on_structure_construction_complete,
};
use crate::logic::world::player_ops::{player_get_relationship, player_is_skirmish_ai_player, player_on_unit_created};
use crate::logic::world::team_ops::TemplateRef;
use crate::logic::world::update_body::UpdateBody;

/// `CMD_FROM_AI`.
const CMD_FROM_AI: i32 = 2;

/// The geometry of an object or template that is not there (the callers only ask about ones that are).
fn no_geometry() -> GeometryInfo {
    GeometryInfo::new(crate::common::geometry::GeometryType::Sphere, false, 0.0, 0.0, 0.0)
}

impl World {
    /// `TheBuildAssistant->update()`.
    pub fn build_assistant_update(&mut self) {
        let mut ba = std::mem::take(&mut self.build_assistant);
        build_assistant_update(self, &mut ba);
        self.build_assistant = ba;
    }

    /// `TheBuildAssistant->sellObject(obj)`.
    pub fn build_assistant_sell(&mut self, obj: ObjectId) {
        let mut ba = std::mem::take(&mut self.build_assistant);
        build_assistant_sell_object(self, &mut ba, obj);
        self.build_assistant = ba;
    }

    /// Logs a call of the build assistant into a unit the world does not hold.
    fn ba_cut(&mut self, call: &'static str, args: &[i64]) {
        self.cut(CutCall::Host { call, args: args.to_vec(), text: Vec::new() });
    }

    /// The final override of the template a handle names.
    fn ba_tmpl(&self, t: TemplateRef) -> Option<&crate::common::thing_template::ThingTemplate> {
        u16::try_from(t).ok().and_then(|id| self.statics.thing_factory.find_by_template_id(id)).map(|o| &o.get_final_override().data)
    }

    /// The first behavior module of the object with a parking place interface.
    fn ba_parking_place(&self, o: ObjectId) -> Option<ModuleIndex> {
        let n = self.find_object(o)?.modules.len();
        (0..n as ModuleIndex).find(|&i| {
            let h = crate::logic::world::module_call::ModuleHandle { object: o, index: i };
            matches!(self.peek_module_body(h), Some(UpdateBody::ParkingPlaceBehavior(_)) | Some(UpdateBody::FlightDeckBehavior(_)))
        })
    }
}

impl BuildAssistantHost for World {
    fn build_globals(&self) -> BuildGlobals {
        let g = self.statics.global_data.the_global_data();
        BuildGlobals {
            max_line_build_objects: g.map_or(0, |g| g.max_line_build_objects),
            min_dist_from_edge_of_map_for_build: g.map_or(0.0, |g| g.min_dist_from_edge_of_map_for_build),
            allowed_height_variation_for_building: g.map_or(0.0, |g| g.allowed_height_variation_for_building),
            supply_build_border: g.map_or(0.0, |g| g.supply_build_border),
            sell_percentage: g.map_or(1.0, |g| g.sell_percentage),
        }
    }
    fn frame(&self) -> u32 {
        World::frame(self)
    }
    fn random_value_real(&mut self, lo: f32, hi: f32) -> f32 {
        self.random.game_logic_random_value_real(lo, hi)
    }
    fn find_object_by_id(&self, id: ObjectId) -> Option<ObjectId> {
        self.find_object(id).map(|o| o.id)
    }
    fn destroy_object(&mut self, o: ObjectId) {
        World::destroy_object(self, o);
    }
    fn deselect_object(&mut self, o: ObjectId) {
        // `PLAYERMASK_ALL`
        self.game_logic_deselect_object(o, 0xffff, true);
    }
    fn all_objects(&self) -> Vec<ObjectId> {
        self.objects.ids_in_order()
    }
    fn obj_template(&self, o: ObjectId) -> TemplateRef {
        self.find_object(o).map_or(0, |x| TemplateRef::from(x.template().get_template_id()))
    }
    fn obj_is_kind_of(&self, o: ObjectId, k: KindOfType) -> bool {
        self.find_object(o).is_some_and(|x| x.is_kind_of(k))
    }
    fn obj_test_status(&self, o: ObjectId, s: ObjectStatusTypes) -> bool {
        self.find_object(o).is_some_and(|x| x.test_status(s))
    }
    fn obj_test_script_status_bit(&self, o: ObjectId, bit: u8) -> bool {
        self.find_object(o).is_some_and(|x| x.test_script_status_bit(bit))
    }
    fn obj_is_disabled(&self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.is_disabled())
    }
    fn obj_is_effectively_dead(&self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.is_effectively_dead())
    }
    fn obj_geometry(&self, o: ObjectId) -> GeometryInfo {
        self.find_object(o).map_or_else(no_geometry, |x| x.geometry_info.clone())
    }
    fn obj_position(&self, o: ObjectId) -> Coord3D {
        self.find_object(o).map(|x| x.get_position()).unwrap_or_default()
    }
    fn obj_orientation(&self, o: ObjectId) -> f32 {
        self.find_object(o).map_or(0.0, |x| x.thing.orientation())
    }
    fn obj_controlling_player(&self, o: ObjectId) -> Option<PlayerIndex> {
        self.get_controlling_player(o)
    }
    fn obj_relationship(&self, a: ObjectId, b: ObjectId) -> Relationship {
        self.get_relationship(a, b)
    }
    fn obj_has_ai(&self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.ai.is_some())
    }
    fn ai_is_busy(&self, o: ObjectId) -> bool {
        // ~~(a question to the AI module: the AI update is not a module of the world until batch I9; the table)~~ **corrected
        // 2026-11-03 (batch I43):** the AI update has been a module of the world since batch I9b; with `live_ai` its own `isBusy()`
        // (it was always the table); the table otherwise
        if self.cuts.live_ai {
            return match (self.ai_peek(o), self.ai_machine_query(o)) {
                (Some(a), Some(m)) => a.is_busy(m),
                _ => false,
            };
        }
        self.cuts.ai_busy.contains(&o)
    }
    fn ai_idle(&mut self, o: ObjectId) {
        self.ai_ask(o, ModuleCall::AiIdleFrom { command_source: CMD_FROM_AI });
    }
    fn ai_construct(&mut self, o: ObjectId, what: TemplateRef, pos: Coord3D, angle: f32, player: PlayerIndex) -> Option<ObjectId> {
        let template = u16::try_from(what).ok();
        match self.ai_ask(o, ModuleCall::AiConstruct { template, pos, angle, owner: Some(player), is_rebuild: false }) {
            Some(ModuleReply::Id(id)) if id != INVALID_ID => Some(id),
            _ => None,
        }
    }
    fn ai_is_quick_path_available(&mut self, o: ObjectId, pos: Coord3D) -> bool {
        // (integration batch I21: with `live_ai`, the AI's own `isQuickPathAvailable`, `ai_paths`, with the object as `ai_self`)
        if self.cuts.live_ai && self.find_object(o).is_some_and(|x| x.ai.is_some()) {
            return self.with_ai_self(o, |w| crate::logic::ai::ai_paths::is_quick_path_available(w, &pos));
        }
        self.ba_cut("AIUpdateInterface::isQuickPathAvailable", &[i64::from(o), i64::from(pos.x.to_bits()), i64::from(pos.y.to_bits())]);
        !self.cuts.ai_no_quick_path.contains(&o)
    }
    fn ai_move_to_position_even_if_sleeping(&mut self, o: ObjectId, pos: Coord3D) {
        // (integration batch I21: with `live_ai`, `aiMoveToPositionEvenIfSleeping(&pos, CMD_FROM_AI)`, BuildAssistant.cpp:1478)
        if self.cuts.live_ai {
            if let Some(ai) = self.find_object(o).and_then(|x| x.ai) {
                use crate::logic::ai::ai_commands::{AiCommandParms, AICMD_MOVE_TO_POSITION_EVEN_IF_SLEEPING};
                let mut p = AiCommandParms::new(AICMD_MOVE_TO_POSITION_EVEN_IF_SLEEPING, crate::common::game_common::CommandSourceType::FromAi as i32);
                p.pos = pos;
                self.module_call(o, ai, crate::logic::world::module_call::ModuleCall::AiDoCommandParms(p));
                return;
            }
        }
        self.ba_cut("AIUpdateInterface::aiMoveToPositionEvenIfSleeping", &[i64::from(o), i64::from(pos.x.to_bits()), i64::from(pos.y.to_bits())]);
    }
    fn obj_producer_id(&self, o: ObjectId) -> ObjectId {
        self.find_object(o).map_or(INVALID_ID, |x| x.producer_id)
    }
    fn obj_construction_percent(&self, o: ObjectId) -> f32 {
        self.find_object(o).map_or(0.0, |x| x.construction_percent)
    }
    fn obj_set_construction_percent(&mut self, o: ObjectId, p: f32) {
        if let Some(x) = self.find_object_mut(o) {
            x.construction_percent = p;
        }
    }
    fn obj_clear_and_set_model_condition(&mut self, o: ObjectId, clear: &[ModelConditionFlagType], set: &[ModelConditionFlagType]) {
        self.clear_and_set_model_condition_flags(o, clear, set);
    }
    fn obj_set_model_condition(&mut self, o: ObjectId, set: &[ModelConditionFlagType]) {
        self.clear_and_set_model_condition_flags(o, &[], set);
    }
    fn obj_set_status(&mut self, o: ObjectId, bits: &[ObjectStatusTypes]) {
        let mut mask = crate::common::object_status_types::ObjectStatusMaskType::default();
        for b in bits {
            mask.set(*b, 1);
        }
        self.set_status(o, mask, true);
    }
    fn obj_has_drawable(&self, o: ObjectId) -> bool {
        self.has_drawable(o)
    }
    fn drawable_set_animation_loop_duration(&mut self, o: ObjectId, frames: u32) {
        self.ba_cut("Drawable::setAnimationLoopDuration", &[i64::from(o), i64::from(frames)]);
    }
    fn obj_has_contain(&self, o: ObjectId) -> bool {
        self.get_contain(o).is_some()
    }
    fn contain_on_selling(&mut self, o: ObjectId) {
        if let Some(c) = self.get_contain(o) {
            self.module_call(o, c, ModuleCall::OnSelling);
        }
    }
    fn obj_has_production(&self, o: ObjectId) -> bool {
        // (integration batch I51: the factory asking from its own call is out of its slot, and is a factory)
        if self.cuts.production_lent.as_ref().is_some_and(|l| l.object == o) {
            return true;
        }
        self.find_object(o).is_some_and(|x| {
            (0..x.modules.len() as ModuleIndex).any(|i| {
                matches!(self.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: o, index: i }), Some(UpdateBody::ProductionUpdate(_)))
            })
        })
    }
    fn production_special_power_construction_template(&self, o: ObjectId) -> Option<TemplateRef> {
        // `pui->getSpecialPowerConstructionCommandButton()->getThingTemplate()`: the button's template (the control bar's).
        // (integration batch I51: the module's own button, given by it while it is lent out, else read from it, with
        // `live_build_assistant`; it was the tests' `special_power_construction_buttons` table, which no live factory is in)
        let name = match &self.cuts.production_lent {
            Some(l) if l.object == o => l.special_power_button.clone()?,
            _ if self.cuts.live_build_assistant => self.production_special_power_construction_button(o)?,
            _ => self.cuts.special_power_construction_buttons.get(&o)?.clone(),
        };
        let b = self.cuts.command_buttons.iter().find(|b| b.name == name)?;
        b.thing_template.map(TemplateRef::from)
    }
    fn production_can_queue_create_unit(&mut self, o: ObjectId, what: TemplateRef) -> CanMakeType {
        // (integration batch I51: the lent factory's own answer, given before it asked)
        let lent = self.cuts.production_lent.as_ref().filter(|l| l.object == o && u32::from(l.template) == what).map(|l| l.can_queue);
        let answer = match lent {
            Some(a) => a,
            None => {
                let Some(p) = self.get_production_update_interface(o) else { return CanMakeType::Ok };
                let Ok(template) = u16::try_from(what) else { return CanMakeType::Ok };
                self.module_call(o, p, ModuleCall::ProductionCanQueueCreateUnit { template }).as_int(0)
            }
        };
        match answer {
            1 => CanMakeType::NoPrereq,
            2 => CanMakeType::NoMoney,
            3 => CanMakeType::FactoryIsDisabled,
            4 => CanMakeType::QueueFull,
            5 => CanMakeType::ParkingPlacesFull,
            6 => CanMakeType::MaxedOutForPlayer,
            _ => CanMakeType::Ok,
        }
    }
    fn production_cancel_and_refund_all(&mut self, o: ObjectId) {
        if let Some(p) = self.get_production_update_interface(o) {
            self.module_call(o, p, ModuleCall::CancelAndRefundAllProduction);
        }
    }
    fn obj_first_parking_place(&self, o: ObjectId) -> Option<usize> {
        self.ba_parking_place(o).map(usize::from)
    }
    fn parking_kill_all_parked_units(&mut self, o: ObjectId, module: usize) {
        if let Ok(idx) = ModuleIndex::try_from(module) {
            self.module_call(o, idx, ModuleCall::ParkingPlaceKillAllParkedUnits);
        }
    }
    fn obj_create_module_count(&self, o: ObjectId) -> usize {
        self.modules_with(o, crate::common::module::MODULEINTERFACE_CREATE).len()
    }
    fn create_module_on_build_complete(&mut self, o: ObjectId, n: usize) {
        if let Some(idx) = self.modules_with(o, crate::common::module::MODULEINTERFACE_CREATE).get(n).copied() {
            self.module_call(o, idx, ModuleCall::OnBuildComplete);
        }
    }
    fn obj_handle_partition_cell_maintenance(&mut self, o: ObjectId) {
        self.handle_partition_cell_maintenance(o);
    }
    fn obj_set_producer(&mut self, o: ObjectId, producer: Option<ObjectId>) {
        self.set_producer(o, producer.unwrap_or(INVALID_ID));
    }
    fn obj_set_position(&mut self, o: ObjectId, pos: Coord3D) {
        self.set_position(o, &pos);
    }
    fn obj_set_orientation(&mut self, o: ObjectId, angle: f32) {
        self.set_orientation(o, angle);
    }
    fn thing_factory_new_object(&mut self, what: TemplateRef, owner: PlayerIndex, status: &[ObjectStatusTypes]) -> ObjectId {
        let team = self.players.get_nth(owner).and_then(|p| p.default_team);
        let mut mask = crate::common::object_status_types::ObjectStatusMaskType::default();
        for b in status {
            mask.set(*b, 1);
        }
        let Ok(t) = u16::try_from(what) else { return INVALID_ID };
        World::thing_factory_new_object(self, t, team, mask).unwrap_or(INVALID_ID)
    }
    fn pathfinder_add_object(&mut self, o: ObjectId) {
        self.contain_pathfind_add_object(o);
    }
    fn find_command_set(&self, o: ObjectId) -> Option<u32> {
        let name = self.object_get_command_set_string(o);
        self.control_bar_find_command_set(&name).map(|s| s as u32)
    }
    fn command_set_button(&self, set: u32, i: i32) -> Option<u32> {
        let i = usize::try_from(i).ok()?;
        self.command_set_get_command_button(set as usize, i).map(|b| b as u32)
    }
    fn button_command_type(&self, button: u32) -> i32 {
        self.cuts.command_buttons.get(button as usize).map_or(0, |b| b.command_type)
    }
    fn button_template(&self, button: u32) -> TemplateRef {
        self.cuts.command_buttons.get(button as usize).and_then(|b| b.thing_template).map_or(0, TemplateRef::from)
    }
    fn tmpl_geometry(&self, t: TemplateRef) -> GeometryInfo {
        self.ba_tmpl(t).map_or_else(no_geometry, |t| t.get_template_geometry_info().clone())
    }
    fn tmpl_is_kind_of(&self, t: TemplateRef, k: KindOfType) -> bool {
        self.ba_tmpl(t).is_some_and(|t| t.is_kind_of(k))
    }
    fn tmpl_factory_exit_width(&self, t: TemplateRef) -> f32 {
        self.ba_tmpl(t).map_or(0.0, |t| t.get_factory_exit_width())
    }
    fn tmpl_factory_extra_bib_width(&self, t: TemplateRef) -> f32 {
        self.ba_tmpl(t).map_or(0.0, |t| t.get_factory_extra_bib_width())
    }
    fn tmpl_vision_range(&self, t: TemplateRef) -> f32 {
        self.ba_tmpl(t).map_or(0.0, |t| t.friend_calc_vision_range())
    }
    fn tmpl_refund_value(&self, t: TemplateRef) -> i32 {
        self.ba_tmpl(t).map_or(0, |t| t.get_refund_value())
    }
    fn tmpl_calc_cost_to_build(&mut self, t: TemplateRef, player: PlayerIndex) -> i32 {
        player_calc_cost_to_build(self, player, t).unwrap_or(0)
    }
    fn tmpl_is_equivalent_to(&self, a: TemplateRef, b: TemplateRef) -> bool {
        match (u16::try_from(a), u16::try_from(b)) {
            (Ok(a), Ok(b)) => self.thing_template_is_equivalent_to(a, b),
            _ => false,
        }
    }
    fn audio_add_voice_created(&mut self, t: TemplateRef, o: ObjectId) {
        self.ba_cut("AudioManager::addAudioEvent(VoiceCreated)", &[i64::from(t), i64::from(o)]);
    }
    fn player_type(&self, p: PlayerIndex) -> PlayerType {
        self.players.get_nth(p).map_or(PlayerType::Human, |x| x.player_type)
    }
    fn player_is_skirmish_ai(&self, p: PlayerIndex) -> bool {
        player_is_skirmish_ai_player(self, p).unwrap_or(false)
    }
    fn player_has_default_team(&self, p: PlayerIndex) -> bool {
        self.players.get_nth(p).is_some_and(|x| x.default_team.is_some())
    }
    fn player_relationship_to_object(&self, p: PlayerIndex, them: ObjectId) -> Relationship {
        player_get_relationship(self, p, self.get_team(them)).unwrap_or(Relationship::Neutral)
    }
    fn player_can_build(&mut self, p: PlayerIndex, t: TemplateRef) -> bool {
        player_can_build(self, p, Some(t)).unwrap_or(false)
    }
    fn player_can_build_more_of_type(&mut self, p: PlayerIndex, t: TemplateRef) -> bool {
        player_can_build_more_of_type(self, p, t).unwrap_or(false)
    }
    fn player_count_money(&self, p: PlayerIndex) -> u32 {
        self.players.get_nth(p).map_or(0, |x| x.money.count_money())
    }
    fn player_deposit(&mut self, p: PlayerIndex, amount: u32) {
        // `getMoney()->deposit(amount)`: the cash sound is the default
        let _ = player_money_deposit(self, p, amount, true);
    }
    fn player_on_structure_created(&mut self, _p: PlayerIndex, _constructor: Option<ObjectId>, _o: ObjectId) {
        // `Player::onStructureCreated` is empty upstream
    }
    fn player_on_structure_construction_complete(&mut self, p: PlayerIndex, constructor: Option<ObjectId>, o: ObjectId) {
        let _ = player_on_structure_construction_complete(self, p, constructor, o, false);
    }
    fn player_on_unit_created(&mut self, p: PlayerIndex, constructor: Option<ObjectId>, o: ObjectId) {
        let _ = player_on_unit_created(self, p, constructor, Some(o));
    }
    fn terrain_ground_height(&self, x: f32, y: f32) -> f32 {
        self.terrain.ground_height(x, y, false).0
    }
    fn terrain_extent(&self) -> Region3D {
        self.terrain.extent().unwrap_or_default()
    }
    fn terrain_maximum_pathfind_extent(&self) -> Region3D {
        self.terrain.maximum_pathfind_extent().unwrap_or_default()
    }
    fn terrain_layer_for_destination(&mut self, pos: Coord3D) -> PathfindLayer {
        self.with_terrain(|t, w| t.layer_for_destination(w, &pos)).unwrap_or(crate::common::game_type::LAYER_GROUND)
    }
    fn terrain_tile_restricts_construction(&self, x: f32, y: f32) -> Option<bool> {
        // (batch I59, #16: with `live_terrain` and a map with `BlendTileData`, the tile's terrain type: `getTerrainNameAt`, then
        // `TheTerrainTypes->findTerrain(name)->getRestrictConstruction()`; no tile, no restriction, as upstream's NULL. A map
        // without the chunk keeps the tests' table)
        if self.cuts.live_terrain && self.terrain.logic_height_map().is_some_and(|m| m.blend_tiles().is_some()) {
            let name = self.terrain.terrain_name_at(x, y)?;
            return self.statics.terrain_types.find_terrain(&name).map(|t| t.restrict_construction());
        }
        let cell = crate::logic::world::object_cuts::terrain_cell(x, y);
        Some(self.cuts.restricted_construction_cells.contains(&cell))
    }
    fn terrain_visual_add_faction_bib(&mut self, o: ObjectId, highlight: bool, extra: f32) {
        self.ba_cut("TerrainVisual::addFactionBib", &[i64::from(o), i64::from(highlight), i64::from(extra.to_bits())]);
    }
    fn terrain_visual_remove_trees_and_props(&mut self, pos: Coord3D, _geom: &GeometryInfo, angle: f32) {
        self.ba_cut("TerrainVisual::removeTreesAndPropsForConstruction", &[i64::from(pos.x.to_bits()), i64::from(pos.y.to_bits()), i64::from(angle.to_bits())]);
    }
    fn partition_iterate_potential_collisions(&mut self, pos: Coord3D, geom: &GeometryInfo, angle: f32) -> Vec<ObjectId> {
        self.partition_iterate_potential_collisions_at(pos, geom, angle)
    }
    fn partition_structures_in_range(&mut self, pos: Coord3D, range: f32) -> Vec<ObjectId> {
        if !self.cuts.live_partition {
            self.ba_cut("PartitionManager::iterateObjectsInRange(STRUCTURE)", &[]);
            return self.cuts.range_scan.clone();
        }
        let filters = [PartitionFilter::AcceptByKindOf { must_be_set: KindOfMaskType::from_indices(&[KINDOF_STRUCTURE]), must_be_clear: KindOfMaskType::new() }];
        let it = self.with_partition(|pm, w| {
            pm.iterate_objects_in_range_at(w, &pos, range, DistanceCalculationType::FromBoundingSphere2D, &filters, crate::logic::object_iter::IterOrderType::Fastest)
        });
        crate::logic::world::player_host_world::iterator_ids(it)
    }
    fn partition_closest_supply_source(&mut self, pos: Coord3D, range: f32) -> Option<ObjectId> {
        if !self.cuts.live_partition {
            self.ba_cut("PartitionManager::getClosestObject(SUPPLY_SOURCE)", &[]);
            return self.cuts.range_scan.first().copied();
        }
        let filters = [PartitionFilter::AcceptByKindOf { must_be_set: KindOfMaskType::from_indices(&[KINDOF_SUPPLY_SOURCE]), must_be_clear: KindOfMaskType::new() }];
        self.with_partition(|pm, w| pm.get_closest_object_at(w, &pos, range, DistanceCalculationType::FromBoundingSphere2D, &filters).object)
    }
    fn partition_geom_collides_with_geom(&self, pos1: Coord3D, geom1: &GeometryInfo, angle1: f32, pos2: Coord3D, geom2: &GeometryInfo, angle2: f32) -> bool {
        crate::logic::world::partition::collide::geom_collides_with_geom(&pos1, geom1, angle1, &pos2, geom2, angle2)
    }
    fn partition_world_to_cell(&self, x: f32, y: f32) -> (i32, i32) {
        self.partition.world_to_cell(x, y)
    }
    fn partition_shroud_status_for_player(&self, player_index: i32, x: i32, y: i32) -> CellShroudStatus {
        self.partition.get_shroud_status_for_player_at_cell(player_index, x, y)
    }
    fn pathfinder_cell_type(&self, layer: PathfindLayer, x: i32, y: i32) -> Option<PathfindCellType> {
        // (batch I9a: the world's pathfinder with `live_pathfinder`)
        let t = match self.pathfinder_cell_type(layer, x, y) {
            Some(cell) => cell?,
            None => self.cuts.contain.cell_types[crate::logic::world::object_cuts::terrain_cell(x as f32, y as f32)],
        };
        match t {
            0 => Some(PathfindCellType::Clear),
            1 => Some(PathfindCellType::Water),
            2 => Some(PathfindCellType::Cliff),
            3 => Some(PathfindCellType::Rubble),
            4 => Some(PathfindCellType::Obstacle),
            5 => Some(PathfindCellType::BridgeImpassable),
            6 => Some(PathfindCellType::Impassable),
            _ => None,
        }
    }
}

// --- p1w13-sonnet --- (integration batch I51)
impl World {
    /// `obj->getProductionUpdateInterface()->getSpecialPowerConstructionCommandButton()`'s name: the first `ProductionUpdate`'s.
    pub(crate) fn production_special_power_construction_button(&self, o: ObjectId) -> Option<Vec<u8>> {
        let x = self.find_object(o)?;
        (0..x.modules.len() as ModuleIndex)
            .find_map(|i| match self.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: o, index: i }) {
                Some(UpdateBody::ProductionUpdate(p)) => Some(p.special_power_construction_command_button().map(<[u8]>::to_vec)),
                _ => None,
            })
            .flatten()
    }
}
// --- end p1w13-sonnet ---

#[cfg(test)]
mod tests {
    //! The assistant itself is checked against the compiled BuildAssistant.cpp by `tests/differential_build_assistant.rs`
    //! with a recording host; this checks a sale on the world, from the player's `sellObject` to the refund.
    use super::*;
    use crate::common::object_status_types::OBJECT_STATUS_SOLD;
    use crate::logic::world::player_host_world::tests::{make, world};

    /// `Player`'s `sellObject` puts the barracks on the sell list (sold, 99.9 % built); from the frame 45 frames after the
    /// sale (the scaffold's) the construction runs down by 100/90 a frame, and at -50 % (frame 179) the player is refunded the build cost (600, at a sell
    /// percentage of 1) and the barracks is destroyed. A tank (not a structure) is not sold. With `live_teams`, so the
    /// barracks' controlling player (whom the refund goes to) is its team's.
    #[test]
    fn a_sold_structure_sinks_and_refunds_its_cost() {
        let (mut w, team) = world();
        // the object code's controlling player is the team arena's (batch I3b)
        w.cuts.live_teams = true;
        let barracks = make(&mut w, b"Barracks", team, Coord3D { x: 200.0, y: 200.0, z: 0.0 });
        let tank = make(&mut w, b"Tank", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        crate::logic::world::player_objects::PlayerObjectHost::build_assistant_sell_object(&mut w, tank);
        crate::logic::world::player_objects::PlayerObjectHost::build_assistant_sell_object(&mut w, barracks);
        assert_eq!(w.build_assistant.sell_list.iter().map(|s| s.id).collect::<Vec<_>>(), vec![barracks]);
        assert!(w.find_object(barracks).unwrap().test_status(OBJECT_STATUS_SOLD));
        assert_eq!(w.find_object(barracks).unwrap().construction_percent, 99.9);
        let mut frame = 0;
        while w.find_object(barracks).is_some_and(|o| !o.is_destroyed()) && frame < 400 {
            frame += 1;
            w.set_frame(frame);
            w.build_assistant_update();
            if frame == 45 {
                assert_eq!(w.find_object(barracks).unwrap().construction_percent, 99.9 - 100.0 / 90.0);
            }
        }
        // 99.9 - (f - 44) * 100/90 <= -50 first at f - 44 = 135: frame 179
        assert_eq!(frame, 179);
        assert!(w.build_assistant.sell_list.is_empty());
        assert_eq!(w.players.get_nth(1).unwrap().money.count_money(), 600);
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
