// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptConditions.cpp (the calls the conditions make out of
//   `ScriptConditions`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I8b: `TheScriptConditions` in the world ([`World::script_conditions`]) and the world as
//! `ScriptConditionsHost`. The engine's `evaluateCondition` and its two direct questions (batch I8a's cut points) are
//! now the conditions'. The engine is checked out of the world while it runs its scripts and hands itself to the
//! conditions; the world puts it back for the evaluation (`getTeamNamed`, `getUnitNamed`, the completions are its), and
//! takes it out again after.
//!
//! **Signature change:** the conditions' host methods take `&mut self`, except the five pure lookups the conditions call
//! inside closures or twice in one expression (`sc_find_template`, `sc_template_name`, `sc_obj_template`,
//! `sc_special_power_type`, `sc_obj_has_special_power`); `evaluateSkirmishHasPrereqsToBuild`'s `canBuildAny` is a loop
//! over the list (the same order and the same first match) so `canBuild` can take the world mutably; the functions of `script_conditions.rs`
//! take the host as `&mut H`. The world answers through module calls (health, last damage, containers, special power
//! modules, warehouses) and the partition manager's queries, which need the world mutably. The differential host follows,
//! and `differential_script_conditions_c1` is unchanged.
//!
//! - **The world:** the players (masks, objects, build facilities, money, buildings, kinds, energy, teams, counts by
//!   template, science points and purchases, side, supply box value, start index, attackers), the script engine's
//!   lookups and notifications, the team factory and teams (trigger questions, state, creation, members), the objects
//!   and their modules, the thing factory, the special power store, the terrain's triggers and bridges, the partition
//!   manager (with `live_partition`), and the victory conditions.
//! - **Cut points** (tables): the AI player's supply questions and the AI's completed waypoint (batch I9), the control
//!   bar's buttons (`cuts.command_buttons`, with the new `upgrade` and `ready_for` fields), the multiplayer session, the
//!   in-game UI's selection (`cuts.selected_drawables`), the camera and the music.
//!
//! `p1w13-sonnet`, integration batch I8b, 2026-11-01.

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::{Coord3D, ICoord3D};
use crate::common::disabled_types::DisabledType;
use crate::common::game_common::ObjectShroudStatus;
use crate::common::ini::IniError;
use crate::common::kind_of::{KindOfMaskType, KindOfType, KINDOF_STRUCTURE, KINDOF_TECH_BUILDING};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::science::ScienceType;
use crate::logic::object_types::ObjectTypes;
use crate::logic::scripts::Condition;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::ObjectId;
use crate::logic::world::module_call::{ModuleCall, ModuleReply};
use crate::logic::world::partition::{DistanceCalculationType, PartitionFilter};
use crate::logic::world::player_ops as po;
use crate::logic::world::script_conditions::{
    ButtonH, ContainInfo, LastDamage, ObjH, PlayerH, ScriptConditionsHost, SpecialPowerH, SpecialPowerModuleInfo, TeamH, TemplateH, TriggerH,
};
use crate::logic::world::script_engine::ScriptEngine;
use crate::logic::world::team_ops as to;

type R<T> = Result<T, IniError>;

/// An `AsciiString` of bytes (a byte that makes none: empty).
fn ascii(b: &[u8]) -> AsciiString {
    AsciiString::try_from_bytes(b).unwrap_or_default()
}

impl World {
    /// `TheScriptConditions->evaluateCondition(condition)`, with the engine that is evaluating it put back in the world for
    /// the conditions' questions to it.
    pub(crate) fn evaluate_condition_with(&mut self, engine: &mut ScriptEngine, condition: &mut Condition) -> R<bool> {
        std::mem::swap(&mut self.script_engine, engine);
        let mut sc = std::mem::take(&mut self.script_conditions);
        let r = sc.evaluate_condition(self, condition);
        self.script_conditions = sc;
        std::mem::swap(&mut self.script_engine, engine);
        r
    }

    /// The same for one of the conditions' direct questions (`f` gets the conditions and the world).
    pub(crate) fn with_conditions_and_engine<T>(&mut self, engine: &mut ScriptEngine, f: impl FnOnce(&crate::logic::world::script_conditions::ScriptConditions, &mut World) -> R<T>) -> R<T> {
        std::mem::swap(&mut self.script_engine, engine);
        let sc = std::mem::take(&mut self.script_conditions);
        let r = f(&sc, self);
        self.script_conditions = sc;
        std::mem::swap(&mut self.script_engine, engine);
        r
    }

    /// The trigger area a handle (its id) names.
    fn sc_trigger(&self, trig: TriggerH) -> Option<&crate::logic::terrain_logic::TriggerArea> {
        self.terrain.triggers().iter().find(|t| t.id() as u32 == trig)
    }

    /// `PolygonTrigger::updateBounds()` of a trigger area: its bounds and radius (see `game_logic_save_host_world`).
    fn sc_trigger_bounds(&self, trig: TriggerH) -> Option<(crate::common::base_type::IRegion2D, f32)> {
        let t = self.sc_trigger(trig)?;
        let points: Vec<ICoord3D> = (0..t.num_points()).map(|i| *t.point(i)).collect();
        Some(crate::logic::world::game_logic_save_host_world::polygon_bounds(&points))
    }

    /// A question to an object's body (`getBodyModule()`).
    fn sc_body_ask(&mut self, o: ObjH, call: ModuleCall) -> Option<ModuleReply> {
        let b = self.get_body_module(o)?;
        Some(self.module_call(o, b, call))
    }
}

impl ScriptConditionsHost for World {
    fn sc_player_from_mask(&mut self, mask: u32) -> Option<PlayerH> {
        crate::logic::world::player_list::player_list_get_player_from_mask(self, mask as u16)
    }
    fn sc_neutral_player(&mut self) -> Option<PlayerH> {
        self.players.get_nth(0).map(|p| p.player_index)
    }
    fn sc_player_mask(&mut self, p: PlayerH) -> u32 {
        1u32.checked_shl(p as u32).unwrap_or(0)
    }
    fn sc_player_has_any_objects(&mut self, p: PlayerH) -> bool {
        po::player_has_any_objects(self, p).unwrap_or(false)
    }
    fn sc_player_has_any_build_facility(&mut self, p: PlayerH) -> bool {
        po::player_has_any_build_facility(self, p).unwrap_or(false)
    }
    fn sc_player_money(&mut self, p: PlayerH) -> u32 {
        self.players.get_nth(p).map_or(0, |x| x.money.count_money())
    }
    fn sc_player_count_buildings(&mut self, p: PlayerH) -> i32 {
        po::player_count_buildings(self, p).unwrap_or(0)
    }
    fn sc_player_count_objects(&mut self, p: PlayerH, must: &KindOfMaskType, must_not: &KindOfMaskType) -> i32 {
        po::player_count_objects(self, p, must, must_not).unwrap_or(0)
    }
    fn sc_player_has_sufficient_power(&mut self, p: PlayerH) -> bool {
        let f = self.frame();
        self.players.get_nth(p).is_some_and(|x| x.energy.has_sufficient_power(f))
    }
    fn sc_player_energy_supply_ratio(&mut self, p: PlayerH) -> f32 {
        let f = self.frame();
        self.players.get_nth(p).map_or(0.0, |x| x.energy.energy_supply_ratio(f))
    }
    fn sc_player_energy_production(&mut self, p: PlayerH) -> i32 {
        let f = self.frame();
        self.players.get_nth(p).map_or(0, |x| x.energy.production(f))
    }
    fn sc_player_energy_consumption(&mut self, p: PlayerH) -> i32 {
        self.players.get_nth(p).map_or(0, |x| x.energy.consumption())
    }
    fn sc_player_teams(&mut self, p: PlayerH) -> Vec<Vec<TeamH>> {
        let protos = self.players.get_nth(p).map(|x| x.team_prototypes.clone()).unwrap_or_default();
        protos.into_iter().map(|pr| self.teams.instances(pr)).collect()
    }
    fn sc_player_count_objects_by_thing_template(&mut self, p: PlayerH, templates: &[TemplateH], ignore_dead: bool) -> Vec<i32> {
        let things: Vec<Option<u32>> = templates.iter().map(|t| Some(*t)).collect();
        let mut counts = vec![0; things.len()];
        let _ = po::player_count_objects_by_thing_template(self, p, &things, ignore_dead, &mut counts, false);
        counts
    }
    fn sc_player_science_purchase_points(&mut self, p: PlayerH) -> i32 {
        self.players.get_nth(p).map_or(0, |x| x.science_purchase_points)
    }
    fn sc_player_is_capable_of_purchasing_science(&mut self, p: PlayerH, science: ScienceType) -> bool {
        po::player_is_capable_of_purchasing_science(self, p, science).unwrap_or(false)
    }
    fn sc_player_side(&mut self, p: PlayerH) -> AsciiString {
        ascii(&self.players.get_nth(p).map(|x| x.side.clone()).unwrap_or_default())
    }
    fn sc_player_supply_box_value(&mut self, _p: PlayerH) -> i32 {
        po::player_get_supply_box_value(self) as i32
    }
    fn sc_player_is_supply_source_safe(&mut self, p: PlayerH, min_supplies: i32) -> bool {
        crate::logic::world::player_ops::PlayerHost::ai_is_supply_source_safe(self, p, min_supplies)
    }
    fn sc_player_is_supply_source_attacked(&mut self, p: PlayerH) -> bool {
        crate::logic::world::player_ops::PlayerHost::ai_is_supply_source_attacked(self, p)
    }
    fn sc_player_mp_start_index(&mut self, p: PlayerH) -> i32 {
        self.players.get_nth(p).map_or(0, |x| x.mp_start_index)
    }
    fn sc_player_attacked_by(&mut self, p: PlayerH, index: i32) -> bool {
        self.players.get_nth(p).is_some_and(|x| usize::try_from(index).ok().and_then(|i| x.attacked_by.get(i)).copied().unwrap_or(false))
    }
    fn sc_player_from_name(&mut self, name: &[u8]) -> Option<PlayerH> {
        let se = std::mem::take(&mut self.script_engine);
        let r = se.get_player_from_ascii_string(self, name);
        self.script_engine = se;
        r
    }
    fn sc_team_named(&mut self, name: &[u8]) -> Option<TeamH> {
        match self.script_engine.get_team_named(self, name) {
            Ok(t) => t.map(|t| t as TeamH),
            Err(e) => {
                self.cuts.team_live_errors.push(format!("ScriptEngine::getTeamNamed: {e:?}"));
                None
            }
        }
    }
    fn sc_unit_named(&mut self, name: &[u8]) -> Option<ObjH> {
        self.script_engine.get_unit_named(name)
    }
    fn sc_did_unit_exist(&mut self, name: &[u8]) -> bool {
        self.script_engine.did_unit_exist(name)
    }
    fn sc_trigger_named(&mut self, name: &[u8]) -> Option<TriggerH> {
        crate::logic::world::script_engine::ScriptEngine::qualified_trigger_area_by_name(self.script_engine.current_player, self, name).map(|t| t as TriggerH)
    }
    fn sc_frame_object_count_changed(&mut self) -> u32 {
        self.script_engine.frame_object_count_changed
    }
    fn sc_object_types(&mut self, name: &AsciiString) -> Option<ObjectTypes> {
        let i = self.script_engine.get_object_types(name.str())?;
        self.script_engine.all_object_type_lists.get(i).cloned()
    }
    fn sc_is_video_complete(&mut self, name: &[u8], remove: bool) -> bool {
        self.script_engine.is_video_complete(name, remove)
    }
    fn sc_is_speech_complete(&mut self, name: &[u8], remove: bool) -> bool {
        let mut se = std::mem::take(&mut self.script_engine);
        let r = se.is_sound_complete(self, name, remove, false);
        self.script_engine = se;
        r
    }
    fn sc_is_audio_complete(&mut self, name: &[u8], remove: bool) -> bool {
        let mut se = std::mem::take(&mut self.script_engine);
        let r = se.is_sound_complete(self, name, remove, true);
        self.script_engine = se;
        r
    }
    fn sc_is_special_power_triggered(&mut self, index: i32, name: &[u8], remove: bool, source: ObjectId) -> bool {
        self.script_engine.is_power_done(0, index, name, remove, source)
    }
    fn sc_is_special_power_midway(&mut self, index: i32, name: &[u8], remove: bool, source: ObjectId) -> bool {
        self.script_engine.is_power_done(1, index, name, remove, source)
    }
    fn sc_is_special_power_complete(&mut self, index: i32, name: &[u8], remove: bool, source: ObjectId) -> bool {
        self.script_engine.is_power_done(2, index, name, remove, source)
    }
    fn sc_is_upgrade_complete(&mut self, index: i32, name: &[u8], remove: bool, source: ObjectId) -> bool {
        self.script_engine.is_power_done(3, index, name, remove, source)
    }
    fn sc_is_science_acquired(&mut self, index: i32, science: ScienceType, remove: bool) -> bool {
        self.script_engine.is_science_acquired(index, science.0, remove)
    }
    fn sc_object_count(&mut self, index: i32, name: &[u8]) -> i32 {
        self.script_engine.get_object_count(self, index, name).unwrap_or(0)
    }
    fn sc_set_object_count(&mut self, index: i32, name: &[u8], count: i32) {
        let _ = self.script_engine.set_object_count(index, name, count);
    }
    fn sc_team_prototype_instances(&mut self, name: &[u8]) -> Option<Vec<TeamH>> {
        let p = self.teams.prototypes().find(|p| p.name == name)?.id;
        Some(self.teams.instances(p))
    }
    fn sc_team_has_any_objects(&mut self, t: TeamH) -> bool {
        to::team_has_any_objects(self, t)
    }
    fn sc_team_has_any_units(&mut self, t: TeamH) -> bool {
        to::team_has_any_units(self, t)
    }
    fn sc_team_name(&mut self, t: TeamH) -> AsciiString {
        let teams = &self.teams;
        ascii(&teams.team(t).and_then(|x| teams.prototype(x.proto)).map(|p| p.name.clone()).unwrap_or_default())
    }
    fn sc_team_some_inside_some_outside(&mut self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        to::team_some_inside_some_outside(self, t, trig, which).unwrap_or(false)
    }
    fn sc_team_all_inside(&mut self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        to::team_all_inside(self, t, trig, which).unwrap_or(false)
    }
    fn sc_team_did_all_enter(&mut self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        to::team_did_all_enter(self, t, trig, which).unwrap_or(false)
    }
    fn sc_team_did_partial_enter(&mut self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        to::team_did_partial_enter(self, t, trig, which).unwrap_or(false)
    }
    fn sc_team_did_all_exit(&mut self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        to::team_did_all_exit(self, t, trig, which).unwrap_or(false)
    }
    fn sc_team_did_partial_exit(&mut self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        to::team_did_partial_exit(self, t, trig, which).unwrap_or(false)
    }
    fn sc_team_did_enter_or_exit(&mut self, t: TeamH) -> bool {
        self.teams.team(t).is_some_and(|x| x.entered_or_exited)
    }
    fn sc_team_state(&mut self, t: TeamH) -> AsciiString {
        ascii(&self.teams.team(t).map(|x| x.state.clone()).unwrap_or_default())
    }
    fn sc_team_is_created(&mut self, t: TeamH) -> bool {
        self.teams.team(t).is_some_and(|x| x.created)
    }
    fn sc_team_controlling_player(&mut self, t: TeamH) -> Option<PlayerH> {
        to::team_get_controlling_player(self, t).ok().flatten()
    }
    fn sc_team_members(&mut self, t: TeamH) -> Vec<ObjH> {
        self.teams.members(t)
    }
    fn sc_find_object(&mut self, id: ObjectId) -> Option<ObjH> {
        self.find_object(id).map(|o| o.id)
    }
    fn sc_obj_is_effectively_dead(&mut self, o: ObjH) -> bool {
        self.find_object(o).is_some_and(|x| x.is_effectively_dead())
    }
    fn sc_obj_position(&mut self, o: ObjH) -> Coord3D {
        self.find_object(o).map(|x| x.get_position()).unwrap_or_default()
    }
    fn sc_obj_template(&self, o: ObjH) -> Option<TemplateH> {
        self.find_object(o).map(|x| TemplateH::from(x.template().get_template_id()))
    }
    fn sc_obj_is_kind_of(&mut self, o: ObjH, kind: KindOfType) -> bool {
        self.find_object(o).is_some_and(|x| x.is_kind_of(kind))
    }
    fn sc_obj_is_inside(&mut self, o: ObjH, trig: TriggerH) -> bool {
        self.is_inside(o, Some(trig as i32))
    }
    fn sc_obj_did_enter(&mut self, o: ObjH, trig: TriggerH) -> bool {
        self.did_enter(o, Some(trig as i32))
    }
    fn sc_obj_did_exit(&mut self, o: ObjH, trig: TriggerH) -> bool {
        self.did_exit(o, Some(trig as i32))
    }
    fn sc_obj_has_body(&mut self, o: ObjH) -> bool {
        self.get_body_module(o).is_some()
    }
    fn sc_obj_health(&mut self, o: ObjH) -> (f32, f32) {
        let h = self.sc_body_ask(o, ModuleCall::GetHealth).map_or(0.0, |r| r.as_real(0.0));
        let i = self.sc_body_ask(o, ModuleCall::GetInitialHealth).map_or(0.0, |r| r.as_real(0.0));
        (h, i)
    }
    fn sc_obj_last_damage(&mut self, o: ObjH) -> Option<LastDamage> {
        match self.sc_body_ask(o, ModuleCall::GetLastDamageInfo)? {
            ModuleReply::Damage(d) => Some(LastDamage {
                source_template: World::thing_factory_find_template(self, &d.input.source_template).map(TemplateH::from),
                source_id: d.input.source_id,
                source_player_mask: u32::from(d.input.source_player_mask),
            }),
            _ => None,
        }
    }
    fn sc_obj_controlling_player(&mut self, o: ObjH) -> Option<PlayerH> {
        self.get_controlling_player(o)
    }
    fn sc_obj_contain(&mut self, o: ObjH) -> Option<ContainInfo> {
        let c = self.get_contain(o)?;
        let player_who_entered = self.module_call(o, c, ModuleCall::GetPlayerWhoEntered).as_int(0) as u32;
        let count = self.module_call(o, c, ModuleCall::GetContainCount).as_int(0) as u32;
        let max = self.module_call(o, c, ModuleCall::GetContainMax).as_int(0) as u32;
        let garrisonable = self.module_call(o, c, ModuleCall::IsGarrisonable).as_bool();
        Some(ContainInfo { player_who_entered, count, max, garrisonable })
    }
    fn sc_obj_status(&mut self, o: ObjH) -> ObjectStatusMaskType {
        self.get_status_bits(o)
    }
    fn sc_obj_is_disabled_by_type(&mut self, o: ObjH, t: DisabledType) -> bool {
        self.find_object(o).is_some_and(|x| x.is_disabled_by_type(t))
    }
    fn sc_obj_is_disabled(&mut self, o: ObjH) -> bool {
        self.find_object(o).is_some_and(|x| x.is_disabled())
    }
    fn sc_obj_shrouded_status(&mut self, o: ObjH, index: i32) -> ObjectShroudStatus {
        self.get_shrouded_status(o, index)
    }
    fn sc_obj_completed_waypoint(&mut self, o: ObjH) -> Option<Option<[AsciiString; 3]>> {
        // the AI's (batch I9): an object with an AI has completed none
        self.find_object(o).and_then(|x| x.ai).map(|_| None)
    }
    fn sc_obj_is_contained(&mut self, o: ObjH) -> bool {
        self.is_contained(o)
    }
    fn sc_obj_is_captured(&mut self, o: ObjH) -> bool {
        self.find_object(o).is_some_and(|x| x.is_captured())
    }
    fn sc_obj_special_power_module(&mut self, o: ObjH, power: SpecialPowerH) -> Option<SpecialPowerModuleInfo> {
        let m = self.get_special_power_module(o, Some(power))?;
        let ready = self.module_call(o, m, ModuleCall::IsReady).as_bool();
        let ready_frame = self.module_call(o, m, ModuleCall::GetReadyFrame).as_int(0) as u32;
        Some(SpecialPowerModuleInfo { ready, ready_frame })
    }
    fn sc_obj_has_special_power(&self, o: ObjH, power_type: i32) -> bool {
        self.find_object(o).is_some_and(|x| x.has_special_power(power_type))
    }
    fn sc_obj_warehouse_boxes(&mut self, o: ObjH) -> Option<i32> {
        let m = self.find_module(o, b"SupplyWarehouseDockUpdate")?;
        Some(self.module_call(o, m, ModuleCall::SupplyWarehouseGetBoxesStored).as_int(0))
    }
    fn sc_find_template(&self, name: &AsciiString) -> Option<TemplateH> {
        World::thing_factory_find_template(self, name.str()).map(TemplateH::from)
    }
    fn sc_template_name(&self, t: TemplateH) -> AsciiString {
        ascii(&u16::try_from(t).ok().and_then(|t| self.statics.thing_factory.find_by_template_id(t)).map(|o| o.data.get_name().to_vec()).unwrap_or_default())
    }
    fn sc_template_build_cost(&mut self, t: TemplateH) -> i32 {
        u16::try_from(t).ok().and_then(|t| self.statics.thing_factory.find_by_template_id(t)).map_or(0, |o| o.get_final_override().data.friend_get_build_cost())
    }
    fn sc_player_can_build(&mut self, p: PlayerH, t: TemplateH) -> bool {
        crate::logic::world::player_objects::player_can_build(self, p, Some(t)).unwrap_or(false)
    }
    fn sc_find_special_power(&mut self, name: &[u8]) -> Option<SpecialPowerH> {
        self.statics.special_power_store.find_special_power_template(name).map(|t| t.id())
    }
    fn sc_can_use_special_power(&mut self, o: ObjH, power: SpecialPowerH) -> bool {
        self.cuts.special_power_can_use(o, Some(power))
    }
    fn sc_special_power_type(&self, power: SpecialPowerH) -> i32 {
        self.statics.special_power_store.find_special_power_template_by_id(power).map_or(-1, |t| t.special_power_type())
    }
    fn sc_science_from_name(&mut self, name: &[u8]) -> ScienceType {
        let store = self.statics.clone();
        store.science_store.get_science_from_internal_name(&mut self.names, name).unwrap_or(crate::common::science::SCIENCE_INVALID)
    }
    fn sc_find_command_button(&mut self, name: &[u8]) -> Option<ButtonH> {
        self.cuts.command_buttons.iter().position(|b| b.name == name).map(|i| i as ButtonH)
    }
    fn sc_button_special_power(&mut self, b: ButtonH) -> Option<SpecialPowerH> {
        self.cuts.command_buttons.get(b as usize).and_then(|x| x.special_power)
    }
    fn sc_button_has_upgrade(&mut self, b: ButtonH) -> bool {
        self.cuts.command_buttons.get(b as usize).is_some_and(|x| x.upgrade.is_some())
    }
    fn sc_button_is_ready(&mut self, b: ButtonH, o: ObjH) -> bool {
        self.cuts.command_buttons.get(b as usize).is_some_and(|x| x.ready_for.contains(&o))
    }
    fn sc_trigger_point_in(&mut self, trig: TriggerH, p: ICoord3D) -> bool {
        self.sc_trigger(trig).is_some_and(|t| t.point_in_trigger(&p))
    }
    fn sc_trigger_center(&mut self, trig: TriggerH) -> Coord3D {
        // `getCenterPoint`: the middle of `updateBounds`' bounds, on the ground
        let Some((b, _)) = self.sc_trigger_bounds(trig) else { return Coord3D::default() };
        let x = (b.lo.x + b.hi.x) as f32 / 2.0;
        let y = (b.lo.y + b.hi.y) as f32 / 2.0;
        Coord3D { x, y, z: self.terrain.ground_height(x, y, false).0 }
    }
    fn sc_trigger_radius(&mut self, trig: TriggerH) -> f32 {
        // `getRadius`: `updateBounds`' radius
        self.sc_trigger_bounds(trig).map_or(0.0, |(_, r)| r)
    }
    fn sc_partition_seen_by(&mut self, o: ObjH, relation: Option<i32>) -> Vec<ObjH> {
        if !self.cuts.live_partition {
            return self.cuts.range_scan.clone();
        }
        let range = self.get_vision_range(o);
        let mut filters = Vec::new();
        if let Some(flags) = relation {
            filters.push(PartitionFilter::Relationship { obj: o, flags });
        }
        filters.push(PartitionFilter::Alive);
        filters.push(PartitionFilter::StealthedAndUndetected { obj: o, allow: false });
        filters.push(PartitionFilter::SameMapStatus { obj: o });
        let it = self.with_partition(|pm, w| pm.iterate_objects_in_range(w, o, range, DistanceCalculationType::FromCenter2D, &filters, crate::logic::object_iter::IterOrderType::Fastest));
        crate::logic::world::player_host_world::iterator_ids(it)
    }
    fn sc_partition_supply_structures(&mut self, center: Coord3D, distance: f32, p: PlayerH) -> Vec<ObjH> {
        if !self.cuts.live_partition {
            return self.cuts.range_scan.clone();
        }
        let filters = [
            PartitionFilter::AcceptByKindOf { must_be_set: KindOfMaskType::from_indices(&[KINDOF_STRUCTURE]), must_be_clear: KindOfMaskType::new() },
            PartitionFilter::PlayerAffiliation { player: p, affiliation: crate::logic::world::partition::filters::allow_player::NEUTRAL, match_: true },
            PartitionFilter::OnMap,
        ];
        let it = self.with_partition(|pm, w| pm.iterate_objects_in_range_at(w, &center, distance, DistanceCalculationType::FromCenter2D, &filters, crate::logic::object_iter::IterOrderType::Fastest));
        crate::logic::world::player_host_world::iterator_ids(it)
    }
    fn sc_partition_tech_building_near(&mut self, center: Coord3D, radius: f32, p: PlayerH) -> bool {
        if !self.cuts.live_partition {
            return !self.cuts.range_scan.is_empty();
        }
        let filters = [
            PartitionFilter::AcceptByKindOf { must_be_set: KindOfMaskType::from_indices(&[KINDOF_TECH_BUILDING]), must_be_clear: KindOfMaskType::new() },
            PartitionFilter::PlayerAffiliation { player: p, affiliation: crate::logic::world::partition::filters::allow_player::ALLIES, match_: false },
            PartitionFilter::OnMap,
        ];
        self.with_partition(|pm, w| pm.get_closest_object_at(w, &center, radius, DistanceCalculationType::FromCenter2D, &filters).object.is_some())
    }
    fn sc_any_bridges_damage_states_changed(&mut self) -> bool {
        self.terrain.any_bridges_damage_states_changed()
    }
    fn sc_is_bridge_broken(&mut self, o: ObjH) -> bool {
        self.terrain.is_bridge_broken(Some(o))
    }
    fn sc_is_bridge_repaired(&mut self, o: ObjH) -> bool {
        self.terrain.is_bridge_repaired(Some(o))
    }
    fn sc_frame(&mut self) -> u32 {
        self.frame()
    }
    fn sc_is_multiplayer_session(&mut self) -> bool {
        self.cuts.new_game.multiplayer_session
    }
    fn sc_frame_selection_changed(&mut self) -> u32 {
        self.cuts.frame_selection_changed
    }
    fn sc_selected_object_names(&mut self) -> Vec<Option<AsciiString>> {
        let ids: Vec<ObjectId> = self.cuts.selected_drawables.iter().copied().collect();
        ids.into_iter().map(|o| self.find_object(o).map(|x| ascii(&x.name))).collect()
    }
    fn sc_camera_movement_finished(&mut self) -> bool {
        self.cuts.game_loop.view_camera_movement_finished
    }
    fn sc_is_local_allied_victory(&mut self) -> Result<bool, IniError> {
        self.victory.is_local_allied_victory(self)
    }
    fn sc_is_local_allied_defeat(&mut self) -> Result<bool, IniError> {
        self.victory.is_local_allied_defeat(self)
    }
    fn sc_is_local_defeat(&mut self) -> bool {
        self.victory.is_local_defeat()
    }
    fn sc_music_track_completed(&mut self, name: &[u8], times: i32) -> bool {
        self.cuts.music_tracks_completed.get(name).is_some_and(|n| *n >= times)
    }
}

#[cfg(test)]
mod tests {
    //! The conditions themselves are checked against the compiled ScriptConditions.cpp by
    //! `tests/differential_script_conditions_c1.rs` with a recording host; this checks one evaluated on the world, with the
    //! engine lent back for its player lookup.
    use super::*;
    use crate::logic::scripts::{condition_type as ct, Parameter};
    use crate::logic::world::player_host_world::tests::world;
    use crate::logic::world::script_conditions::comparison::EQUAL;

    fn credits(amount: i32) -> Condition {
        let mut c = Condition::new_for_read();
        c.condition_type = ct::PLAYER_HAS_CREDITS;
        c.num_parms = 3;
        c.parms[0] = Some(Parameter::new(0, amount));
        c.parms[1] = Some(Parameter::new(0, EQUAL));
        let mut side = Parameter::new(0, 0);
        side.string = b"Player1".to_vec();
        c.parms[2] = Some(side);
        c
    }

    /// `PLAYER_HAS_CREDITS` (`Player1`, equal) with player 1 holding 500: true for 500, false for 400. The player is found
    /// by the engine's `getPlayerFromAsciiString` while the engine evaluating the condition is lent back to the world, and
    /// the engine is out of the world again afterwards.
    #[test]
    fn a_condition_is_answered_by_the_world() {
        let (mut w, _) = world();
        w.players.get_nth_mut(1).unwrap().money.deposit(500, false, &mut Vec::new());
        let mut engine = std::mem::take(&mut w.script_engine);
        engine.counters[0].value = 42; // marks this engine
        assert!(w.evaluate_condition_with(&mut engine, &mut credits(500)).unwrap());
        assert!(!w.evaluate_condition_with(&mut engine, &mut credits(400)).unwrap());
        assert_eq!((engine.counters[0].value, w.script_engine.counters[0].value), (42, 0));
    }
}
