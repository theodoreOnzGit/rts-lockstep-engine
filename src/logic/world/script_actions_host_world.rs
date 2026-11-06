// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptActions.cpp (the calls the actions make out of
//   `ScriptActions`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I8c: `TheScriptActions` in the world ([`World::script_actions`]) and the world as
//! `ScriptActionsHost`. The engine's `executeAction` and the actions' `reset`, `update` and `closeWindows` (batch I8a's cut
//! points) are now the actions'. As for the conditions (batch I8b), the engine evaluating a script is lent back to the
//! world while its action runs, so the actions' questions to `TheScriptEngine` reach it.
//!
//! The 381 host methods were written from the trait's own signatures: the 142 the world answers have their bodies here (41 more `&self` cut points answer a default without a log, 198 `&mut self` ones log)
//! (the script engine's lookups and timers, the waypoints, the ground height, the team factory and teams, the players'
//! orders, relationships, money, ranks, sciences and skill points, the objects' names, positions, statuses, teams,
//! colours and upgrades, the command sets and buttons of the control bar's tables, the special power store, the water,
//! the boundary, the scoring, the hulk lifetime and the buildable overrides). **The rest are cut points**: each `&mut
//! self` one is logged as [`CutCall::Host`] with its name and answers its type's default (false, 0, none, empty); a
//! `&self` one answers the default without a log. They are the client (the camera and view, the in-game UI, the display,
//! drawables, the radar, the audio, EVA, the window manager, the control bar's flashes and display), the AI and its groups
//! (batch I9: every order to an object's AI or a group), containers, special power modules and the partition manager's
//! searches the actions make. Making each of them live is later work, in the same file.
//!
//! `p1w13-sonnet`, integration batch I8c, 2026-11-01.

#![allow(clippy::needless_return)]

use crate::common::ascii_string::AsciiString;
use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::disabled_types::DisabledType;
use crate::common::game_common::Relationship;
use crate::common::game_type::ObjectId;
use crate::common::ini::IniError;
use crate::common::kind_of::KindOfType;
use crate::logic::scripts::ScriptAction;
use crate::logic::world::game_logic::World;
use crate::logic::client_event::{ClientEvent, ScriptAudioEvent};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::script_actions::{
    AudioEvent, ButtonFrom, ButtonH, ButtonSearch, CommandSetH, GroupH, ObjH, PlayerH, ProtoH, ScriptActionsHost, SpecialPowerH, TeamH, TeamTemplateInfo,
    TemplateH, TriggerH, WaypointH, WindowH,
};
use crate::logic::world::script_engine::ScriptEngine;

type R<T> = Result<T, IniError>;

/// A script's relationship number as a `Relationship` (`ENEMIES` 0, `NEUTRAL` 1, `ALLIES` 2).
fn rel(r: i32) -> Relationship {
    match r {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

impl World {
    /// `TheScriptActions->executeAction(action)`, with the engine that runs it put back in the world for the action's
    /// questions to it.
    pub(crate) fn execute_action_with(&mut self, engine: &mut ScriptEngine, action: &ScriptAction) -> R<()> {
        std::mem::swap(&mut self.script_engine, engine);
        let mut sa = std::mem::take(&mut self.script_actions);
        let r = sa.execute_action(self, action);
        self.script_actions = sa;
        std::mem::swap(&mut self.script_engine, engine);
        // (integration batch I29: what the action destroyed, told to the running engine before its next question)
        let notes = std::mem::take(&mut self.cuts.pending_script_notes);
        engine.apply_notes(self, notes);
        match r {
            Ok(()) => Ok(()),
            Err(crate::logic::world::script_actions::ActionError::Undefined(e)) => Err(e),
            // (internal to the dispatch: every type is translated)
            Err(crate::logic::world::script_actions::ActionError::NotTranslated(t)) => {
                self.cuts.team_live_errors.push(format!("ScriptActions::executeAction: type {t} not translated"));
                Ok(())
            }
        }
    }

    /// Integration batch I20: a `GameLogic` member a script sets, at once outside a frame, else when the frame ends (`logic_update`
    /// holds `game_logic` while it runs, and scripts run inside it).
    pub(crate) fn game_logic_write(&mut self, w: crate::logic::world::object_cuts::GameLogicWrite) {
        if self.in_logic_frame {
            self.cuts.pending_game_logic_writes.push(w);
        } else {
            self.apply_game_logic_write(w);
        }
    }

    /// The write applied to `game_logic`.
    pub(crate) fn apply_game_logic_write(&mut self, w: crate::logic::world::object_cuts::GameLogicWrite) {
        use crate::logic::world::object_cuts::GameLogicWrite as W;
        match w {
            W::Option { which, enabled } => match which {
                0 => self.game_logic.show_behind_building_markers = enabled,
                1 => self.game_logic.draw_icon_ui = enabled,
                2 => self.game_logic.show_dynamic_lod = enabled,
                _ => {}
            },
            W::ControlBarOverride { command_set, slot, button } => {
                if let Err(e) = crate::logic::world::game_logic_update::set_control_bar_override(&mut self.game_logic, &command_set, slot, button) {
                    self.cuts.team_live_errors.push(format!("GameLogic::setControlBarOverride: {e:?}"));
                }
            }
            W::HulkMaxLifetimeOverride(frames) => self.game_logic.script_hulk_max_lifetime_override = frames,
        }
    }

    /// `TheScriptActions->reset()`.
    pub(crate) fn script_actions_reset(&mut self) {
        let mut sa = std::mem::take(&mut self.script_actions);
        sa.reset(self);
        self.script_actions = sa;
    }

    /// `TheScriptActions->closeWindows(suppress)`.
    pub(crate) fn script_actions_close_windows(&mut self, suppress: bool) {
        let mut sa = std::mem::take(&mut self.script_actions);
        sa.close_windows(self, suppress);
        self.script_actions = sa;
    }

    /// GitHub #51: one of the actions' calls on `TheAudio`, sent to the client (see [`ScriptAudioEvent`]: nothing the logic
    /// reads, saves or puts in its CRC changes).
    pub(crate) fn sa_audio(&mut self, e: ScriptAudioEvent) {
        if let Some(t) = &mut self.cuts.trace {
            t.push(format!("evt {e:?}"));
        }
        self.events.push(ClientEvent::ScriptAudio(e));
    }

    /// Logs one of the actions' calls into a unit the world does not hold.
    fn sa_cut(&mut self, call: &'static str) {
        self.cut(CutCall::Host { call, args: Vec::new(), text: Vec::new() });
    }
}

/// `REALLY_FAR` (ScriptActions.cpp): `100000 * MAP_XY_FACTOR`.
const REALLY_FAR: f32 = 100000.0 * 10.0;

/// A kind-of mask of one bit.
fn kinds(k: crate::common::kind_of::KindOfType) -> crate::common::kind_of::KindOfMaskType {
    let mut m = crate::common::kind_of::KindOfMaskType::new();
    m.set(k, 1);
    m
}

impl World {
    /// Integration batch I26: `iterateObjectsInRange(obj, REALLY_FAR, FROM_CENTER_3D, filters, ITER_SORTED_NEAR_TO_FAR)`, the objects in
    /// the iterator's order.
    fn scan_from(&mut self, obj: ObjectId, filters: &[crate::logic::world::partition::filters::PartitionFilter]) -> Vec<ObjectId> {
        let mut it = self.with_partition(|pm, w| {
            pm.iterate_objects_in_range(w, obj, REALLY_FAR, crate::logic::world::partition::DistanceCalculationType::FromCenter3D, filters, crate::logic::object_iter::IterOrderType::SortedNearToFar)
        });
        let mut out = Vec::new();
        let mut o = it.first();
        while let Some(id) = o {
            out.push(id);
            o = it.next();
        }
        out
    }
}

impl ScriptActionsHost for World {
    fn sa_win_create_from_script(&mut self, _name: &[u8]) -> WindowH {
        self.sa_cut("win_create_from_script");
        Default::default()
    }
    fn sa_win_destroy(&mut self, _w: WindowH) {
        self.sa_cut("win_destroy");
    }
    fn sa_logic_close_windows(&mut self) {
        self.sa_cut("logic_close_windows");
    }
    fn sa_find_object_by_id(&self, id: ObjectId) -> Option<ObjH> {
        self.find_object(id).map(|o| o.id)
    }
    fn sa_ui_set_input_enabled(&mut self, _enabled: bool) {
        self.sa_cut("ui_set_input_enabled");
    }
    fn sa_mouse_set_visibility(&mut self, _visible: bool) {
        self.sa_cut("mouse_set_visibility");
    }
    fn sa_ui_deselect_all_drawables(&mut self) {
        self.sa_cut("ui_deselect_all_drawables");
    }
    fn sa_ui_clear_attack_move_to_mode(&mut self) {
        self.sa_cut("ui_clear_attack_move_to_mode");
    }
    fn sa_ui_set_waypoint_mode(&mut self, _on: bool) {
        self.sa_cut("ui_set_waypoint_mode");
    }
    fn sa_control_bar_delete_build_tooltip_layout(&mut self) {
        self.sa_cut("control_bar_delete_build_tooltip_layout");
    }
    fn sa_look_at_reset_modes(&mut self) {
        self.sa_cut("look_at_reset_modes");
    }
    fn sa_has_campaign_manager(&self) -> bool {
        // (GitHub #61: `TheCampaignManager` always exists (`GameClient::init`); ~~false, a cut~~)
        true
    }
    fn sa_campaign_set_victorious(&mut self, victorious: bool) {
        // (GitHub #61: live; was a cut)
        self.campaign.set_victorious(victorious);
    }
    fn sa_am_i_observer(&self) -> bool {
        self.victory.am_i_observer()
    }
    fn sa_set_infantry_light_scale(&mut self, _setting: f32) {
        self.sa_cut("set_infantry_light_scale");
    }
    fn sa_is_night(&self) -> bool {
        self.statics.global_data.the_global_data().is_some_and(|g| g.time_of_day == crate::common::game_type::TimeOfDay::Night as i32)
    }
    fn sa_start_quick_end_game_timer(&mut self) {
        self.script_engine.start_quick_end_game_timer();
    }
    fn sa_start_end_game_timer(&mut self) {
        self.script_engine.start_end_game_timer();
    }
    fn sa_start_close_window_timer(&mut self) {
        self.script_engine.start_close_window_timer();
    }
    fn sa_has_shown_mp_local_defeat_window(&self) -> bool {
        self.script_engine.has_shown_mp_local_defeat_window()
    }
    fn sa_mark_mp_local_defeat_window_shown(&mut self) {
        self.script_engine.mark_mp_local_defeat_window_shown();
    }
    fn sa_append_debug_message(&mut self, _message: &[u8], _pause: bool) {
        self.sa_cut("append_debug_message");
    }
    fn sa_unit_named(&self, name: &[u8]) -> Option<ObjH> {
        self.script_engine.get_unit_named(name)
    }
    fn sa_team_named(&self, name: &[u8]) -> Option<TeamH> {
        self.script_engine.get_team_named(self, name).ok().flatten()
    }
    fn sa_did_unit_exist(&self, name: &[u8]) -> bool {
        self.script_engine.did_unit_exist(name)
    }
    fn sa_transfer_object_name(&mut self, name: &[u8], o: ObjH) {
        // (batch I35: queued if the engine is out of the world, rather than given to its placeholder)
        if name.is_empty() {
            return;
        }
        self.script_note(crate::logic::world::script_engine::ScriptNote::TransferObjectName { name: name.to_vec(), object: o });
    }
    fn sa_add_object_to_cache(&mut self, o: ObjH) {
        let mut se = std::mem::take(&mut self.script_engine);
        se.add_object_to_cache(self, o);
        self.script_engine = se;
    }
    fn sa_current_player(&self) -> Option<PlayerH> {
        self.script_engine.get_current_player()
    }
    fn sa_player_from_name(&mut self, name: &[u8]) -> Option<PlayerH> {
        let se = std::mem::take(&mut self.script_engine);
        let r = se.get_player_from_ascii_string(self, name);
        self.script_engine = se;
        r
    }
    fn sa_local_player(&self) -> Option<PlayerH> {
        (0..16).find(|&i| crate::logic::world::player_ops::player_is_local_player(self, i))
    }
    fn sa_player_is_observer(&self, p: PlayerH) -> bool {
        crate::logic::world::player_ops::player_is_player_observer(self, p).unwrap_or(false)
    }
    fn sa_player_build_specific_building(&mut self, p: PlayerH, building: &[u8]) {
        let r = crate::logic::world::player_ops::player_build_specific_building(self, p, building);
        self.script_player("Player::buildSpecificBuilding", r);
    }
    fn sa_player_build_by_supplies(&mut self, p: PlayerH, cash: i32, building: &[u8]) {
        let r = crate::logic::world::player_ops::player_build_by_supplies(self, p, cash, building);
        self.script_player("Player::buildBySupplies", r);
    }
    fn sa_player_build_specific_building_nearest_team(&mut self, p: PlayerH, building: &[u8], t: TeamH) {
        let r = crate::logic::world::player_ops::player_build_specific_building_nearest_team(self, p, building, u32::try_from(t).ok());
        self.script_player("Player::buildSpecificBuildingNearestTeam", r);
    }
    fn sa_player_build_upgrade(&mut self, p: PlayerH, upgrade: &[u8]) {
        let r = crate::logic::world::player_ops::player_build_upgrade(self, p, upgrade);
        self.script_player("Player::buildUpgrade", r);
    }
    fn sa_player_build_base_defense(&mut self, p: PlayerH, flank: bool) {
        let r = crate::logic::world::player_ops::player_build_base_defense(self, p, flank);
        self.script_player("Player::buildBaseDefense", r);
    }
    fn sa_player_build_base_defense_structure(&mut self, p: PlayerH, building: &[u8], flank: bool) {
        let r = crate::logic::world::player_ops::player_build_base_defense_structure(self, p, building, flank);
        self.script_player("Player::buildBaseDefenseStructure", r);
    }
    fn sa_audio_add_event(&mut self, e: &AudioEvent) {
        // (#51: was a cut) the client's audio manager makes the call. (#50) A logical event's `GameLogicRandomValue` draws are
        // the logic's: made here, at the action, from the World's generator, as `addAudioEvent` makes them upstream (the
        // event is the action's local, so its playing index is the constructor's -1 and nothing is kept for a next call)
        let picks = if e.logical {
            crate::logic::logical_audio::logical_audio_draws(&self.statics.audio, &e.name, &mut self.random)
        } else {
            Default::default()
        };
        self.sa_audio(ScriptAudioEvent::AddEvent(e.clone(), picks));
    }
    fn sa_waypoint_by_name(&self, name: &[u8]) -> Option<WaypointH> {
        AsciiString::try_from_bytes(name).ok().and_then(|n| self.terrain.waypoint_by_name(&n)).and_then(|h| self.terrain.waypoint(h)).map(|w| w.id())
    }
    fn sa_first_waypoint(&self) -> Option<WaypointH> {
        self.terrain.first_waypoint().and_then(|h| self.terrain.waypoint(h)).map(|w| w.id())
    }
    fn sa_waypoint_next(&self, w: WaypointH) -> Option<WaypointH> {
        let h = self.terrain.waypoint_by_id(w as u32)?;
        self.terrain.next_waypoint(h).and_then(|n| self.terrain.waypoint(n)).map(|x| x.id())
    }
    fn sa_waypoint_name(&self, w: WaypointH) -> Vec<u8> {
        self.terrain.waypoint_by_id(w as u32).and_then(|h| self.terrain.waypoint(h)).map(|x| x.name().str().to_vec()).unwrap_or_default()
    }
    fn sa_waypoint_location(&self, w: WaypointH) -> Coord3D {
        self.terrain.waypoint_by_id(w as u32).and_then(|h| self.terrain.waypoint(h)).map(|x| *x.location()).unwrap_or_default()
    }
    fn sa_ground_height(&self, x: f32, y: f32) -> f32 {
        self.terrain.ground_height(x, y, false).0
    }
    fn sa_create_crater_in_terrain(&mut self, o: ObjH) {
        // (integration batch I20: the terrain logic's with `live_terrain`, the cut otherwise)
        if self.cuts.live_terrain {
            if let Some(view) = crate::logic::world::terrain_host_world::object_view(self, o) {
                if self.terrain.create_crater_in_terrain(&view).is_ok() {
                    return;
                }
            }
        }
        self.sa_cut("create_crater_in_terrain");
    }
    fn sa_pathfinder_add_object(&mut self, o: ObjH) {
        self.pathfinder_add_object_to_map(o)
    }
    fn sa_oversize_the_terrain(&mut self, _amount: i32) {
        self.sa_cut("oversize_the_terrain");
    }
    fn sa_view_set_camera_lock(&mut self, _id: ObjectId) {
        self.sa_cut("view_set_camera_lock");
    }
    fn sa_view_snap_to_camera_lock(&mut self) {
        self.sa_cut("view_snap_to_camera_lock");
    }
    fn sa_view_set_snap_mode(&mut self, _lock_type: i32, _dist: f32) {
        self.sa_cut("view_set_snap_mode");
    }
    fn sa_view_move_camera_to(&mut self, _p: &Coord3D, _frames: i32, _shutter: i32, _orient: bool, _ease_in: f32, _ease_out: f32) {
        self.sa_cut("view_move_camera_to");
    }
    fn sa_view_move_camera_along_waypoint_path(&mut self, _w: WaypointH, _frames: i32, _shutter: i32, _orient: bool, _ease_in: f32, _ease_out: f32) {
        self.sa_cut("view_move_camera_along_waypoint_path");
    }
    fn sa_view_zoom_camera(&mut self, _zoom: f32, _ms: i32, _ease_in: f32, _ease_out: f32) {
        self.sa_cut("view_zoom_camera");
    }
    fn sa_view_pitch_camera(&mut self, _pitch: f32, _ms: i32, _ease_in: f32, _ease_out: f32) {
        self.sa_cut("view_pitch_camera");
    }
    fn sa_view_rotate_camera(&mut self, _rotations: f32, _frames: i32, _ease_in: f32, _ease_out: f32) {
        self.sa_cut("view_rotate_camera");
    }
    fn sa_view_rotate_camera_toward_object(&mut self, _id: ObjectId, _ms: i32, _hold: i32, _ease_in: f32, _ease_out: f32) {
        self.sa_cut("view_rotate_camera_toward_object");
    }
    fn sa_view_rotate_camera_toward_position(&mut self, _p: &Coord3D, _ms: i32, _ease_in: f32, _ease_out: f32, _reverse: bool) {
        self.sa_cut("view_rotate_camera_toward_position");
    }
    fn sa_view_reset_camera(&mut self, _p: &Coord3D, _frames: i32, _ease_in: f32, _ease_out: f32) {
        self.sa_cut("view_reset_camera");
    }
    fn sa_view_scroll_by(&mut self, _d: &Coord2D) {
        self.sa_cut("view_scroll_by");
    }
    fn sa_view_camera_mod_look_toward(&mut self, _p: &Coord3D) {
        self.sa_cut("view_camera_mod_look_toward");
    }
    fn sa_view_camera_mod_final_look_toward(&mut self, _p: &Coord3D) {
        self.sa_cut("view_camera_mod_final_look_toward");
    }
    fn sa_view_camera_mod_final_move_to(&mut self, _p: &Coord3D) {
        self.sa_cut("view_camera_mod_final_move_to");
    }
    fn sa_view_camera_mod_final_pitch(&mut self, _pitch: f32, _ease_in: f32, _ease_out: f32) {
        self.sa_cut("view_camera_mod_final_pitch");
    }
    fn sa_view_camera_mod_final_zoom(&mut self, _zoom: f32, _ease_in: f32, _ease_out: f32) {
        self.sa_cut("view_camera_mod_final_zoom");
    }
    fn sa_drawables(&self) -> Vec<(bool, Coord3D)> {
        Default::default()
    }
    fn sa_team_damage_members(&mut self, t: TeamH, amount: f32) {
        let _ = crate::logic::world::team_ops::team_damage_team_members(self, t as u32, amount);
    }
    fn sa_team_set_state(&mut self, t: TeamH, state: &[u8]) {
        if let Some(x) = self.teams.team_mut(t as u32) {
            x.state = state.to_vec();
        }
    }
    fn sa_team_set_active(&mut self, t: TeamH) {
        let _ = crate::logic::world::team_ops::team_set_active(self, t as u32);
    }
    fn sa_team_as_ai_group(&mut self, t: TeamH, g: GroupH) {
        if let (Ok(t), Ok(g)) = (u32::try_from(t), u32::try_from(g)) {
            crate::logic::world::team_ops::team_get_team_as_ai_group(self, t, Some(g));
        }
    }
    fn sa_team_members(&self, t: TeamH) -> Vec<ObjH> {
        self.teams.members(t as u32)
    }
    fn sa_find_team_prototype(&self, name: &[u8]) -> Option<ProtoH> {
        self.teams.prototypes().find(|p| p.name == name).map(|p| p.id as ProtoH)
    }
    fn sa_proto_template_info(&self, p: ProtoH) -> TeamTemplateInfo {
        // (integration batch I28: was a default, so a scripted reinforcement had no units, no transport and no waypoint)
        let Some(info) = u32::try_from(p).ok().and_then(|p| self.teams.prototype(p)).map(|p| &p.team_template) else { return Default::default() };
        TeamTemplateInfo {
            start_reinforce_waypoint: info.start_reinforce_waypoint.clone(),
            transport_unit_type: info.transport_unit_type.clone(),
            units_info: info.units_info[..info.num_units_info.clamp(0, info.units_info.len() as i32) as usize].iter().map(|u| (u.unit_thing_name.clone(), u.max_units)).collect(),
            team_starts_full: info.team_starts_full,
            transports_exit: info.transports_exit,
        }
    }
    fn sa_create_inactive_team(&mut self, name: &[u8]) -> Option<TeamH> {
        crate::logic::world::team_ops::factory_create_inactive_team(self, name).ok().map(|t| t as TeamH)
    }
    fn sa_create_team(&mut self, name: &[u8]) -> Option<TeamH> {
        crate::logic::world::team_ops::factory_create_team(self, name).ok().map(|t| t as TeamH)
    }
    fn sa_create_group(&mut self) -> Option<GroupH> {
        Some(crate::logic::world::game_logic_update::GameLogicHost::u_create_group(self) as GroupH)
    }
    fn sa_group_move_to_position(&mut self, g: GroupH, p: &Coord3D, add_waypoint: bool, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_move_to_position(w, p, add_waypoint, cmd));
    }
    fn sa_group_attack_team(&mut self, g: GroupH, t: TeamH, max_shots: i32, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_attack_team(w, u32::try_from(t).ok(), max_shots, cmd));
    }
    fn sa_obj_has_ai(&self, o: ObjH) -> bool {
        self.find_object(o).is_some_and(|x| x.ai.is_some())
    }
    fn sa_ai_clear_waypoint_queue(&mut self, o: ObjH) {
        self.ai_ask(o, crate::logic::world::module_call::ModuleCall::AiClearWaypointQueue);
    }
    fn sa_ai_choose_locomotor_set(&mut self, o: ObjH, set: i32) {
        self.ai_ask(o, crate::logic::world::module_call::ModuleCall::AiChooseLocomotorSet(set));
    }
    fn sa_ai_move_to_position(&mut self, o: ObjH, p: &Coord3D, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_MOVE_TO_POSITION, cmd, |q| q.pos = *p);
    }
    fn sa_ai_move_to_and_evacuate(&mut self, o: ObjH, p: &Coord3D, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_MOVE_TO_POSITION_AND_EVACUATE, cmd, |q| q.pos = *p);
    }
    fn sa_ai_move_to_and_evacuate_and_exit(&mut self, o: ObjH, p: &Coord3D, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT, cmd, |q| q.pos = *p);
    }
    fn sa_ai_force_attack_object(&mut self, o: ObjH, victim: ObjH, max_shots: i32, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_FORCE_ATTACK_OBJECT, cmd, |q| {
            q.obj = victim;
            q.int_value = max_shots;
        });
    }
    fn sa_ai_set_is_recruitable(&mut self, o: ObjH, recruitable: bool) {
        self.ai_ask(o, crate::logic::world::module_call::ModuleCall::AiSetIsRecruitable(recruitable));
    }
    fn sa_find_template(&self, name: &[u8]) -> Option<TemplateH> {
        World::thing_factory_find_template(self, name).map(TemplateH::from)
    }
    fn sa_new_object(&mut self, t: TemplateH, team: Option<TeamH>) -> Option<ObjH> {
        let t = u16::try_from(t).ok()?;
        World::thing_factory_new_object(self, t, team.map(|t| t as u32), Default::default()).ok()
    }
    fn sa_obj_template(&self, o: ObjH) -> Option<TemplateH> {
        self.find_object(o).map(|x| TemplateH::from(x.template().get_template_id()))
    }
    fn sa_template_is_equivalent_to(&self, a: TemplateH, b: Option<TemplateH>) -> bool {
        match (u16::try_from(a), b.map(u16::try_from)) {
            (Ok(a), Some(Ok(b))) => self.thing_template_is_equivalent_to(a, b),
            _ => false,
        }
    }
    fn sa_obj_is_effectively_dead(&self, o: ObjH) -> bool {
        self.find_object(o).is_some_and(|x| x.is_effectively_dead())
    }
    fn sa_obj_is_kind_of(&self, o: ObjH, k: KindOfType) -> bool {
        self.find_object(o).is_some_and(|x| x.is_kind_of(k))
    }
    fn sa_obj_is_disabled_by_type(&self, o: ObjH, t: DisabledType) -> bool {
        self.find_object(o).is_some_and(|x| x.is_disabled_by_type(t))
    }
    fn sa_obj_set_name(&mut self, o: ObjH, name: &[u8]) {
        if let Some(x) = self.find_object_mut(o) {
            x.name = name.to_vec();
        }
    }
    fn sa_obj_set_position(&mut self, o: ObjH, p: &Coord3D) {
        self.set_position(o, p);
    }
    fn sa_obj_set_orientation(&mut self, o: ObjH, angle: f32) {
        self.set_orientation(o, angle);
    }
    fn sa_obj_major_radius(&self, o: ObjH) -> f32 {
        self.find_object(o).map_or(0.0, |x| x.geometry_info.get_major_radius())
    }
    fn sa_obj_transport_slot_count(&mut self, o: ObjH) -> i32 {
        // (integration batch I22: was `Default::default()`)
        self.get_transport_slot_count(o)
    }
    fn sa_obj_is_contained(&self, o: ObjH) -> bool {
        self.is_contained(o)
    }
    fn sa_obj_leave_group(&mut self, o: ObjH) {
        self.leave_group(o);
    }
    fn sa_obj_set_script_status(&mut self, o: ObjH, bit: i32, value: bool) {
        self.set_script_status(o, bit as u8, value);
    }
    fn sa_obj_has_body(&self, o: ObjH) -> bool {
        self.get_body_module(o).is_some()
    }
    fn sa_body_set_indestructible(&mut self, o: ObjH, value: bool) {
        if let Some(b) = self.find_object(o).and_then(|x| x.body) {
            self.module_call(o, b, crate::logic::world::module_call::ModuleCall::SetIndestructible(value));
        }
    }
    fn sa_obj_is_selectable(&self, o: ObjH) -> bool {
        self.is_selectable(o)
    }
    fn sa_obj_set_selectable(&mut self, o: ObjH, value: bool) {
        self.set_selectable(o, value);
    }
    fn sa_obj_has_drawable(&self, o: ObjH) -> bool {
        self.has_drawable(o)
    }
    fn sa_drawable_enable_ambient_sound(&mut self, o: ObjH, enable: bool) {
        // (#51: was a cut) the client's drawable makes the call
        self.emit(crate::logic::world::object_events::ObjectClientEvent::DrawableEnableAmbientSoundFromScript { object_id: o, enable });
    }
    fn sa_obj_has_contain(&self, o: ObjH) -> bool {
        self.get_contain(o).is_some()
    }
    fn sa_contain_max(&mut self, o: ObjH) -> u32 {
        // (integration batch I22: was `Default::default()`)
        let Some(c) = self.get_contain(o) else { return 0 };
        self.module_call(o, c, crate::logic::world::module_call::ModuleCall::GetContainMax).as_int(0) as u32
    }
    fn sa_contain_is_valid_container_for(&mut self, c: ObjH, o: ObjH, check_capacity: bool) -> bool {
        // (integration batch I22: was `Default::default()`)
        let Some(m) = self.get_contain(c) else { return false };
        self.module_call(c, m, crate::logic::world::module_call::ModuleCall::IsValidContainerFor { obj: o, check_capacity }).as_bool()
    }
    fn sa_contain_add(&mut self, c: ObjH, o: ObjH) {
        if let Some(ct) = self.get_contain(c) {
            self.module_call(c, ct, crate::logic::world::module_call::ModuleCall::AddToContain(o));
        }
    }
    fn sa_obj_has_deliver_payload(&self, o: ObjH) -> bool {
        // (integration batch I22: was `Default::default()`)
        self.find_module(o, b"DeliverPayloadAIUpdate").is_some()
    }
    fn sa_deliver_payload_put_in_container_template(&self, o: ObjH) -> Option<TemplateH> {
        // (integration batch I22: was `Default::default()`)
        // `TheThingFactory->findTemplate(getDeliverPayloadAIUpdateModuleData()->m_putInContainerName)`
        let m = self.find_module(o, b"DeliverPayloadAIUpdate")?;
        let name = match self.peek_module_body(crate::logic::world::module_call::ModuleHandle { object: o, index: m })? {
            crate::logic::world::update_body::UpdateBody::AiUpdate(a) => match &a.module.as_ref()?.ai.as_ref()?.subclass {
                crate::logic::ai::ai_update::AiSubclass::DeliverPayload(d) => d.module_data.put_in_container_name.clone(),
                _ => return None,
            },
            _ => return None,
        };
        self.sa_find_template(&name)
    }
    fn sa_deliver_payload(&mut self, o: ObjH, p: &Coord3D) {
        self.ai_ask(o, crate::logic::world::module_call::ModuleCall::AiDeliverPayloadViaModuleData(*p));
    }
    fn sa_attack_info(&self, name: &[u8]) -> Vec<u8> {
        // (integration batch I22: was `Default::default()`)
        // `getAttackInfo(name)`: the first of index 1 on with the name, else the default (index 0, an empty name)
        let infos = &self.script_engine.attack_priority_info;
        infos.iter().skip(1).find(|a| a.name.as_slice() == name).or(infos.first()).map(|a| a.name.clone()).unwrap_or_default()
    }
    fn sa_team_set_attack_priority_name(&mut self, t: TeamH, name: &[u8]) {
        if let Ok(t) = u32::try_from(t) {
            if let Err(e) = crate::logic::world::team_ops::team_set_attack_priority_name(self, t, name) {
                self.cuts.team_live_errors.push(format!("Team::setAttackPriorityName: {e:?}"));
            }
        }
    }
    fn sa_ai_set_attack_info(&mut self, o: ObjH, info: &[u8]) {
        // `TheScriptEngine->getAttackInfo(name)`: index 1 on with the name, else 0 (the default)
        let infos = &self.script_engine.attack_priority_info;
        let found = infos.iter().enumerate().skip(1).find(|(_, a)| a.name.as_slice() == info).map(|(i, _)| i);
        let index = (!infos.is_empty()).then(|| found.unwrap_or(0));
        self.ai_ask(o, crate::logic::world::module_call::ModuleCall::AiSetAttackInfo(index));
    }
    fn sa_player_set_team_delay_seconds(&mut self, p: PlayerH, seconds: i32) {
        let r = crate::logic::world::player_ops::player_set_team_delay_seconds(self, p, seconds);
        self.script_player("Player::setTeamDelaySeconds", r);
    }
    fn sa_ai_set_attitude(&mut self, o: ObjH, attitude: i32) {
        self.ai_ask(o, crate::logic::world::module_call::ModuleCall::AiSetAttitude(attitude));
    }
    fn sa_group_set_attitude(&mut self, g: GroupH, attitude: i32) {
        self.script_group(g, |gr, w| gr.set_attitude(w, attitude));
    }
    fn sa_obj_set_status(&mut self, o: ObjH, status: i32, value: bool) {
        self.set_status(o, crate::logic::world::object_state::mask_of(status), value);
    }
    fn sa_trigger_named(&mut self, name: &[u8]) -> Option<TriggerH> {
        crate::logic::world::script_engine::ScriptEngine::qualified_trigger_area_by_name(self.script_engine.current_player, self, name).map(|t| t as TriggerH)
    }
    fn sa_ai_attack_area(&mut self, o: ObjH, trig: TriggerH, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_ATTACK_AREA, cmd, |q| q.polygon = Some(trig));
    }
    fn sa_ai_attack_team(&mut self, o: ObjH, t: TeamH, max_shots: i32, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_ATTACK_TEAM, cmd, |q| {
            q.team = u32::try_from(t).ok();
            q.int_value = max_shots;
        });
    }
    fn sa_group_attack_area(&mut self, g: GroupH, trig: TriggerH, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_command(w, crate::logic::ai::ai_group::GroupAiCommand::AttackArea { area: trig }, cmd));
    }
    fn sa_group_attack_object(&mut self, g: GroupH, victim: ObjH, max_shots: i32, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_attack_object(w, victim, max_shots, cmd));
    }
    fn sa_ai_enter(&mut self, o: ObjH, target: ObjH, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_ENTER, cmd, |q| q.obj = target);
    }
    fn sa_group_enter(&mut self, g: GroupH, target: ObjH, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_command(w, crate::logic::ai::ai_group::GroupAiCommand::Enter { obj: target }, cmd));
    }
    fn sa_ai_evacuate(&mut self, o: ObjH, expose: bool, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_EVACUATE, cmd, |q| q.int_value = i32::from(expose));
    }
    fn sa_group_evacuate(&mut self, g: GroupH, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_evacuate(w, cmd));
    }
    fn sa_contain_set_evac_disposition(&mut self, o: ObjH, disposition: u32) {
        if let Some(ct) = self.get_contain(o) {
            self.module_call(o, ct, crate::logic::world::module_call::ModuleCall::SetEvacDisposition(disposition as i32));
        }
    }
    fn sa_obj_position(&self, o: ObjH) -> Coord3D {
        self.find_object(o).map(|x| x.get_position()).unwrap_or_default()
    }
    fn sa_closest_waypoint_on_path(&self, p: &Coord3D, label: &[u8]) -> Option<WaypointH> {
        let h = self.terrain.closest_waypoint_on_path(p, &AsciiString::try_from_bytes(label).ok()?)?;
        self.terrain.waypoint(h).map(|w| w.id())
    }
    fn sa_ai_follow_waypoint_path(&mut self, o: ObjH, w: WaypointH, exact: bool, cmd: i32) {
        let c = if exact { crate::logic::ai::ai_commands::AICMD_FOLLOW_WAYPOINT_PATH_EXACT } else { crate::logic::ai::ai_commands::AICMD_FOLLOW_WAYPOINT_PATH };
        self.script_order(o, c, cmd, |q| q.waypoint = u32::try_from(w).ok());
    }
    fn sa_group_follow_waypoint_path(&mut self, g: GroupH, w: WaypointH, as_team: bool, exact: bool, cmd: i32) {
        let Ok(way) = u32::try_from(w) else { return };
        self.script_group(g, |gr, w| gr.group_follow_waypoint_path(w, way, exact, as_team, cmd));
    }
    fn sa_skirmish_enemy_player(&mut self) -> Option<PlayerH> {
        let se = std::mem::take(&mut self.script_engine);
        let r = se.get_skirmish_enemy_player(self);
        self.script_engine = se;
        r
    }
    fn sa_player_mp_start_index(&self, p: PlayerH) -> i32 {
        self.players.get_nth(p).map_or(0, |x| x.mp_start_index)
    }
    fn sa_player_check_bridges(&mut self, p: PlayerH, o: ObjH, w: WaypointH) {
        let r = crate::logic::world::player_ops::player_check_bridges(self, p, Some(o), u32::try_from(w).ok());
        self.script_player("Player::checkBridges", r);
    }
    fn sa_ai_guard_position(&mut self, o: ObjH, p: &Coord3D, mode: i32, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_GUARD_POSITION, cmd, |q| {
            q.pos = *p;
            q.int_value = mode;
        });
    }
    fn sa_group_guard_position(&mut self, g: GroupH, p: &Coord3D, mode: i32, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_guard(w, Some(crate::logic::ai::ai_group::GroupAiCommand::GuardPosition { pos: *p, mode }), cmd));
    }
    fn sa_group_guard_object(&mut self, g: GroupH, o: ObjH, mode: i32, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_guard(w, Some(crate::logic::ai::ai_group::GroupAiCommand::GuardObject { obj: o, mode }), cmd));
    }
    fn sa_group_guard_area(&mut self, g: GroupH, trig: TriggerH, mode: i32, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_guard(w, Some(crate::logic::ai::ai_group::GroupAiCommand::GuardArea { area: trig, mode }), cmd));
    }
    fn sa_ai_hunt(&mut self, o: ObjH, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_HUNT, cmd, |_| {});
    }
    fn sa_group_hunt(&mut self, g: GroupH, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_command(w, crate::logic::ai::ai_group::GroupAiCommand::Hunt, cmd));
    }
    fn sa_find_command_button(&self, name: &[u8]) -> Option<ButtonH> {
        self.cuts.command_buttons.iter().position(|b| b.name == name).map(|i| i as ButtonH)
    }
    fn sa_button_command_type(&self, b: ButtonH) -> i32 {
        self.cuts.command_buttons.get(b as usize).map_or(0, |x| x.command_type)
    }
    fn sa_button_has_special_power(&self, b: ButtonH) -> bool {
        self.cuts.command_buttons.get(b as usize).is_some_and(|x| x.special_power.is_some())
    }
    fn sa_button_options(&self, b: ButtonH) -> u32 {
        // (integration batch I22: was `Default::default()`)
        self.cuts.command_buttons.get(b as usize).map_or(0, |x| x.options)
    }
    fn sa_obj_command_set_string(&self, o: ObjH) -> Vec<u8> {
        self.object_get_command_set_string(o)
    }
    fn sa_find_command_set(&self, name: &[u8]) -> Option<CommandSetH> {
        self.control_bar_find_command_set(name).map(|s| s as CommandSetH)
    }
    fn sa_command_set_button(&self, s: CommandSetH, i: i32) -> Option<ButtonH> {
        self.command_set_get_command_button(usize::try_from(s).ok()?, usize::try_from(i).ok()?).map(|b| b as ButtonH)
    }
    fn sa_template_name(&self, t: TemplateH) -> Vec<u8> {
        u16::try_from(t).ok().and_then(|t| self.statics.thing_factory.find_by_template_id(t)).map(|o| o.data.get_name().to_vec()).unwrap_or_default()
    }
    fn sa_obj_has_command_button_hunt(&self, o: ObjH) -> bool {
        // (integration batch I22: was `Default::default()`)
        self.find_module(o, b"CommandButtonHuntUpdate").is_some()
    }
    fn sa_command_button_hunt_set(&mut self, o: ObjH, ability: &[u8]) {
        if let Some(m) = self.find_module(o, b"CommandButtonHuntUpdate") {
            self.module_call(o, m, crate::logic::world::module_call::ModuleCall::CommandButtonHuntSetCommandButton(ability.to_vec()));
        }
    }
    fn sa_player_set_units_should_hunt(&mut self, p: PlayerH, hunt: bool, cmd: i32) {
        let _ = crate::logic::world::player_objects::player_set_units_should_hunt(self, p, hunt, cmd);
    }
    fn sa_player_sell_everything(&mut self, p: PlayerH) {
        let _ = crate::logic::world::player_objects::player_sell_everything_under_the_sun(self, p);
    }
    fn sa_player_set_can_build_base(&mut self, p: PlayerH, can: bool) {
        if let Some(x) = self.players.get_nth_mut(p) {
            x.can_build_base = can;
        }
    }
    fn sa_player_set_can_build_units(&mut self, p: PlayerH, can: bool) {
        if let Some(x) = self.players.get_nth_mut(p) {
            x.can_build_units = can;
        }
    }
    fn sa_player_set_objects_enabled(&mut self, p: PlayerH, name: &[u8], enable: bool) {
        let _ = crate::logic::world::player_ops::player_set_objects_enabled(self, p, name, enable);
    }
    fn sa_player_repair_structure(&mut self, p: PlayerH, id: ObjectId) {
        let r = crate::logic::world::player_ops::player_repair_structure(self, p, id);
        self.script_player("Player::repairStructure", r);
    }
    fn sa_proto_controlling_player(&self, p: ProtoH) -> Option<PlayerH> {
        self.teams.prototype(p as u32).and_then(|x| x.owning_player)
    }
    fn sa_player_build_specific_team(&mut self, p: PlayerH, proto: ProtoH) {
        let r = crate::logic::world::player_ops::player_build_specific_team(self, p, u32::try_from(proto).ok());
        self.script_player("Player::buildSpecificTeam", r);
    }
    fn sa_player_recruit_specific_team(&mut self, p: PlayerH, proto: ProtoH, radius: f32) {
        let r = crate::logic::world::player_ops::player_recruit_specific_team(self, p, u32::try_from(proto).ok(), radius);
        self.script_player("Player::recruitSpecificTeam", r);
    }
    fn sa_obj_attempt_damage(&mut self, o: ObjH, source: ObjectId, damage_type: i32, death_type: i32, amount: f32) {
        let mut info = crate::logic::damage::DamageInfo::default();
        info.input.source_id = source;
        info.input.damage_type = damage_type;
        info.input.death_type = death_type;
        info.input.amount = amount;
        self.attempt_damage(o, &mut info)
    }
    fn sa_logic_destroy_object(&mut self, o: ObjH) {
        self.destroy_object(o);
    }
    fn sa_team_delete(&mut self, t: TeamH, ignore_dead: bool) {
        let _ = crate::logic::world::team_ops::team_delete_team(self, t as u32, ignore_dead);
    }
    fn sa_ai_wander(&mut self, o: ObjH, w: WaypointH, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_WANDER, cmd, |q| q.waypoint = u32::try_from(w).ok());
    }
    fn sa_ai_wander_in_place(&mut self, o: ObjH, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_WANDER_IN_PLACE, cmd, |_| {});
    }
    fn sa_ai_panic(&mut self, o: ObjH, w: WaypointH, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_PANIC, cmd, |q| q.waypoint = u32::try_from(w).ok());
    }
    fn sa_team_prototype(&self, t: TeamH) -> Option<ProtoH> {
        self.teams.team(t as u32).map(|x| x.proto as ProtoH)
    }
    fn sa_proto_change_priority(&mut self, p: ProtoH, up: bool) {
        let Ok(p) = u32::try_from(p) else { return };
        let r = if up {
            crate::logic::world::team_ops::proto_increase_ai_priority_for_success(self, p)
        } else {
            crate::logic::world::team_ops::proto_decrease_ai_priority_for_failure(self, p)
        };
        if let Err(e) = r {
            self.cuts.team_live_errors.push(format!("TeamPrototype AI priority: {e:?}"));
        }
    }
    fn sa_proto_production_priority(&self, p: ProtoH) -> i32 {
        // (integration batch I22: was `Default::default()`)
        u32::try_from(p).ok().and_then(|p| self.teams.prototype(p)).map_or(0, |p| p.team_template.production_priority)
    }
    fn sa_obj_kill(&mut self, o: ObjH) {
        self.kill(o, crate::logic::damage::DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
    }
    fn sa_team_kill(&mut self, t: TeamH) {
        let _ = crate::logic::world::team_ops::team_kill_team(self, t as u32);
    }
    fn sa_player_kill(&mut self, p: PlayerH) {
        let _ = crate::logic::world::player_objects::player_kill_player(self, p);
    }
    fn sa_ui_message(&mut self, _text: &[u8]) {
        self.sa_cut("ui_message");
    }
    fn sa_ui_popup_message(&mut self, _message: &[u8], _x: i32, _y: i32, _width: i32, _pause: bool, _pause_music: bool) {
        self.sa_cut("ui_popup_message");
    }
    fn sa_game_text_fetch(&self, _label: &[u8]) -> (Vec<u16>, bool) {
        Default::default()
    }
    fn sa_display_set_cinematic_text(&mut self, _text: &[u8]) {
        self.sa_cut("display_set_cinematic_text");
    }
    fn sa_display_set_cinematic_font(&mut self, _name: &[u8], _size: i32, _bold: bool) {
        self.sa_cut("display_set_cinematic_font");
    }
    fn sa_adjust_font_size(&self, _size: i32) -> i32 {
        Default::default()
    }
    fn sa_display_set_cinematic_text_frames(&mut self, _frames: i32) {
        self.sa_cut("display_set_cinematic_text_frames");
    }
    fn sa_button_set_flash_count(&mut self, _b: ButtonH, _count: i32) {
        self.sa_cut("button_set_flash_count");
    }
    fn sa_control_bar_set_flash(&mut self, _on: bool) {
        self.sa_cut("control_bar_set_flash");
    }
    fn sa_obj_set_custom_indicator_color(&mut self, o: ObjH, c: i32) {
        self.set_custom_indicator_color(o, c);
    }
    fn sa_obj_indicator_color(&self, o: ObjH) -> i32 {
        self.get_indicator_color(o)
    }
    fn sa_obj_night_indicator_color(&self, o: ObjH) -> i32 {
        self.get_night_indicator_color(o)
    }
    fn sa_drawable_set_flash_color(&mut self, _o: ObjH, _c: i32) {
        self.sa_cut("drawable_set_flash_color");
    }
    fn sa_drawable_set_flash_count(&mut self, _o: ObjH, _count: i32) {
        self.sa_cut("drawable_set_flash_count");
    }
    fn sa_drawable_set_indicator_color(&mut self, _o: ObjH, _c: i32) {
        self.sa_cut("drawable_set_indicator_color");
    }
    fn sa_team_has_any_objects(&self, t: TeamH) -> bool {
        crate::logic::world::team_ops::team_has_any_objects(self, t as u32)
    }
    fn sa_display_play_movie(&mut self, _name: &[u8]) {
        self.sa_cut("display_play_movie");
    }
    fn sa_ui_play_movie(&mut self, _name: &[u8]) {
        self.sa_cut("ui_play_movie");
    }
    fn sa_ui_military_subtitle(&mut self, _label: &[u8], _duration: i32) {
        self.sa_cut("ui_military_subtitle");
    }
    fn sa_player_transfer_assets_from(&mut self, dest: PlayerH, src: PlayerH) {
        let _ = crate::logic::world::player_objects::player_transfer_assets_from_that(self, dest, src);
    }
    fn sa_player_default_team(&self, p: PlayerH) -> Option<TeamH> {
        self.players.get_nth(p).and_then(|x| x.default_team).map(|t| t as TeamH)
    }
    fn sa_obj_set_team(&mut self, o: ObjH, t: Option<TeamH>) {
        self.set_team(o, t.map(|t| t as u32));
    }
    fn sa_obj_update_upgrade_modules(&mut self, o: ObjH) {
        self.update_upgrade_modules(o);
    }
    fn sa_player_set_list_in_score_screen(&mut self, p: PlayerH, list: bool) {
        if let Some(x) = self.players.get_nth_mut(p) {
            x.list_in_score_screen = list;
        }
    }
    fn sa_logic_enable_scoring(&mut self, score: bool) {
        self.game_logic.scoring_enabled = score;
    }
    fn sa_player_set_player_relationship(&mut self, p: PlayerH, other: PlayerH, r: i32) {
        let r = match r {
            0 => crate::common::game_common::Relationship::Enemies,
            2 => crate::common::game_common::Relationship::Allies,
            _ => crate::common::game_common::Relationship::Neutral,
        };
        let _ = crate::logic::world::player_ops::player_set_player_relationship(self, p, Some(other), r);
    }
    /// `TheRadar->createEvent(pos, type)`: the cut log's record, and (GitHub #35) the call as a client event, for the
    /// client's radar (`client::radar`).
    fn sa_radar_create_event(&mut self, p: &Coord3D, event_type: i32) {
        self.sa_cut("radar_create_event");
        self.events.push(crate::logic::client_event::ClientEvent::Radar(crate::logic::client_event::RadarCall::CreateEvent { position: *p, event_type }));
    }
    fn sa_team_has_any_units(&self, t: TeamH) -> bool {
        crate::logic::world::team_ops::team_has_any_units(self, t as u32)
    }
    fn sa_team_estimate_position(&self, t: TeamH) -> Option<Coord3D> {
        crate::logic::world::team_ops::team_get_estimate_team_position(self, t as u32)
    }
    /// `TheRadar->hide(hide)`: the cut log's record, and (GitHub #35) the call as a client event.
    fn sa_radar_hide(&mut self, hide: bool) {
        self.sa_cut("radar_hide");
        self.events.push(crate::logic::client_event::ClientEvent::Radar(crate::logic::client_event::RadarCall::Hide(hide)));
    }
    fn sa_view_set_view_filter(&mut self, _filter: i32) -> bool {
        self.sa_cut("view_set_view_filter");
        Default::default()
    }
    fn sa_view_set_view_filter_mode(&mut self, _mode: i32) -> bool {
        self.sa_cut("view_set_view_filter_mode");
        Default::default()
    }
    fn sa_view_set_view_filter_pos(&mut self, _p: &Coord3D) {
        self.sa_cut("view_set_view_filter_pos");
    }
    fn sa_view_look_at(&mut self, _p: &Coord3D) {
        self.sa_cut("view_look_at");
    }
    fn sa_player_count(&self) -> i32 {
        self.players.player_count()
    }
    fn sa_nth_player(&self, i: i32) -> Option<PlayerH> {
        self.players.get_nth(i).map(|x| x.player_index)
    }
    fn sa_player_is_human(&self, p: PlayerH) -> bool {
        self.players.get_nth(p).is_some_and(|x| x.player_type == crate::common::game_common::PlayerType::Human)
    }
    fn sa_player_mask(&self, p: PlayerH) -> u32 {
        1u32.checked_shl(p as u32).unwrap_or(0)
    }
    fn sa_partition_shroud(&mut self, cover: bool, undo: bool, x: f32, y: f32, r: f32, mask: u32) {
        // (integration batch I26: was a cut or a default; the partition's own with `live_partition`)
        if !self.cuts.live_partition {
            return self.sa_cut("partition_shroud");
        }
        let mask = mask as crate::common::game_common::PlayerMaskType;
        let _ = self.with_partition(|pm, w| match (cover, undo) {
            (false, false) => pm.do_shroud_reveal(w, x, y, r, mask),
            (false, true) => pm.undo_shroud_reveal(w, x, y, r, mask),
            (true, false) => pm.do_shroud_cover(w, x, y, r, mask),
            (true, true) => pm.undo_shroud_cover(w, x, y, r, mask),
        });
    }

    fn sa_partition_reveal_map(&mut self, i: i32) {
        // (integration batch I26: was a cut or a default; the partition's own with `live_partition`)
        if !self.cuts.live_partition {
            return self.sa_cut("partition_reveal_map");
        }
        self.with_partition(|pm, w| pm.reveal_map_for_player(w, i));
    }

    fn sa_partition_reveal_map_permanently(&mut self, i: i32, undo: bool) {
        // (integration batch I26: was a cut or a default; the partition's own with `live_partition`)
        if !self.cuts.live_partition {
            return self.sa_cut("partition_reveal_map_permanently");
        }
        self.with_partition(|pm, w| if undo { pm.undo_reveal_map_for_player_permanently(w, i) } else { pm.reveal_map_for_player_permanently(w, i) });
    }

    fn sa_partition_shroud_map(&mut self, i: i32) {
        // (integration batch I26: was a cut or a default; the partition's own with `live_partition`)
        if !self.cuts.live_partition {
            return self.sa_cut("partition_shroud_map");
        }
        self.with_partition(|pm, w| pm.shroud_map_for_player(w, i));
    }

    fn sa_team_set_recruitable(&mut self, t: TeamH, recruitable: bool) {
        if let Some(team) = u32::try_from(t).ok().and_then(|t| self.teams.team_mut(t)) {
            team.set_recruitable(recruitable);
        }
    }
    fn sa_find_team(&self, name: &[u8]) -> Option<TeamH> {
        self.script_engine.get_team_named(self, name).ok().flatten()
    }
    fn sa_global_alpha(&self, _shroud: bool) -> u8 {
        Default::default()
    }
    fn sa_display_set_border_shroud_level(&mut self, _level: u8) {
        self.sa_cut("display_set_border_shroud_level");
    }
    fn sa_player_set_units_should_idle_or_resume(&mut self, p: PlayerH, idle: bool) {
        let _ = crate::logic::world::player_objects::player_set_units_should_idle_or_resume(self, p, idle);
    }
    fn sa_audio_pause_ambient(&mut self, pause: bool) {
        self.sa_audio(ScriptAudioEvent::PauseAmbient(pause));
    }
    fn sa_audio_remove_event_handle(&mut self, handle: i32) {
        self.sa_audio(ScriptAudioEvent::RemoveEventHandle(handle));
    }
    fn sa_set_current_track_name(&mut self, name: &[u8]) {
        // (integration batch I28: was a cut) `TheScriptEngine->setCurrentTrackName(name)`: the engine's (saved) `m_currentTrackName`
        self.script_engine.current_track_name = name.to_vec();
    }
    fn sa_contain_player_who_entered(&mut self, o: ObjH) -> u32 {
        // (integration batch I22: was `Default::default()`)
        let Some(c) = self.get_contain(o) else { return 0 };
        self.module_call(o, c, crate::logic::world::module_call::ModuleCall::GetPlayerWhoEntered).as_int(0) as u32
    }
    fn sa_contain_count(&mut self, o: ObjH) -> u32 {
        // (integration batch I22: was `Default::default()`)
        let Some(c) = self.get_contain(o) else { return 0 };
        self.module_call(o, c, crate::logic::world::module_call::ModuleCall::GetContainCount).as_int(0) as u32
    }
    fn sa_team_controlling_player(&self, t: TeamH) -> Option<PlayerH> {
        crate::logic::world::team_ops::team_get_controlling_player(self, t as u32).ok().flatten()
    }
    fn sa_contain_remove_all_contained(&mut self, o: ObjH, expose: bool) {
        if let Some(ct) = self.get_contain(o) {
            self.module_call(o, ct, crate::logic::world::module_call::ModuleCall::RemoveAllContained { expose_stealth_units: expose });
        }
    }
    fn sa_partition_garrison_candidates(&mut self, leader: ObjH, money_hacker: bool) -> Vec<ObjH> {
        // (integration batch I26: was a cut or a default; the partition's own with `live_partition`)
        // `iterateObjectsInRange(.., REALLY_FAR, FROM_CENTER_3D, filters, ITER_SORTED_NEAR_TO_FAR)`
        if !self.cuts.live_partition {
            self.sa_cut("partition_garrison_candidates");
            return Vec::new();
        }
        use crate::logic::world::partition::filters::PartitionFilter as F;
        // a money hacker looks for an internet centre, else buildings the team's player may garrison (the leader's team's player)
        let first = if money_hacker {
            F::AcceptByKindOf { must_be_set: kinds(crate::common::kind_of::KINDOF_FS_INTERNET_CENTER), must_be_clear: crate::common::kind_of::KindOfMaskType::new() }
        } else {
            F::GarrisonableByPlayer { player: self.get_controlling_player(leader), match_: true, command_source: crate::common::game_common::CommandSourceType::FromScript }
        };
        let filters = [first, F::SameMapStatus { obj: leader }];
        self.scan_from(leader, &filters)
    }

    fn sa_ai_exit(&mut self, o: ObjH, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_EXIT, cmd, |_| {});
    }
    fn sa_partition_unit_garrison_candidates(&mut self, unit: ObjH, money_hacker: bool) -> Vec<ObjH> {
        // (integration batch I26: was a cut or a default; the partition's own with `live_partition`)
        // `iterateObjectsInRange(.., REALLY_FAR, FROM_CENTER_3D, filters, ITER_SORTED_NEAR_TO_FAR)`
        if !self.cuts.live_partition {
            self.sa_cut("partition_unit_garrison_candidates");
            return Vec::new();
        }
        use crate::logic::world::partition::filters::PartitionFilter as F;
        let none = crate::common::kind_of::KindOfMaskType::new();
        let internet = kinds(crate::common::kind_of::KINDOF_FS_INTERNET_CENTER);
        let third = if money_hacker { F::AcceptByKindOf { must_be_set: internet, must_be_clear: none } } else { F::RejectByKindOf { must_be_set: internet, must_be_clear: none } };
        let filters = [F::AcceptByKindOf { must_be_set: kinds(crate::common::kind_of::KINDOF_STRUCTURE), must_be_clear: none }, F::SameMapStatus { obj: unit }, third];
        self.scan_from(unit, &filters)
    }

    fn sa_obj_controlling_player(&self, o: ObjH) -> Option<PlayerH> {
        self.get_controlling_player(o)
    }
    fn sa_obj_team(&self, o: ObjH) -> Option<TeamH> {
        self.get_team(o).map(|t| t as TeamH)
    }
    fn sa_obj_set_disabled(&mut self, o: ObjH, t: DisabledType) {
        self.set_disabled(o, t);
    }
    fn sa_obj_set_disabled_until(&mut self, o: ObjH, t: DisabledType, frame: u32) {
        self.set_disabled_until(o, t, frame);
    }
    fn sa_logic_deselect_object(&mut self, o: ObjH, mask: u32, affect_client: bool) {
        self.game_logic_deselect_object(o, mask as u16, affect_client);
    }
    fn sa_neutral_player(&self) -> Option<PlayerH> {
        self.players.get_nth(0).map(|x| x.player_index)
    }
    fn sa_obj_has_sticky_bomb_update(&self, o: ObjH) -> bool {
        // (integration batch I22: was `Default::default()`)
        self.find_module(o, b"StickyBombUpdate").is_some()
    }
    fn sa_obj_random_offset_on_perimeter(&mut self, o: ObjH) -> Coord3D {
        // `getGeometryInfo().makeRandomOffsetOnPerimeter(pos)`: an offset (from zero), with the logic's random values
        let Some(g) = self.find_object(o).map(|x| x.geometry_info.clone()) else { return Coord3D::default() };
        let mut p = Coord3D::default();
        g.make_random_offset_on_perimeter(&mut self.random, &mut p);
        p
    }
    fn sa_obj_transform_vector(&self, o: ObjH, v: &Coord3D) -> Coord3D {
        // (integration batch I22: was `Default::default()`)
        let m = crate::logic::weapon_host::WeaponHost::transform_matrix(self, o);
        let r = crate::ww::matrix3d::Matrix3D::transform_vector(&m, &crate::ww::vector3::Vector3 { x: v.x, y: v.y, z: v.z });
        Coord3D { x: r.x, y: r.y, z: r.z }
    }
    fn sa_sticky_bomb_init(&mut self, bomb: ObjH, target: ObjH, pos: &Coord3D) {
        // `initStickyBomb(target, NULL, &pos)`
        if let Some(m) = self.find_module(bomb, b"StickyBombUpdate") {
            self.module_call(bomb, m, crate::logic::world::module_call::ModuleCall::StickyBombInit { target, bomber: crate::common::game_type::INVALID_ID, specific_pos: Some(*pos) });
        }
    }
    fn sa_player_garrison_all(&mut self, p: PlayerH, garrison: bool, cmd: i32) {
        let _ = if garrison {
            crate::logic::world::player_objects::player_garrison_all_units(self, p, cmd)
        } else {
            crate::logic::world::player_objects::player_ungarrison_all_units(self, p, cmd)
        };
    }
    fn sa_control_bar_show(&mut self, _hide: bool, _immediate: bool) {
        self.sa_cut("control_bar_show");
    }
    fn sa_display_enable_letter_box(&mut self, _enable: bool) {
        self.sa_cut("display_enable_letter_box");
    }
    fn sa_view_set_fade_parameters(&mut self, _frames: i32, _direction: i32) {
        self.sa_cut("view_set_fade_parameters");
    }
    fn sa_view_filter_type(&self) -> i32 {
        Default::default()
    }
    fn sa_sky_box_set(&mut self, _draw: bool) {
        self.sa_cut("sky_box_set");
    }
    fn sa_snow_set_visible(&mut self, _visible: bool) {
        self.sa_cut("snow_set_visible");
    }
    fn sa_freeze_time(&mut self, freeze: bool) {
        if freeze {
            self.script_engine.do_freeze_time();
        } else {
            self.script_engine.do_unfreeze_time();
        }
    }
    fn sa_ai_has_cur_locomotor(&mut self, o: ObjH) -> bool {
        // (integration batch I22: was `Default::default()`)
        self.ai_ask(o, crate::logic::world::module_call::ModuleCall::AiHasCurLocomotor).is_some_and(|r| r.as_bool())
    }
    fn sa_locomotor_set_close_enough_dist(&mut self, o: ObjH, dist: f32) {
        self.ai_ask(o, crate::logic::world::module_call::ModuleCall::AiLocomotorSetCloseEnoughDist(dist));
    }
    fn sa_ui_set_superweapon_display_enabled(&mut self, _enabled: bool) {
        self.sa_cut("ui_set_superweapon_display_enabled");
    }
    fn sa_ui_object_superweapon_display(&mut self, _o: ObjH, _hide: bool) {
        self.sa_cut("ui_object_superweapon_display");
    }
    fn sa_audio_set_volume(&mut self, volume: f32, affect: i32) {
        self.sa_audio(ScriptAudioEvent::SetVolume { volume, affect });
    }
    fn sa_team_set_controlling_player(&mut self, t: TeamH, p: PlayerH) {
        let _ = crate::logic::world::team_ops::team_set_controlling_player(self, t as u32, Some(p));
    }
    fn sa_money_count(&self, p: PlayerH) -> u32 {
        self.players.get_nth(p).map_or(0, |x| x.money.count_money())
    }
    fn sa_money_withdraw(&mut self, p: PlayerH, amount: u32) {
        let _ = crate::logic::world::player_objects::player_money_withdraw(self, p, amount, true);
    }
    fn sa_money_deposit(&mut self, p: PlayerH, amount: u32) {
        let _ = crate::logic::world::player_objects::player_money_deposit(self, p, amount, true);
    }
    fn sa_ui_add_named_timer(&mut self, _name: &[u8], _text: &[u16], _countdown: bool) {
        self.sa_cut("ui_add_named_timer");
    }
    fn sa_ui_remove_named_timer(&mut self, _name: &[u8]) {
        self.sa_cut("ui_remove_named_timer");
    }
    fn sa_ui_show_named_timer_display(&mut self, _show: bool) {
        self.sa_cut("ui_show_named_timer_display");
    }
    fn sa_find_special_power(&self, name: &[u8]) -> Option<SpecialPowerH> {
        self.statics.special_power_store.find_special_power_template(name).map(|t| t.id() as SpecialPowerH)
    }
    fn sa_obj_has_special_power_module(&mut self, o: ObjH, power: SpecialPowerH) -> bool {
        // (integration batch I23: was a stub)
        self.get_special_power_module(o, u32::try_from(power).ok()).is_some()
    }

    fn sa_spm_pause_countdown(&mut self, o: ObjH, power: SpecialPowerH, pause: bool) {
        // (integration batch I23: was a stub)
        let Some(m) = self.get_special_power_module(o, u32::try_from(power).ok()) else { return Default::default() };
        self.module_call(o, m, crate::logic::world::module_call::ModuleCall::PauseCountdown { disabling: pause });
    }

    fn sa_spm_ready_frame(&mut self, o: ObjH, power: SpecialPowerH) -> u32 {
        // (integration batch I23: was a stub)
        let Some(m) = self.get_special_power_module(o, u32::try_from(power).ok()) else { return Default::default() };
        self.module_call(o, m, crate::logic::world::module_call::ModuleCall::GetReadyFrame).as_int(0) as u32
    }

    fn sa_spm_set_ready_frame(&mut self, o: ObjH, power: SpecialPowerH, frame: u32) {
        // (integration batch I23: was a stub)
        let Some(m) = self.get_special_power_module(o, u32::try_from(power).ok()) else { return Default::default() };
        self.module_call(o, m, crate::logic::world::module_call::ModuleCall::SetReadyFrame { frame });
    }

    fn sa_spm_do_at_location(&mut self, o: ObjH, power: SpecialPowerH, p: &Coord3D, angle: f32, options: u32) {
        // (integration batch I23: was a stub)
        let Some(m) = self.get_special_power_module(o, u32::try_from(power).ok()) else { return Default::default() };
        self.module_call(o, m, crate::logic::world::module_call::ModuleCall::DoSpecialPowerAtLocation { loc: *p, angle, command_options: options });
    }

    fn sa_frame(&self) -> u32 {
        self.frame()
    }
    fn sa_player_teams(&self, p: PlayerH) -> Vec<Vec<TeamH>> {
        let protos = self.players.get_nth(p).map(|x| x.team_prototypes.clone()).unwrap_or_default();
        protos.into_iter().map(|pr| self.teams.instances(pr).into_iter().map(|t| t as TeamH).collect()).collect()
    }
    fn sa_special_power_radius_cursor(&self, power: SpecialPowerH) -> f32 {
        // (integration batch I23: was a stub)
        u32::try_from(power).ok().and_then(|p| self.statics.special_power_store.find_special_power_template_by_id(p)).map_or(0.0, |t| t.radius_cursor_radius())
    }

    fn sa_special_power_type(&self, power: SpecialPowerH) -> i32 {
        self.statics.special_power_store.find_special_power_template_by_id(power as u32).map_or(-1, |t| t.special_power_type())
    }
    fn sa_spm_is_ready(&mut self, o: ObjH, power: SpecialPowerH) -> bool {
        // (integration batch I23: was a stub)
        let Some(m) = self.get_special_power_module(o, u32::try_from(power).ok()) else { return Default::default() };
        self.module_call(o, m, crate::logic::world::module_call::ModuleCall::IsReady).as_bool()
    }

    fn sa_spm_reference_template(&mut self, o: ObjH, power: SpecialPowerH) -> Option<TemplateH> {
        // (integration batch I23: was a stub)
        let m = self.get_special_power_module(o, u32::try_from(power).ok())?;
        match self.module_call(o, m, crate::logic::world::module_call::ModuleCall::GetReferenceThingTemplate) {
            crate::logic::world::module_call::ModuleReply::Int(t) => Some(t as TemplateH),
            _ => None,
        }
    }

    fn sa_spm_do_at_object(&mut self, o: ObjH, power: SpecialPowerH, target: ObjH, options: u32) {
        // (integration batch I23: was a stub)
        let Some(m) = self.get_special_power_module(o, u32::try_from(power).ok()) else { return Default::default() };
        self.module_call(o, m, crate::logic::world::module_call::ModuleCall::DoSpecialPowerAtObject { target, command_options: options });
    }

    fn sa_player_compute_superweapon_target(&mut self, p: PlayerH, power: SpecialPowerH, index: i32, weight: f32) -> Option<Coord3D> {
        // (integration batch I26: was a cut) `player->computeSuperweaponTarget(power, &pos, playerIndex, weight)`: its AI's
        let mut pos = Coord3D::default();
        let found = crate::logic::world::player_ops::player_compute_superweapon_target(self, p, u32::try_from(power).ok()?, &mut pos, index, weight).unwrap_or(false);
        found.then_some(pos)
    }

    fn sa_player_closest_construction_zone(&mut self, p: PlayerH, t: TemplateH) -> Option<Coord3D> {
        // (integration batch I26: was a cut) `player->calcClosestConstructionZoneLocation(template, &pos)`: its AI's
        let mut pos = Coord3D::default();
        let found = crate::logic::world::player_ops::player_calc_closest_construction_zone_location(self, p, t as crate::logic::world::team_ops::TemplateRef, &mut pos).unwrap_or(false);
        found.then_some(pos)
    }

    fn sa_button_name(&self, b: ButtonH) -> Vec<u8> {
        self.cuts.command_buttons.get(b as usize).map(|x| x.name.clone()).unwrap_or_default()
    }
    fn sa_obj_do_command_button(&mut self, o: ObjH, b: ButtonH, cmd: i32) {
        // (integration batch I23: was a stub)
        if let Ok(b) = usize::try_from(b) {
            self.do_command_button(o, b, cmd);
        }
    }

    fn sa_obj_do_command_button_at_object(&mut self, o: ObjH, b: ButtonH, target: ObjH, cmd: i32) {
        // (integration batch I23: was a stub)
        if let Ok(b) = usize::try_from(b) {
            self.do_command_button_at_object(o, b, Some(target), cmd);
        }
    }

    fn sa_obj_do_command_button_at_position(&mut self, o: ObjH, b: ButtonH, p: &Coord3D, cmd: i32) {
        // (integration batch I23: was a stub)
        if let Ok(b) = usize::try_from(b) {
            self.do_command_button_at_position(o, b, p, cmd);
        }
    }

    fn sa_obj_do_command_button_using_waypoints(&mut self, o: ObjH, b: ButtonH, w: WaypointH, cmd: i32) {
        // (integration batch I23: was a stub)
        if let (Ok(b), Ok(w)) = (usize::try_from(b), u32::try_from(w)) {
            self.do_command_button_using_waypoints(o, b, w, cmd);
        }
    }

    fn sa_group_do_command_button(&mut self, g: GroupH, b: ButtonH, cmd: i32) {
        // (integration batch I23: was a stub)
        let (Ok(g), Ok(b)) = (u32::try_from(g), u32::try_from(b)) else { return };
        self.with_group(g, |gr, w| gr.group_do_command_button(w, b, crate::logic::ai::ai_group::GroupButtonCall::Plain, cmd));
    }

    fn sa_group_do_command_button_at_object(&mut self, g: GroupH, b: ButtonH, target: ObjH, cmd: i32) {
        // (integration batch I23: was a stub)
        let (Ok(g), Ok(b)) = (u32::try_from(g), u32::try_from(b)) else { return };
        self.with_group(g, |gr, w| gr.group_do_command_button(w, b, crate::logic::ai::ai_group::GroupButtonCall::AtObject(target), cmd));
    }

    fn sa_group_do_command_button_at_position(&mut self, g: GroupH, b: ButtonH, p: &Coord3D, cmd: i32) {
        // (integration batch I23: was a stub)
        let (Ok(g), Ok(b)) = (u32::try_from(g), u32::try_from(b)) else { return };
        self.with_group(g, |gr, w| gr.group_do_command_button(w, b, crate::logic::ai::ai_group::GroupButtonCall::AtPosition(*p), cmd));
    }

    /// `TheRadar->refreshTerrain(TheTerrainLogic)`: the cut log's record, and (GitHub #35) the call as a client event.
    fn sa_radar_refresh_terrain(&mut self) {
        self.sa_cut("radar_refresh_terrain");
        self.events.push(crate::logic::client_event::ClientEvent::Radar(crate::logic::client_event::RadarCall::RefreshTerrain));
    }
    fn sa_view_set_default_view(&mut self, _pitch: f32, _angle: f32, _max_height: f32) {
        self.sa_cut("view_set_default_view");
    }
    fn sa_ai_idle(&mut self, o: ObjH, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_IDLE, cmd, |_| {});
    }
    fn sa_group_idle(&mut self, g: GroupH, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_idle(w, cmd));
    }
    fn sa_team_name(&self, t: TeamH) -> Vec<u8> {
        let teams = &self.teams;
        teams.team(t as u32).and_then(|x| teams.prototype(x.proto)).map(|p| p.name.clone()).unwrap_or_default()
    }
    fn sa_team_set_override_team_relationship(&mut self, t: TeamH, other: TeamH, r: i32) {
        let _ = crate::logic::world::team_ops::team_set_override_team_relationship(self, t as u32, other as u32, rel(r));
    }
    fn sa_team_remove_override_team_relationship(&mut self, t: TeamH, other: TeamH) {
        let _ = crate::logic::world::team_ops::team_remove_override_team_relationship(self, t as u32, other as u32);
    }
    fn sa_team_set_override_player_relationship(&mut self, t: TeamH, p: PlayerH, r: i32) {
        let _ = crate::logic::world::team_ops::team_set_override_player_relationship(self, t as u32, p, rel(r));
    }
    fn sa_team_remove_override_player_relationship(&mut self, t: TeamH, p: PlayerH) {
        let _ = crate::logic::world::team_ops::team_remove_override_player_relationship(self, t as u32, p);
    }
    fn sa_player_with_name_key(&self, name: &[u8]) -> Option<PlayerH> {
        let key = (0..self.players.player_count()).find(|&i| self.players.get_nth(i).is_some_and(|x| self.names.key_to_name(crate::common::name_key_generator::NameKeyType(x.name_key as u32)) == name));
        key
    }
    fn sa_player_set_team_relationship(&mut self, p: PlayerH, t: TeamH, r: i32) {
        let _ = crate::logic::world::player_ops::player_set_team_relationship(self, p, Some(t as u32), rel(r));
    }
    fn sa_player_remove_team_relationship(&mut self, p: PlayerH, t: TeamH) {
        let _ = crate::logic::world::player_ops::player_remove_team_relationship(self, p, Some(t as u32));
    }
    fn sa_find_script(&self, name: &[u8]) -> bool {
        self.script_engine.find_script(self, name).is_some()
    }
    fn sa_append_sequential_script(&mut self, team: Option<TeamH>, object: ObjectId, script: &[u8], times_to_loop: i32) {
        // (integration batch I25: was a cut) a new `SequentialScript` of the team or object, the script found by name, appended to the
        // running engine's (the world's while an action runs, `execute_action_with`)
        let Some(r) = self.script_engine.find_script(self, script) else { return };
        let mut s = crate::logic::world::script_engine::SequentialScript::new(r);
        s.team_to_exec_on = team;
        s.object_id = object;
        s.times_to_loop = times_to_loop;
        self.script_engine.append_sequential_script(&s);
    }

    fn sa_remove_all_sequential_scripts_obj(&mut self, o: ObjH) {
        self.script_engine.remove_all_sequential_scripts_of_object(o);
    }

    fn sa_remove_all_sequential_scripts_team(&mut self, t: TeamH) {
        // `removeAllSequentialScripts(team)` is upstream's `notifyOfTeamDestruction(team)`
        self.script_engine.notify_of_team_destruction(t);
    }

    fn sa_set_sequential_timer_obj(&mut self, o: ObjH, frames: i32) {
        self.script_engine.set_sequential_timer_object(o, frames);
    }

    fn sa_set_sequential_timer_team(&mut self, t: TeamH, frames: i32) {
        self.script_engine.set_sequential_timer_team(t, frames);
    }

    fn sa_obj_has_waypoint_following_weapon(&self, o: ObjH) -> bool {
        // (integration batch I22: `findWaypointFollowingCapableWeapon() != NULL`; was always false)
        let Some(obj) = self.objects.find(o) else { return false };
        let templates = self.statics.weapon_store.templates();
        (0..crate::common::game_type::WEAPONSLOT_COUNT)
            .any(|i| obj.weapon_set.get_weapon_in_weapon_slot(i).is_some_and(|w| templates.get(w.template()).is_some_and(|t| t.capable_of_following_waypoint)))
    }
    fn sa_weapon_force_fire(&mut self, o: ObjH, p: &Coord3D) -> Option<ObjH> {
        // (integration batch I22: that weapon's `forceFireWeapon(obj, &pos)`, the projectile)
        match crate::logic::world::object_weapons::force_fire_waypoint_following_weapon(self, o, p) {
            Ok(projectile) => projectile,
            Err(e) => {
                self.combat_error("Weapon::forceFireWeapon", e);
                None
            }
        }
    }
    fn sa_water_handle(&self, name: &[u8]) -> Option<i32> {
        self.terrain.trigger_area_by_name(&AsciiString::try_from_bytes(name).ok()?).map(|i| i as i32)
    }
    fn sa_set_water_height(&mut self, w: i32, height: f32, damage: f32, force: bool) {
        let water = usize::try_from(w).ok().map(crate::logic::terrain_logic::WaterHandle::Polygon);
        let _ = self.terrain_set_water_height(water, height, damage, force);
    }
    fn sa_change_water_height_over_time(&mut self, w: i32, height: f32, time: f32, damage: f32) {
        let water = usize::try_from(w).ok().map(crate::logic::terrain_logic::WaterHandle::Polygon);
        let _ = self.terrain_change_water_height_over_time(water, height, time, damage);
    }
    fn sa_set_active_boundary(&mut self, border: i32) {
        self.terrain_set_active_boundary(border);
    }
    fn sa_refresh_shroud_for_local_player(&mut self) {
        self.sa_cut("refresh_shroud_for_local_player");
    }
    fn sa_ui_select_drawable(&mut self, _o: ObjH) {
        self.sa_cut("ui_select_drawable");
    }
    fn sa_view_position(&self) -> Coord3D {
        Default::default()
    }
    fn sa_view_shake(&mut self, _p: &Coord3D, _shake_type: i32) {
        self.sa_cut("view_shake");
    }
    fn sa_contained(&mut self, o: ObjH) -> Vec<ObjH> {
        // (integration batch I22: was `Default::default()`)
        let Some(c) = self.get_contain(o) else { return Vec::new() };
        match self.module_call(o, c, crate::logic::world::module_call::ModuleCall::GetContainedItems) {
            crate::logic::world::module_call::ModuleReply::Ids(ids) => ids,
            _ => Vec::new(),
        }
    }
    fn sa_logic_set_buildable_status_override(&mut self, t: TemplateH, status: i32) {
        let name = self.statics.thing_factory.find_by_template_id(t as u16).map(|x| x.data.get_name().to_vec());
        crate::logic::world::game_logic_update::set_buildable_status_override(&mut self.game_logic, name.as_deref(), status);
    }
    fn sa_obj_has_cave(&self, o: ObjH) -> bool {
        // (integration batch I22: was `Default::default()`)
        // only `CaveContain` answers `getCaveInterface()`
        self.find_module(o, b"CaveContain").is_some()
    }
    fn sa_cave_try_set_index(&mut self, o: ObjH, index: i32) {
        if let Some(ct) = self.get_contain(o) {
            self.module_call(o, ct, crate::logic::world::module_call::ModuleCall::TryToSetCaveIndex(index));
        }
    }
    fn sa_obj_has_warehouse(&self, o: ObjH) -> bool {
        // (integration batch I22: was `Default::default()`)
        self.find_module(o, b"SupplyWarehouseDockUpdate").is_some()
    }
    fn sa_warehouse_set_cash_value(&mut self, o: ObjH, value: i32) {
        if let Some(m) = self.find_module(o, b"SupplyWarehouseDockUpdate") {
            self.module_call(o, m, crate::logic::world::module_call::ModuleCall::SupplyWarehouseSetCashValue(value));
        }
    }
    fn sa_audio_set_event_enabled(&mut self, name: &[u8], enable: bool) {
        self.sa_audio(ScriptAudioEvent::SetEventEnabled { name: name.to_vec(), enable });
    }
    fn sa_audio_remove_disabled_events(&mut self) {
        self.sa_audio(ScriptAudioEvent::RemoveDisabledEvents);
    }
    fn sa_audio_remove_event_named(&mut self, name: &[u8]) {
        self.sa_audio(ScriptAudioEvent::RemoveEventNamed(name.to_vec()));
    }
    fn sa_audio_set_event_volume_override(&mut self, name: &[u8], volume: f32) {
        self.sa_audio(ScriptAudioEvent::SetEventVolumeOverride { name: name.to_vec(), volume });
    }
    fn sa_set_topple_direction(&mut self, name: &[u8], dir: &Coord3D) {
        // (the running engine is the world's while an action runs, `execute_action_with`)
        if let Err(e) = self.script_engine.set_topple_direction(name, Some(*dir)) {
            self.cuts.team_live_errors.push(format!("ScriptEngine::setToppleDirection: {e:?}"));
        }
    }
    /// `TheRadar->forceOn(on)`: the cut log's record, and (GitHub #35) the call as a client event.
    fn sa_radar_force_on(&mut self, on: bool) {
        self.sa_cut("radar_force_on");
        self.events.push(crate::logic::client_event::ClientEvent::Radar(crate::logic::client_event::RadarCall::ForceOn(on)));
    }
    fn sa_partition_closest_of_template(&mut self, p: &Coord3D, t: TemplateH, trig: TriggerH, obj: ObjH) -> Option<(ObjH, f32)> {
        // (integration batch I26: was a cut or a default; the partition's own with `live_partition`)
        if !self.cuts.live_partition {
            return None;
        }
        // `getClosestObject(&pos, REALLY_FAR, FROM_CENTER_2D, {PartitionFilterThing(t), PartitionFilterPolygonTrigger(trig),
        // PartitionFilterSameMapStatus(obj)}, &dist)`
        use crate::logic::world::partition::filters::PartitionFilter as F;
        let filters = [F::Thing { template_key: t, match_: true }, F::PolygonTrigger { trigger: trig }, F::SameMapStatus { obj }];
        let c = self.with_partition(|pm, w| pm.get_closest_object_at(w, p, REALLY_FAR, crate::logic::world::partition::DistanceCalculationType::FromCenter2D, &filters));
        c.object.map(|o| (o, c.dist))
    }

    fn sa_object_types(&self, name: &[u8]) -> Option<Vec<Vec<u8>>> {
        let t = &self.script_engine.all_object_type_lists[self.script_engine.get_object_types(name)?];
        Some((0..t.list_size() as i32).map(|i| t.get_nth_in_list(i).str().to_vec()).collect())
    }
    fn sa_ai_move_to_object(&mut self, o: ObjH, target: Option<ObjH>, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_MOVE_TO_OBJECT, cmd, |q| q.obj = target.unwrap_or(crate::common::game_type::INVALID_ID));
    }
    fn sa_find_upgrade(&self, name: &[u8]) -> Option<i32> {
        // `findUpgrade(name)`: an upgrade whose name it is (a name never made a key names none)
        self.statics.upgrade_center.templates().find(|u| u.upgrade_name() == name).map(|u| u.upgrade_name_key().0 as i32)
    }
    fn sa_obj_give_upgrade(&mut self, o: ObjH, upgrade: i32) {
        let mask = self.statics.upgrade_center.find_upgrade_by_key(crate::common::name_key_generator::NameKeyType(upgrade as u32)).map(|u| u.upgrade_mask());
        self.give_upgrade(o, mask.as_ref());
    }
    fn sa_group_center(&mut self, g: GroupH) -> Option<Coord3D> {
        // (integration batch I22: was `Default::default()`)
        let (pos, any) = self.with_group(u32::try_from(g).ok()?, |gr, w| gr.get_center(w))?;
        any.then_some(pos)
    }
    fn sa_partition_nearest_group_with_value(&mut self, index: i32, src: &Coord3D, value: i32, greater: bool) -> Option<Coord3D> {
        // (integration batch I26: was a cut or a default; the partition's own with `live_partition`)
        if !self.cuts.live_partition {
            self.sa_cut("partition_nearest_group_with_value");
            return None;
        }
        // `getNearestGroupWithValue(playerIndex, ALLOW_ENEMIES, VOT_CashValue, &src, value, greater, &out)`: the location it sets
        let enemies = crate::logic::world::partition::filters::allow_player::ENEMIES;
        let cash = crate::logic::world::partition::shroud::ValueOrThreat::CashValue;
        self.with_partition(|pm, w| pm.get_nearest_group_with_value(w, index, enemies, cash, src, value, greater)).0
    }

    fn sa_group_attack_move_to_position(&mut self, g: GroupH, p: &Coord3D, max_shots: i32, cmd: i32) {
        self.script_group(g, |gr, w| gr.group_attack_move_to_position(w, p, max_shots, cmd));
    }
    fn sa_button_special_power_id(&self, b: ButtonH) -> Option<u32> {
        self.cuts.command_buttons.get(b as usize).and_then(|x| x.special_power)
    }
    fn sa_group_special_power_source(&mut self, g: GroupH, id: u32) -> Option<ObjH> {
        // (integration batch I23: was a stub)
        self.with_group(u32::try_from(g).ok()?, |gr, w| gr.get_special_power_source_object(w, id)).flatten()
    }

    fn sa_group_command_button_source(&mut self, g: GroupH, command_type: i32) -> Option<ObjH> {
        // (integration batch I23: was a stub)
        self.with_group(u32::try_from(g).ok()?, |gr, w| gr.get_command_button_source_object(w, command_type)).flatten()
    }

    fn sa_partition_most_valuable_target(&mut self, p: &Coord3D, range: f32, from: ButtonFrom) -> Option<ObjH> {
        // (integration batch I27: was a cut) `iterateObjectsInRange(&pos, range, FROM_CENTER_2D, {the player's enemies, valid targets of
        // the button from the source, its map status}, ITER_SORTED_EXPENSIVE_TO_CHEAP)`'s first, with `live_partition`
        if !self.cuts.live_partition {
            self.sa_cut("partition_most_valuable_target");
            return None;
        }
        use crate::logic::world::partition::filters::{allow_player, PartitionFilter as F};
        let player = from.player?; // (the action has checked it)
        let filters = [
            F::PlayerAffiliation { player, affiliation: allow_player::ENEMIES, match_: true },
            F::ValidCommandButtonTarget { source: from.src, command_button: from.button, match_: true, command_source: crate::common::game_common::CommandSourceType::FromScript },
            F::SameMapStatus { obj: from.src },
        ];
        let mut it = self.with_partition(|pm, w| {
            pm.iterate_objects_in_range_at(w, p, range, crate::logic::world::partition::DistanceCalculationType::FromCenter2D, &filters, crate::logic::object_iter::IterOrderType::SortedExpensiveToCheap)
        });
        it.first()
    }

    fn sa_button_is_valid_to_use_on(&mut self, b: ButtonH, src: ObjH, target: Option<ObjH>, cmd: i32) -> bool {
        // (integration batch I27: was a default) `button->isValidToUseOn(src, target, NULL, cmdSource)`
        let Ok(b) = usize::try_from(b) else { return false };
        self.command_button_is_valid_to_use_on(b, src, target, None, crate::common::game_common::CommandSourceType::from_i32(cmd))
    }

    fn sa_partition_closest_for_button(&mut self, p: &Coord3D, search: ButtonSearch, from: ButtonFrom) -> Option<(ObjH, f32)> {
        // (integration batch I27: was a default) `getClosestObject(&pos, REALLY_FAR, FROM_CENTER_2D, filters, &dist)` with
        // `live_partition`, the filters as ScriptActions.cpp's searches build them
        if !self.cuts.live_partition {
            return None;
        }
        use crate::logic::world::partition::filters::{allow_player, PartitionFilter as F};
        // (a `NULL` player is dereferenced by the affiliation filter upstream)
        let Some(player) = from.player else {
            self.cuts.team_live_errors.push("PartitionFilterPlayerAffiliation: NULL player".into());
            return None;
        };
        let none = crate::common::kind_of::KindOfMaskType::new();
        let structure = kinds(crate::common::kind_of::KINDOF_STRUCTURE);
        let enemies = F::PlayerAffiliation { player, affiliation: allow_player::ENEMIES, match_: true };
        let valid = F::ValidCommandButtonTarget { source: from.src, command_button: from.button, match_: true, command_source: crate::common::game_common::CommandSourceType::FromScript };
        let same = F::SameMapStatus { obj: from.src };
        let filters: Vec<F> = match search {
            ButtonSearch::Enemy => vec![enemies, valid, same],
            ButtonSearch::GarrisonedBuilding => vec![enemies, F::AcceptByKindOf { must_be_set: structure, must_be_clear: none }, F::Garrisonable { match_: true }, valid, same],
            ButtonSearch::KindOf(bit) => vec![enemies, F::AcceptByKindOf { must_be_set: kinds(bit), must_be_clear: none }, valid, same],
            ButtonSearch::Building => vec![enemies, F::AcceptByKindOf { must_be_set: structure, must_be_clear: none }, valid, same],
            ButtonSearch::BuildingClass(bit) => {
                vec![enemies, F::AcceptByKindOf { must_be_set: structure, must_be_clear: none }, F::AcceptByKindOf { must_be_set: kinds(bit), must_be_clear: none }, valid, same]
            }
            ButtonSearch::Template(tt) => {
                let either = F::PlayerAffiliation { player, affiliation: allow_player::ENEMIES | allow_player::NEUTRAL, match_: true };
                vec![either, F::Thing { template_key: tt, match_: true }, valid, same]
            }
            ButtonSearch::Unmanned => {
                let either = F::PlayerAffiliation { player, affiliation: allow_player::ENEMIES | allow_player::NEUTRAL, match_: true };
                vec![either, F::UnmannedObject { match_: true }]
            }
        };
        let c = self.with_partition(|pm, w| pm.get_closest_object_at(w, p, REALLY_FAR, crate::logic::world::partition::DistanceCalculationType::FromCenter2D, &filters));
        c.object.map(|o| (o, c.dist))
    }

    fn sa_group_do_command_button_at_object_opt(&mut self, g: GroupH, b: ButtonH, target: Option<ObjH>, cmd: i32) {
        // (integration batch I23: was a stub)
        let (Ok(g), Ok(b)) = (u32::try_from(g), u32::try_from(b)) else { return };
        self.with_group(g, |gr, w| gr.group_do_command_button(w, b, crate::logic::ai::ai_group::GroupButtonCall::AtObject(target.unwrap_or(crate::common::game_type::INVALID_ID)), cmd));
    }

    fn sa_view_camera_mod_freeze(&mut self, _time: bool) {
        self.sa_cut("view_camera_mod_freeze");
    }
    fn sa_view_camera_mod_final_time_multiplier(&mut self, _m: i32) {
        self.sa_cut("view_camera_mod_final_time_multiplier");
    }
    fn sa_view_camera_mod_rolling_average(&mut self, _n: i32) {
        self.sa_cut("view_camera_mod_rolling_average");
    }
    fn sa_view_set_time_multiplier(&mut self, _m: i32) {
        self.sa_cut("view_set_time_multiplier");
    }
    fn sa_audio_pause(&mut self, pause: bool, affect: i32) {
        self.sa_audio(ScriptAudioEvent::Pause { pause, affect });
    }
    fn sa_global_fps_limit(&self) -> i32 {
        Default::default()
    }
    fn sa_engine_set_fps_limit(&mut self, _fps: i32) {
        self.sa_cut("engine_set_fps_limit");
    }
    fn sa_set_use_fps_limit(&mut self, _use_limit: bool) {
        self.sa_cut("set_use_fps_limit");
    }
    fn sa_player_add_skill_points(&mut self, p: PlayerH, delta: i32) {
        let _ = crate::logic::world::player_ops::player_add_skill_points(self, p, delta);
    }
    fn sa_player_rank_level(&self, p: PlayerH) -> i32 {
        self.players.get_nth(p).map_or(0, |x| x.rank_level)
    }
    fn sa_player_set_rank_level(&mut self, p: PlayerH, level: i32) {
        let _ = crate::logic::world::player_ops::player_set_rank_level(self, p, level);
    }
    fn sa_logic_set_rank_level_limit(&mut self, level: i32) {
        self.cuts.player_host.rank_level_limit = level;
    }
    fn sa_science_from_name(&self, name: &[u8]) -> Option<i32> {
        let store = &self.statics.science_store;
        store.infos().map(|i| i.science()).find(|s| store.get_internal_name_for_science(&self.names, *s) == name).map(|s| s.0)
    }
    fn sa_player_grant_science(&mut self, p: PlayerH, science: i32) {
        let _ = crate::logic::world::player_ops::player_grant_science(self, p, crate::common::science::ScienceType(science));
    }
    fn sa_player_purchase_science(&mut self, p: PlayerH, science: i32) {
        let _ = crate::logic::world::player_ops::player_attempt_to_purchase_science(self, p, crate::common::science::ScienceType(science));
    }
    fn sa_player_science_availability_from_string(&self, _p: PlayerH, name: &[u8]) -> Option<i32> {
        // (integration batch I22: was `Default::default()`)
        use crate::logic::world::player_ops::{player_get_science_availability_type_from_string as from, ScienceAvailabilityType as S};
        match from(name) {
            S::Invalid => None,
            s => Some(s as i32),
        }
    }
    fn sa_player_set_science_availability(&mut self, p: PlayerH, science: i32, availability: i32) {
        use crate::logic::world::player_ops::ScienceAvailabilityType as A;
        let ty = match availability {
            0 => A::Available,
            1 => A::Disabled,
            2 => A::Hidden,
            _ => A::Invalid,
        };
        if let Err(e) = crate::logic::world::player_ops::player_set_science_availability(self, p, crate::common::science::ScienceType(science), ty) {
            self.cuts.team_live_errors.push(format!("Player::setScienceAvailability: {e:?}"));
        }
    }
    fn sa_group_set_emoticon(&mut self, g: GroupH, name: &[u8], frames: i32) {
        self.script_group(g, |gr, w| gr.group_set_emoticon(w, name, frames));
    }
    fn sa_drawable_set_emoticon(&mut self, _o: ObjH, _name: &[u8], _frames: i32) {
        self.sa_cut("drawable_set_emoticon");
    }
    fn sa_object_type_list_maintenance(&mut self, list: &[u8], object_type: &[u8], add: bool) {
        // (integration batch I26: was a cut) the running engine's (the world's while an action runs)
        self.script_engine.do_object_type_list_maintenance(list, object_type, add);
    }

    fn sa_named_map_reveal(&mut self, look: &[u8], waypoint: &[u8], radius: f32, player: &[u8]) {
        // (integration batch I26: was a cut) `createNamedMapReveal(..)` then `doNamedMapReveal(look)` on the running engine
        self.script_engine.create_named_map_reveal(look, waypoint, radius, player);
        let mut se = std::mem::take(&mut self.script_engine);
        se.do_named_map_reveal(self, look);
        self.script_engine = se;
    }

    fn sa_undo_named_map_reveal(&mut self, look: &[u8]) {
        // (integration batch I26: was a cut) `undoNamedMapReveal(look)` then `removeNamedMapReveal(look)`
        let mut se = std::mem::take(&mut self.script_engine);
        se.undo_named_map_reveal(self, look);
        self.script_engine = se;
        self.script_engine.remove_named_map_reveal(look);
    }

    /// `TheEva->setEvaEnabled(enabled)`: the cut log's record, and (GitHub #55) the call as a client event, for the client's
    /// `Eva` (`client::eva`).
    fn sa_eva_set_enabled(&mut self, enabled: bool) {
        self.sa_cut("eva_set_enabled");
        self.events.push(crate::logic::client_event::ClientEvent::EvaSetEnabled(enabled));
    }
    fn sa_logic_set_option(&mut self, which: i32, enabled: bool) {
        self.game_logic_write(crate::logic::world::object_cuts::GameLogicWrite::Option { which, enabled });
    }
    fn sa_logic_set_hulk_max_lifetime_override(&mut self, frames: i32) {
        // (integration batch I20: through the frame-safe write; it wrote `game_logic` directly, which the frame holds while scripts run,
        // so the value was lost)
        self.game_logic_write(crate::logic::world::object_cuts::GameLogicWrite::HulkMaxLifetimeOverride(frames));
    }
    fn sa_view_camera_slave_mode(&mut self, _slave: Option<(&[u8], &[u8])>) {
        self.sa_cut("view_camera_slave_mode");
    }
    fn sa_view_add_camera_shake(&mut self, _p: &Coord3D, _radius: f32, _duration: f32, _amplitude: f32) {
        self.sa_cut("view_add_camera_shake");
    }
    fn sa_ai_face_object(&mut self, o: ObjH, target: ObjH, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_FACE_OBJECT, cmd, |q| q.obj = target);
    }
    fn sa_ai_face_position(&mut self, o: ObjH, p: &Coord3D, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_FACE_POSITION, cmd, |q| q.pos = *p);
    }
    fn sa_player_set_skillset(&mut self, p: PlayerH, skillset: i32) {
        if let Err(e) = crate::logic::world::player_ops::player_friend_set_skillset(self, p, skillset) {
            self.cuts.team_live_errors.push(format!("Player::friend_setSkillset: {e:?}"));
        }
    }
    fn sa_player_guard_supply_center(&mut self, p: PlayerH, t: TeamH, supplies: i32) {
        let r = crate::logic::world::player_ops::player_guard_supply_center(self, p, u32::try_from(t).ok(), supplies);
        self.script_player("Player::guardSupplyCenter", r);
    }
    fn sa_ai_guard_tunnel_network(&mut self, o: ObjH, mode: i32, cmd: i32) {
        self.script_order(o, crate::logic::ai::ai_commands::AICMD_GUARD_TUNNEL_NETWORK, cmd, |q| q.int_value = mode);
    }
    fn sa_template_command_set_string(&self, t: TemplateH) -> Vec<u8> {
        // (integration batch I22: was `Default::default()`)
        u16::try_from(t).ok().and_then(|t| self.statics.thing_factory.find_by_template_id(t)).map(|o| o.data.friend_get_command_set_string().to_vec()).unwrap_or_default()
    }
    fn sa_logic_set_control_bar_override(&mut self, command_set: &[u8], slot: i32, b: Option<ButtonH>) {
        let button = b.and_then(|b| u32::try_from(b).ok());
        self.game_logic_write(crate::logic::world::object_cuts::GameLogicWrite::ControlBarOverride { command_set: command_set.to_vec(), slot, button });
    }
    fn sa_player_set_skill_points_modifier(&mut self, p: PlayerH, m: f32) {
        if let Some(x) = self.players.get_nth_mut(p) {
            x.skill_points_modifier = m;
        }
    }
    fn sa_view_set_guard_band_bias(&mut self, _x: f32, _y: f32) {
        self.sa_cut("view_set_guard_band_bias");
    }
    fn sa_all_objects(&self) -> Vec<ObjH> {
        self.objects.ids_in_order()
    }
    fn sa_obj_set_receiving_difficulty_bonus(&mut self, o: ObjH, b: bool) {
        self.set_receiving_difficulty_bonus(o, b);
    }
    fn sa_set_objects_should_receive_difficulty_bonus(&mut self, b: bool) {
        self.script_engine.objects_should_receive_difficulty_bonus = b;
    }
    fn sa_set_choose_victim_always_uses_normal(&mut self, b: bool) {
        // (integration batch I15: `TheScriptEngine->setChooseVictimAlwaysUsesNormal(b)`, the engine's own flag, which the AI reads)
        self.script_engine.choose_victim_always_uses_normal = b;
    }
    fn sa_obj_has_railroad(&self, o: ObjH) -> bool {
        // (integration batch I22: was `Default::default()`)
        self.find_module(o, b"RailroadBehavior").is_some()
    }
    fn sa_railroad_set_held(&mut self, o: ObjH, held: bool) {
        if let Some(m) = self.find_module(o, b"RailroadBehavior") {
            self.module_call(o, m, crate::logic::world::module_call::ModuleCall::RailroadSetHeld(held));
        }
    }

}

#[cfg(test)]
mod tests {
    //! The actions themselves are checked against the compiled ScriptActions.cpp by `tests/differential_script_actions.rs`
    //! with a recording host; this checks one executed on the world, with the engine lent back for its player lookup.
    use super::*;
    use crate::logic::scripts::{action_type as at, Parameter};
    use crate::logic::world::player_host_world::tests::world;

    fn give_money(side: &[u8], amount: i32) -> ScriptAction {
        let mut a = ScriptAction::new_for_read();
        a.action_type = at::PLAYER_GIVE_MONEY;
        a.num_parms = 2;
        let mut p = Parameter::new(0, 0);
        p.string = side.to_vec();
        a.parms[0] = Some(p);
        a.parms[1] = Some(Parameter::new(0, amount));
        a
    }

    /// `PLAYER_GIVE_MONEY` (`Player1`, 300) twice, then -200: player 1 holds 300, 600, 400. The player is found by the
    /// engine's `getPlayerFromAsciiString` while the engine executing the action is lent back to the world, and the
    /// engine is out of the world again afterwards. A side no player has gives nothing (upstream returns quietly).
    #[test]
    fn an_action_is_carried_out_on_the_world() {
        let (mut w, _) = world();
        let mut engine = std::mem::take(&mut w.script_engine);
        engine.counters[0].value = 42; // marks this engine
        let money = |w: &World| w.players.get_nth(1).unwrap().money.count_money();
        w.execute_action_with(&mut engine, &give_money(b"Player1", 300)).unwrap();
        assert_eq!(money(&w), 300);
        w.execute_action_with(&mut engine, &give_money(b"Player1", 300)).unwrap();
        assert_eq!(money(&w), 600);
        w.execute_action_with(&mut engine, &give_money(b"Player1", -200)).unwrap();
        assert_eq!(money(&w), 400);
        w.execute_action_with(&mut engine, &give_money(b"NoSuchPlayer", 300)).unwrap();
        assert_eq!(money(&w), 400);
        assert_eq!((engine.counters[0].value, w.script_engine.counters[0].value), (42, 0));
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }

    /// An action of `action_type` with string, int and real parameters, in that order.
    fn action(action_type: i32, strings: &[&[u8]], ints: &[i32], reals: &[f32]) -> ScriptAction {
        let mut a = ScriptAction::new_for_read();
        a.action_type = action_type;
        let mut i = 0;
        for s in strings {
            let mut p = Parameter::new(0, 0);
            p.string = s.to_vec();
            a.parms[i] = Some(p);
            i += 1;
        }
        for n in ints {
            a.parms[i] = Some(Parameter::new(0, *n));
            i += 1;
        }
        for r in reals {
            let mut p = Parameter::new(0, 0);
            p.real = *r;
            a.parms[i] = Some(p);
            i += 1;
        }
        a.num_parms = i as i32;
        a
    }

    /// GitHub #51: the scripts' audio actions are client events, and change nothing the logic CRCs or saves.
    ///
    /// *Predictions, written before the first run:* `PLAY_SOUND_EFFECT Snd` sends one `AddEvent` (logical, the local
    /// player's index, no position, no object); `MUSIC_SET_TRACK Track_A 1 0` sends `RemoveEventHandle(5)`
    /// (`AHSV_StopTheMusicFade`) then a non-logical `AddEvent` without fade, and sets the engine's (saved)
    /// `currentTrackName` as before; `SOUND_SET_VOLUME 50` sends `SetVolume { 0.5, Sound | Sound3D = 6 }`;
    /// `SUSPEND_BACKGROUND_SOUNDS` sends `Pause { true, 2 }`; `SOUND_DISABLE_TYPE Snd` sends `SetEventEnabled { Snd,
    /// false }`. Nothing else: the logic's random state, its made-up audio handle (`cuts.last_audio_handle`, 0) and its
    /// CRC are as before the actions.
    #[test]
    fn the_scripts_audio_actions_are_client_events_that_change_no_logic_state() {
        use crate::logic::client_event::{ClientEvent, ScriptAudioEvent};
        use crate::logic::world::game_logic_update::CRC_RECALC;
        let (mut w, _) = world();
        let mut engine = std::mem::take(&mut w.script_engine);
        w.take_client_events();
        w.players.set_local_player(Some(1));
        let local = w.players.local_index().expect("a local player");
        let random = w.random.clone();
        let handle = w.cuts.last_audio_handle;
        let crc = w.logic_crc(CRC_RECALC, None).unwrap();
        for a in [
            action(at::PLAY_SOUND_EFFECT, &[b"Snd"], &[], &[]),
            action(at::SOUND_SET_VOLUME, &[], &[], &[50.0]),
            action(at::SUSPEND_BACKGROUND_SOUNDS, &[], &[], &[]),
            action(at::SOUND_DISABLE_TYPE, &[b"Snd"], &[], &[]),
        ] {
            w.execute_action_with(&mut engine, &a).unwrap();
        }
        assert_eq!(w.logic_crc(CRC_RECALC, None).unwrap(), crc, "the CRC");
        w.execute_action_with(&mut engine, &action(at::MUSIC_SET_TRACK, &[b"Track_A"], &[1, 0], &[])).unwrap();
        let ev = w.take_client_events();
        let ev: Vec<_> = ev.into_iter().filter_map(|e| if let ClientEvent::ScriptAudio(s) = e { Some(s) } else { None }).collect();
        let add = |name: &[u8], logical| {
            ScriptAudioEvent::AddEvent(AudioEvent { name: name.to_vec(), position: None, logical, player_index: local, object: 0, uninterruptable: false, should_fade: false }, Default::default())
        };
        assert_eq!(
            ev,
            [
                add(b"Snd", true),
                ScriptAudioEvent::SetVolume { volume: 0.5, affect: 6 },
                ScriptAudioEvent::Pause { pause: true, affect: 2 },
                ScriptAudioEvent::SetEventEnabled { name: b"Snd".to_vec(), enable: false },
                ScriptAudioEvent::RemoveEventHandle(5),
                add(b"Track_A", false),
            ]
        );
        assert_eq!(engine.current_track_name, b"Track_A", "the engine's saved track, as before #51");
        assert_eq!(w.random, random, "no logic random draw");
        assert_eq!(w.cuts.last_audio_handle, handle, "no made-up handle taken");
    }

    /// GitHub #50: a script's logical sound makes upstream's `GameLogicRandomValue` draws in the logic, and the client
    /// plays the file the logic picked.
    ///
    /// *Predictions, written before the first run:*
    /// - `PLAY_SOUND_EFFECT Rnd` (five `Sounds`, `RANDOM`, three `Attack`, no `Decay`) advances the World's logic stream
    ///   by exactly two draws, `GameLogicRandomValue(0, 4)` then `(0, 2)` (a copy of the generator making those two
    ///   draws ends equal), and the event carries those two values and no decay;
    /// - the client, given the event, plays `Data\Audio\Sounds\r<sound>.wav` (no "not the last one" bump: the script's
    ///   event is a fresh local, index -1) after the attack `a<attack>.wav`, and its own copy of the logic stream is
    ///   untouched;
    /// - `Solo` (one file, no `RANDOM`) draws nothing; `SoloRandom` (one file, `RANDOM`) draws **once**: upstream's
    ///   `GetGameLogicRandomValue(0, 0)` advances the state (`delta` is 1, not 0), so "a single-file sound draws
    ///   nothing" holds only without `RANDOM`;
    /// - `Missing` (no description), `MUSIC_SET_TRACK` (not logical) and `PLAY_SOUND_EFFECT` of a dialog event without
    ///   `Attack`/`Decay` draw nothing.
    ///
    /// *Results (2026-11-06):* all held: two draws, sound 2 and attack 1 from the test world's stream; the client
    /// started `a1.wav` and holds `r2.wav` (playing index 2) as the sound to follow; its logic state untouched;
    /// `SoloRandom` drew once (0), the others none. The first run failed in the test's own host, not the logic: its file
    /// system said every file exists, so the localized `Sounds\English\\a1.wav` was preferred (upstream's
    /// `adjustForLocalization`); the host now has no files.
    #[test]
    fn a_scripts_logical_sound_draws_upstreams_logic_values_and_the_client_plays_them() {
        use crate::client::audio_info::AudioTables;
        use crate::client::client_audio::{ClientAudio, WorldAudioHost};
        use crate::client::game_audio::tests::LogDevice;
        use crate::client::game_audio::AudioManager;
        use crate::logic::client_event::{ClientEvent, ScriptAudioEvent};
        use crate::logic::logical_audio::LogicalAudioPicks;
        const INI: &[u8] = b"AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n MusicFolder = Tracks\n StreamingFolder = Speech\n SoundsExtension = wav\n SampleCount2D = 4\n SampleCount3D = 8\n MinSampleVolume = 1%\n DefaultSoundVolume = 100%\n Default3DSoundVolume = 100%\n DefaultSpeechVolume = 100%\n DefaultMusicVolume = 100%\nEnd\n\
AudioEvent Rnd\n Sounds = r0 r1 r2 r3 r4\n Control = random\n Attack = a0 a1 a2\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent Solo\n Sounds = solo\n Type = ui everyone\nEnd\n\
AudioEvent SoloRandom\n Sounds = solo\n Control = random\n Type = ui everyone\nEnd\n\
DialogEvent Talk\n Filename = talk.wav\nEnd\n";
        let (mut w, _) = world();
        let mut engine = std::mem::take(&mut w.script_engine);
        w.take_client_events();
        w.players.set_local_player(Some(1));
        let mut t = AudioTables::new();
        t.load_text(b"a.ini", INI).expect("the audio INI");
        std::sync::Arc::make_mut(&mut w.statics).audio = t.clone();
        let play = |w: &mut World, engine: &mut ScriptEngine, name: &[u8]| {
            w.execute_action_with(engine, &action(at::PLAY_SOUND_EFFECT, &[name], &[], &[])).unwrap();
            let ev = w.take_client_events();
            let adds: Vec<_> = ev
                .into_iter()
                .filter_map(|e| if let ClientEvent::ScriptAudio(s @ ScriptAudioEvent::AddEvent(..)) = e { Some(s) } else { None })
                .collect();
            assert_eq!(adds.len(), 1);
            adds.into_iter().next().unwrap()
        };
        // Rnd: two draws
        let mut predicted = w.random.clone();
        let sound = predicted.game_logic_random_value(0, 4);
        let attack = predicted.game_logic_random_value(0, 2);
        let ev = play(&mut w, &mut engine, b"Rnd");
        assert_eq!(w.random, predicted, "exactly two logic draws");
        let ScriptAudioEvent::AddEvent(_, picks) = &ev else { unreachable!() };
        assert_eq!(*picks, LogicalAudioPicks { sound: Some(sound), attack: Some(attack), decay: None });
        // the client plays the logic's picks
        let mut audio = ClientAudio::new(AudioManager::new(t));
        let client_logic = audio.manager.random.logic;
        let mut dev = LogDevice::default();
        let files = |_: &[u8]| false;
        let host = WorldAudioHost { world: &w, local_player_index: None, files: &files, language: b"English" };
        audio.apply_event(&host, &mut dev, &ClientEvent::ScriptAudio(ev));
        audio.manager.update(&host, &mut dev, &Default::default());
        let started: Vec<String> = dev.started.iter().map(|s| String::from_utf8_lossy(&s.1).into_owned()).collect();
        assert_eq!(started, [format!("Data\\Audio\\Sounds\\a{attack}.wav")], "the attack portion first");
        let req = audio.manager.playing_sounds.first().map(|p| (String::from_utf8_lossy(&p.event.filename_to_load).into_owned(), p.event.playing_audio_index));
        assert_eq!(req, Some((format!("Data\\Audio\\Sounds\\r{sound}.wav"), sound)), "the logic's sound, then");
        assert_eq!(audio.manager.random.logic, client_logic, "the client draws nothing from a logic stream");
        // Solo: none; SoloRandom: one, the value 0
        let before = w.random.clone();
        let ev = play(&mut w, &mut engine, b"Solo");
        assert_eq!((w.random == before, ev), (true, ScriptAudioEvent::AddEvent(AudioEvent { name: b"Solo".to_vec(), position: None, logical: true, player_index: w.players.local_index().unwrap(), object: 0, uninterruptable: false, should_fade: false }, LogicalAudioPicks::default())));
        let mut predicted = w.random.clone();
        assert_eq!(predicted.game_logic_random_value(0, 0), 0);
        let ev = play(&mut w, &mut engine, b"SoloRandom");
        assert_eq!(w.random, predicted, "a single RANDOM file still draws once");
        assert!(matches!(ev, ScriptAudioEvent::AddEvent(_, LogicalAudioPicks { sound: Some(0), attack: None, decay: None })));
        // no description, a dialog event without attack or decay, a music change: no draw
        let before = w.random.clone();
        play(&mut w, &mut engine, b"Missing");
        play(&mut w, &mut engine, b"Talk");
        w.execute_action_with(&mut engine, &action(at::MUSIC_SET_TRACK, &[b"Track_A"], &[1, 0], &[])).unwrap();
        assert_eq!(w.random, before);
    }
}
