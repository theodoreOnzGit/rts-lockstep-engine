// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp (the calls `ScriptEngine` makes out of
//   itself)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I8a: `TheScriptEngine` in the world ([`World::script_engine`]) and the world as
//! `ScriptEngineHost`. `GameLogic::update`'s `TheScriptEngine->update()` and the time freezes it asks about (batch I4's
//! cut points) are now the engine's. The scripts are the sides list's (`World::sides`, each side's `m_scripts`).
//!
//! - **The world:** the frame, the logic's random values, the name keys, the sides' script lists, the player list and
//!   its players (type, name key, start index, mask, difficulty, activity, skirmish AI, default team), the team
//!   factory's prototypes and teams, the objects (names, indicator colours, controlling players, AI, death), the
//!   terrain's trigger areas and waypoints, the partition manager's shroud reveals, the thing factory and templates,
//!   and the science store.
//! - **Cut points** ([`CutCall::Host`], answers in tables): ~~`TheScriptConditions` (batch I8b: a condition is false)~~ (the
//!   world's since batch I8b), `TheScriptActions` (batch I8c: an action is logged), the AI's groups and its enemies (batch I9), the audio, the
//!   campaign, the message stream and the game engine's frame limit.
//!
//! `p1w13-sonnet`, integration batch I8a, 2026-11-01.

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::Coord3D;
use crate::common::game_common::PlayerType;
use crate::common::ini::{IniError, Undefined};
use crate::common::name_key_generator::NameKeyType;
use crate::common::science::ScienceType;
use crate::logic::scripts::{Condition, Parameter, ScriptAction, ScriptList};
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::ObjectId;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::player_ops::{player_get_player_difficulty, player_is_player_active, player_is_skirmish_ai_player};
use crate::logic::world::script_engine::{GroupH, ObjH, PlayerH, ProtoH, ScriptEngine, ScriptEngineHost, TeamH, TemplateH, TriggerH};

type R<T> = Result<T, IniError>;

impl World {
    /// `TheScriptEngine->update()`.
    pub fn script_engine_update(&mut self) -> R<()> {
        let mut se = std::mem::take(&mut self.script_engine);
        let r = se.update(self);
        self.script_engine = se;
        r
    }

    /// `TheScriptEngine->newMap()` (`startNewGame` calls it once the players exist): the engine's counters, flags and
    /// timers set for a new map (until then a constructed engine's update does nothing, as upstream's).
    pub fn script_engine_new_map(&mut self) -> R<()> {
        let mut se = std::mem::take(&mut self.script_engine);
        let r = se.new_map(self);
        self.script_engine = se;
        r
    }

    /// Logs a call of the script engine into a unit the world does not hold.
    fn se_cut(&mut self, call: &'static str, args: &[i64], text: &[u8]) {
        self.cut(CutCall::Host { call, args: args.to_vec(), text: text.to_vec() });
    }
}

impl ScriptEngineHost for World {
    fn se_take_notes(&mut self) -> Vec<crate::logic::world::script_engine::ScriptNote> {
        std::mem::take(&mut self.cuts.pending_script_notes)
    }
    fn se_restore_fps_limit(&mut self) {
        self.se_cut("GameEngine::setFramesPerSecondLimit", &[], &[]);
    }
    fn se_frame(&self) -> u32 {
        self.frame()
    }
    fn se_random_int(&mut self, lo: i32, hi: i32) -> i32 {
        self.random.game_logic_random_value(lo, hi)
    }
    fn se_append_clear_game_data(&mut self) {
        self.se_cut("MessageStream::appendMessage(MSG_CLEAR_GAME_DATA)", &[], &[]);
    }
    fn se_audio_length_ms(&mut self, name: &[u8]) -> f32 {
        self.se_cut("AudioManager::getAudioLengthMS", &[], name);
        self.cuts.audio_lengths_ms.get(name).copied().unwrap_or(0.0)
    }
    fn se_is_challenge_campaign(&self) -> bool {
        self.cuts.new_game.campaign == Some(true)
    }
    fn se_name_key(&mut self, name: &[u8]) -> NameKeyType {
        self.names.name_to_key(name)
    }
    fn se_actions_reset(&mut self) {
        // batch I8c: the world's own actions
        self.script_actions_reset();
    }
    fn se_conditions_reset(&mut self) {
        // batch I8b: the world's own conditions
        self.script_conditions.reset();
    }
    fn se_actions_update(&mut self) {
        // batch I8c: the world's own actions (`update()` is empty upstream)
        self.script_actions.update();
    }
    fn se_conditions_update(&mut self) {
        // batch I8b: the world's own conditions (`update()` is empty upstream)
        self.script_conditions.update();
    }
    fn se_actions_close_windows(&mut self) {
        // batch I8c: `TheScriptActions->closeWindows(FALSE)`
        self.script_actions_close_windows(false);
    }
    fn se_execute_action(&mut self, engine: &mut ScriptEngine, action: &ScriptAction) -> R<()> {
        // batch I8c: the world's own actions
        self.execute_action_with(engine, action)
    }
    fn se_evaluate_condition(&mut self, engine: &mut ScriptEngine, condition: &mut Condition) -> R<bool> {
        // batch I8b: the world's own conditions
        self.evaluate_condition_with(engine, condition)
    }
    fn se_skirmish_command_button_is_ready(&mut self, engine: &mut ScriptEngine, p1: Option<&Parameter>, p2: Option<&Parameter>, all: bool) -> R<bool> {
        // `evaluateSkirmishCommandButtonIsReady(NULL, p1, p2, allReady)`: the parameters as a condition's 1 and 2
        let mut c = Condition::new_for_read();
        c.num_parms = 3;
        c.parms[1] = p1.cloned();
        c.parms[2] = p2.cloned();
        self.with_conditions_and_engine(engine, |sc, w| sc.skirmish_command_button_is_ready(w, &c, 1, 2, all))
    }
    fn se_team_is_contained(&mut self, engine: &mut ScriptEngine, p0: Option<&Parameter>, all: bool) -> R<bool> {
        let mut c = Condition::new_for_read();
        c.num_parms = 1;
        c.parms[0] = p0.cloned();
        self.with_conditions_and_engine(engine, |sc, w| sc.team_is_contained(w, &c, 0, all))
    }
    fn se_num_sides(&self) -> i32 {
        self.sides.get_num_sides()
    }
    fn se_script_list(&self, side: usize) -> Option<&ScriptList> {
        self.sides.get_side_info(side as i32).ok().and_then(|s| s.scripts.as_ref())
    }
    fn se_script_list_mut(&mut self, side: usize) -> Option<&mut ScriptList> {
        self.sides.get_side_info_mut(side as i32).ok().and_then(|s| s.scripts.as_mut())
    }
    fn se_reset_script_lists(&mut self) {
        // `ScriptList::reset()`: the lists read with the map deleted
        for i in 0..self.sides.get_num_sides() {
            if let Ok(s) = self.sides.get_side_info_mut(i) {
                s.scripts = None;
            }
        }
    }
    fn se_nth_player(&self, i: i32) -> Option<PlayerH> {
        self.players.get_nth(i).map(|p| p.player_index)
    }
    fn se_player_count(&self) -> i32 {
        self.players.player_count()
    }
    fn se_local_player(&self) -> Option<PlayerH> {
        (0..16).find(|&i| crate::logic::world::player_ops::player_is_local_player(self, i))
    }
    fn se_find_player_with_name_key(&self, key: NameKeyType) -> Option<PlayerH> {
        self.players.find_with_name_key(key.0 as i32).map(|p| p.player_index)
    }
    fn se_update_team_states(&mut self, engine: &mut ScriptEngine) {
        // (integration batch I30: the running engine lent back to the world for the teams' scripts)
        std::mem::swap(&mut self.script_engine, engine);
        let r = crate::logic::world::player_list::player_list_update_team_states(self);
        std::mem::swap(&mut self.script_engine, engine);
        if let Err(e) = r {
            self.cuts.team_live_errors.push(format!("PlayerList::updateTeamStates: {e:?}"));
        }
    }
    fn se_owned_script(&self, index: usize) -> Option<&crate::logic::scripts::Script> {
        self.team_scripts.owned.get(index)?.as_ref()
    }
    fn se_owned_script_mut(&mut self, index: usize) -> Option<&mut crate::logic::scripts::Script> {
        self.team_scripts.owned.get_mut(index)?.as_mut()
    }
    fn se_player_current_enemy(&mut self, p: PlayerH) -> Option<PlayerH> {
        // (batch I9e: `Player::getCurrentEnemy()`, the AI player's, live with `cuts.live_ai`)
        crate::logic::world::player_ops::player_get_current_enemy(self, p).ok().flatten()
    }
    fn se_player_is_human(&self, p: PlayerH) -> bool {
        self.players.get_nth(p).is_some_and(|x| x.player_type == PlayerType::Human)
    }
    fn se_player_name_key(&self, p: PlayerH) -> NameKeyType {
        NameKeyType(self.players.get_nth(p).map_or(0, |x| x.name_key as u32))
    }
    fn se_player_mp_start_index(&self, p: PlayerH) -> i32 {
        self.players.get_nth(p).map_or(0, |x| x.mp_start_index)
    }
    fn se_player_mask(&self, p: PlayerH) -> u32 {
        1u32.checked_shl(p as u32).unwrap_or(0)
    }
    fn se_player_difficulty(&self, p: PlayerH) -> i32 {
        player_get_player_difficulty(self, p).map_or(1, |d| d as i32)
    }
    fn se_player_is_active(&self, p: PlayerH) -> bool {
        player_is_player_active(self, p).unwrap_or(false)
    }
    fn se_player_is_skirmish_ai(&self, p: PlayerH) -> bool {
        player_is_skirmish_ai_player(self, p).unwrap_or(false)
    }
    fn se_player_default_team(&self, p: PlayerH) -> Option<TeamH> {
        self.players.get_nth(p).and_then(|x| x.default_team).map(|t| t as TeamH)
    }
    fn se_find_team_prototype(&self, name: &[u8]) -> Option<ProtoH> {
        self.teams.prototypes().find(|p| p.name == name).map(|p| p.id as ProtoH)
    }
    fn se_proto_is_singleton(&self, p: ProtoH) -> bool {
        self.teams.prototype(p as u32).is_some_and(|p| p.get_is_singleton())
    }
    fn se_proto_max_instances(&self, p: ProtoH) -> i32 {
        self.teams.prototype(p as u32).map_or(0, |p| p.team_template.max_instances)
    }
    fn se_proto_instances(&self, p: ProtoH) -> Vec<TeamH> {
        self.teams.instances(p as u32).into_iter().map(|t| t as TeamH).collect()
    }
    fn se_team_is_active(&self, t: TeamH) -> bool {
        self.teams.team(t as u32).is_some_and(|t| t.active)
    }
    fn se_team_name(&self, t: TeamH) -> Vec<u8> {
        let teams = &self.teams;
        teams.team(t as u32).and_then(|t| teams.prototype(t.proto)).map(|p| p.name.clone()).unwrap_or_default()
    }
    fn se_team_controlling_player(&self, t: TeamH) -> Option<PlayerH> {
        crate::logic::world::team_ops::team_get_controlling_player(self, t as u32).ok().flatten()
    }
    fn se_team_as_new_ai_group(&mut self, t: TeamH) -> GroupH {
        let g = crate::logic::world::game_logic_update::GameLogicHost::u_create_group(self);
        for o in self.teams.members(t as u32) {
            crate::logic::world::game_logic_update::GameLogicHost::u_group_add(self, g, o as u32);
        }
        g as GroupH
    }
    fn se_group_is_idle(&self, g: GroupH) -> bool {
        // `AIGroup::isIdle()`: every member's AI idle (the AI's tables until batch I9; since I9c-2, with live groups, each
        // member's AI's base `isIdle` on its machine: a subclass's override of `isIdle` is not seen here)
        if self.cuts.live_ai {
            return self.ai_group_members(g as u32).iter().all(|&o| match (self.ai_peek(o), self.ai_machine_query(o)) {
                (Some(a), Some(m)) => a.base_is_idle(m),
                _ => true,
            });
        }
        self.cuts.player_host.groups.get(&(g as u32)).is_none_or(|m| m.iter().all(|o| self.cuts.ai_idle.contains(o)))
    }
    fn se_group_is_ai_dead(&self, g: GroupH) -> bool {
        // `AIGroup::isGroupAiDead()`: no member alive with an AI
        if self.cuts.live_ai {
            return self.ai_group_members(g as u32).iter().all(|o| self.find_object(*o).is_none_or(|x| x.ai.is_none() || x.is_effectively_dead()));
        }
        self.cuts.player_host.groups.get(&(g as u32)).is_none_or(|m| {
            m.iter().all(|o| self.find_object(*o).is_none_or(|x| x.ai.is_none() || x.is_effectively_dead()))
        })
    }
    fn se_find_object(&self, id: ObjectId) -> Option<ObjH> {
        self.find_object(id).map(|o| o.id)
    }
    fn se_objects(&self) -> Vec<ObjH> {
        self.objects.ids_in_order()
    }
    fn se_obj_name(&self, o: ObjH) -> Vec<u8> {
        self.find_object(o).map(|x| x.name.clone()).unwrap_or_default()
    }
    fn se_obj_set_name(&mut self, o: ObjH, name: &[u8]) {
        if let Some(x) = self.find_object_mut(o) {
            x.name = name.to_vec();
        }
    }
    fn se_obj_has_custom_indicator_color(&self, o: ObjH) -> bool {
        self.find_object(o).is_some_and(|x| x.indicator_color != 0)
    }
    fn se_obj_indicator_color(&self, o: ObjH) -> i32 {
        self.get_indicator_color(o)
    }
    fn se_obj_set_custom_indicator_color(&mut self, o: ObjH, color: i32) {
        self.set_custom_indicator_color(o, color);
    }
    fn se_obj_remove_custom_indicator_color(&mut self, o: ObjH) {
        self.remove_custom_indicator_color(o);
    }
    fn se_obj_controlling_player(&self, o: ObjH) -> Option<PlayerH> {
        self.get_controlling_player(o)
    }
    fn se_obj_has_ai(&self, o: ObjH) -> bool {
        self.find_object(o).is_some_and(|x| x.ai.is_some())
    }
    fn se_ai_is_idle(&mut self, o: ObjH) -> bool {
        // (integration batch I25: with `live_ai`, the AI's own (virtual) `isIdle()`; it was the `ai_idle` table, which no live unit is
        // in, so no unit's sequential script ever started)
        if self.cuts.live_ai {
            return self.ai_ask(o, crate::logic::world::module_call::ModuleCall::AiIsIdle).is_some_and(|r| r.as_bool());
        }
        self.cuts.ai_idle.contains(&o)
    }
    fn se_obj_is_effectively_dead(&self, o: ObjH) -> bool {
        self.find_object(o).is_some_and(|x| x.is_effectively_dead())
    }
    fn se_trigger_area_by_name(&self, name: &[u8]) -> Option<TriggerH> {
        let n = AsciiString::try_from_bytes(name).ok()?;
        self.terrain.trigger_area_by_name(&n).and_then(|i| self.terrain.triggers().get(i)).map(|t| t.id())
    }
    fn se_waypoint_location(&self, name: &[u8]) -> Option<Coord3D> {
        let h = self.terrain.waypoint_by_name(&AsciiString::try_from_bytes(name).ok()?)?;
        self.terrain.waypoint(h).map(|w| *w.location())
    }
    fn se_do_shroud_reveal(&mut self, x: f32, y: f32, radius: f32, mask: u32) {
        self.partition_do_shroud_reveal(x, y, radius, mask as u16);
    }
    fn se_undo_shroud_reveal(&mut self, x: f32, y: f32, radius: f32, mask: u32) {
        self.partition_undo_shroud_reveal(x, y, radius, mask as u16);
    }
    fn se_find_template(&self, name: &[u8]) -> Option<TemplateH> {
        World::thing_factory_find_template(self, name).map(TemplateH::from)
    }
    fn se_templates(&self) -> Vec<TemplateH> {
        self.statics.thing_factory.templates().map(|t| TemplateH::from(t.data.get_template_id())).collect()
    }
    fn se_template_is_kind_of(&self, t: TemplateH, kind: i32) -> R<bool> {
        if !(0..crate::common::kind_of::KINDOF_COUNT).contains(&kind) {
            return Err(IniError::undefined(Undefined::ArrayIndexOutOfRange));
        }
        let tt = u16::try_from(t).ok().and_then(|t| self.statics.thing_factory.find_by_template_id(t)).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        Ok(tt.data.is_kind_of(kind))
    }
    fn se_template_final_override(&self, t: TemplateH) -> TemplateH {
        u16::try_from(t).ok().and_then(|id| self.statics.thing_factory.find_by_template_id(id)).map_or(t, |o| TemplateH::from(o.get_final_override().data.get_template_id()))
    }
    fn se_team_exists(&self, id: TeamH) -> bool {
        self.teams.find_team_by_id(id as u32).is_some()
    }
    fn se_template_name(&self, t: TemplateH) -> Vec<u8> {
        u16::try_from(t).ok().and_then(|id| self.statics.thing_factory.find_by_template_id(id)).map(|o| o.data.get_name().to_vec()).unwrap_or_default()
    }
    fn se_science_name(&self, science: i32) -> Vec<u8> {
        self.statics.science_store.get_internal_name_for_science(&self.names, ScienceType(science))
    }
    fn se_science_from_name(&self, name: &[u8]) -> Option<i32> {
        // `getScienceFromInternalName`: the science whose internal name it is (a name never made a key names none)
        let store = &self.statics.science_store;
        store.infos().map(|i| i.science()).find(|s| store.get_internal_name_for_science(&self.names, *s) == name).map(|s| s.0)
    }
    fn se_actions_enable_difficulty_bonuses(&mut self, enable: bool) {
        self.se_cut("ScriptActions::doEnableOrDisableObjectDifficultyBonuses", &[i64::from(enable)], &[]);
    }
    fn se_audio_add_event(&mut self, name: &[u8], player: i32) {
        self.se_cut("AudioManager::addAudioEvent", &[i64::from(player)], name);
    }
}

#[cfg(test)]
mod tests {
    //! The engine itself is checked against the compiled ScriptEngine.cpp by `tests/differential_script_engine.rs` with a recording
    //! host; this checks it in the world's frame.
    use super::*;
    use crate::common::recorder::game_mode;
    use crate::logic::world::player_host_world::tests::world;

    /// The world's frame runs the engine (`TheScriptEngine->update()`). A constructed engine does nothing until a map
    /// is started (upstream's constructor leaves `m_endGameTimer` 0; `newMap` sets it to -1). After `newMap` the update
    /// runs `TheScriptActions->update()` and `TheScriptConditions->update()` (the world's since batches I8b and I8c).
    #[test]
    fn the_frame_runs_the_script_engine() {
        let (mut w, _) = world();
        w.cuts.game_loop.game_mode = game_mode::SINGLE_PLAYER;
        w.logic_update().unwrap();
        let calls: Vec<&'static str> = w.cuts.log.as_ref().unwrap().iter().filter_map(|c| if let CutCall::Host { call, .. } = c { Some(*call) } else { None }).collect();
        assert!(!calls.contains(&"ScriptActions::update"), "{calls:?}");
        w.script_engine_new_map().unwrap();
        w.cuts.log = Some(Vec::new());
        w.logic_update().unwrap();
        let calls: Vec<&'static str> = w.cuts.log.as_ref().unwrap().iter().filter_map(|c| if let CutCall::Host { call, .. } = c { Some(*call) } else { None }).collect();
        assert!(!calls.contains(&"ScriptEngine::update"), "{calls:?}");
        // (`TheScriptConditions->update()` and `TheScriptActions->update()` are the world's since batches I8b and I8c)
        assert!(!calls.contains(&"ScriptActions::update"), "{calls:?}");
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
