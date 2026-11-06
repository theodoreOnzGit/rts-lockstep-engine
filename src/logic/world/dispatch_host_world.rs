// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogicDispatch.cpp (the calls `logicMessageDispatcher`
//   makes out of itself)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Integration batch I7b: the world as `DispatchHost`, so `GameLogic::logicMessageDispatcher`
//! ([`logic_message_dispatcher`]) runs each command of the command list on the world. The command list is now real
//! messages: [`GameLoopCuts::messages`](super::game_logic_host_world::GameLoopCuts) (`TheCommandList`'s arena) and
//! [`GameLoopCuts::commands`](super::game_logic_host_world::GameLoopCuts), in order; `GameLogic::update` dispatches
//! them (batch I4's cut point) and resets the list. [`World::push_command`] appends one.
//!
//! - **The world:** the players (the list, the local player, relationships, `killPlayer`, `transferAssetsFromThat`,
//!   `attemptToPurchaseScience`, the retaliation mode, the hotkey teams, the selection), the objects (position,
//!   controlling player, status, `kill`, `destroyObject`, selection through `GameLogic::selectObject`), their modules
//!   (the AI's `executeWaypointQueue` and `aiExit`, `releaseWeaponLock`, the exit interface's rally point, the
//!   production queue), the thing factory and upgrade center, the build assistant (batch I6), the terrain (clamping a
//!   position), and the victory conditions (batch I5).
//! - **Cut points** ([`CutCall::Host`], answers in tables): the AI's groups (their members in `cuts.player_host.groups`;
//!   a group command is logged and answers false, batch I9), the pathfinder's quick path check, `clearGameData`
//!   (~~a new game, `prepareNewGame`, `startNewGame`~~: the world's since batch I7c), the multiplayer settings' beacon limit, the
//!   recorder, the network, the stats collector, the in-game UI and the rest of the client ([`ClientCall`]).
//!
//! `p1w13-sonnet`, integration batch I7b, 2026-11-01.

use crate::common::base_type::Coord3D;
use crate::common::game_common::Relationship;
use crate::common::ini::{IniError, Undefined};
use crate::common::message_stream::{GameMessage, MessageType};
use crate::common::name_key_generator::NameKeyType;
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_RECONSTRUCTING, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::science::ScienceType;
use crate::logic::world::game_logic::World;
use crate::logic::world::game_logic_dispatch::{ClientCall, DispatchHost, GroupCall, StatusAsked};
use crate::logic::world::game_logic_update::{deselect_object, select_object};
use crate::logic::world::ids::{ObjectId, INVALID_ID};
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::player_objects::{player_calc_cost_to_build, player_kill_player, player_money_deposit, player_transfer_assets_from_that};
use crate::logic::world::player_ops::{player_attempt_to_purchase_science, player_count_objects_by_thing_template, player_get_relationship, player_is_local_player};
use crate::logic::world::player_squads::{
    player_get_current_selection_as_ai_group, player_process_add_team_game_message, player_process_create_team_game_message,
    player_process_select_team_game_message, player_set_currently_selected_ai_group,
};
use crate::logic::world::team_ops::TemplateRef;

type R<T> = Result<T, IniError>;

/// `CMD_FROM_PLAYER`.
const CMD_FROM_PLAYER: i32 = 0;

impl World {
    /// Appends a command to `TheCommandList` (as the network or the message stream does): its type, its player and its
    /// arguments (`f` appends them).
    pub fn push_command(&mut self, ty: MessageType, player: i32, f: impl FnOnce(&mut GameMessage)) {
        // (batch I55: onto the arena's `TheCommandList`, which the recorder reads)
        let gl = &mut self.cuts.game_loop;
        let id = gl.messages.new_message(ty, player);
        if let Some(m) = gl.messages.get_mut(id) {
            m.set_player_index(player);
            f(m);
        }
        let _ = gl.messages.append_message(crate::common::message_stream::Messages::COMMAND_LIST, id);
    }

    /// Logs a call of the dispatcher into a unit the world does not hold.
    fn dispatch_cut(&mut self, call: &'static str, args: &[i64]) {
        self.cut(CutCall::Host { call, args: args.to_vec(), text: Vec::new() });
    }

    /// The player's mask (`getPlayerMask()`: `1 << index`).
    fn dispatch_player_mask(p: i32) -> u16 {
        1u16.checked_shl(p as u32).unwrap_or(0)
    }
}

impl DispatchHost for World {
    fn d_is_in_game(&self) -> bool {
        // `isInGame()`: `getGameMode() != GAME_NONE`
        self.cuts.game_loop.game_mode != crate::common::recorder::game_mode::NONE
    }
    fn d_player_exists(&self, i: i32) -> bool {
        self.players.get_nth(i).is_some()
    }
    fn d_player_count(&self) -> i32 {
        self.players.player_count()
    }
    fn d_local_player(&self) -> Option<i32> {
        (0..16).find(|&i| player_is_local_player(self, i))
    }
    fn d_find_object(&self, id: u32) -> bool {
        self.find_object(id as ObjectId).is_some()
    }
    fn d_object_position(&self, obj: u32) -> Coord3D {
        self.find_object(obj as ObjectId).map(|o| o.get_position()).unwrap_or_default()
    }
    fn d_controlling_player(&self, obj: u32) -> Option<i32> {
        self.get_controlling_player(obj as ObjectId)
    }
    fn d_create_group(&mut self) -> u32 {
        crate::logic::world::game_logic_update::GameLogicHost::u_create_group(self)
    }
    fn d_destroy_group(&mut self, g: u32) {
        crate::logic::world::game_logic_update::GameLogicHost::u_destroy_group(self, g);
    }
    fn d_selection_into_group(&mut self, player: i32, g: u32) {
        let _ = player_get_current_selection_as_ai_group(self, player, Some(g));
    }
    fn d_group_is_empty(&self, g: u32) -> bool {
        if self.cuts.live_ai {
            return self.ai_group_members(g).is_empty();
        }
        self.cuts.player_host.groups.get(&g).is_none_or(|m| m.is_empty())
    }
    fn d_group_remove_not_owned(&mut self, g: u32, player: i32) -> bool {
        // `removeAnyObjectsNotOwnedByPlayer`: answers whether the group is now empty (and so deleted)
        if self.cuts.live_ai {
            return self.with_group(g, |gr, w| gr.remove_any_objects_not_owned_by_player(w, Some(player as u32))).unwrap_or(true);
        }
        let members = self.cuts.player_host.groups.get(&g).cloned().unwrap_or_default();
        let kept: Vec<ObjectId> = members.into_iter().filter(|&o| self.get_controlling_player(o) == Some(player)).collect();
        let empty = kept.is_empty();
        if empty {
            self.cuts.player_host.groups.remove(&g);
        } else {
            self.cuts.player_host.groups.insert(g, kept);
        }
        empty
    }
    fn d_group_ids(&self, g: u32) -> Vec<u32> {
        if self.cuts.live_ai {
            return self.ai_group_members(g).into_iter().map(|o| o as u32).collect();
        }
        self.cuts.player_host.groups.get(&g).map(|m| m.iter().map(|o| *o as u32).collect()).unwrap_or_default()
    }
    fn d_group_add(&mut self, g: u32, obj: u32) {
        crate::logic::world::game_logic_update::GameLogicHost::u_group_add(self, g, obj);
    }
    fn d_group(&mut self, g: u32, call: GroupCall) -> R<bool> {
        // batch I9c-2: with live groups, the group's command; else (the AI's groups not live) the command is logged
        if self.cuts.live_ai {
            return Ok(self.live_group_call(g, call));
        }
        self.cut(CutCall::Host { call: "AIGroup command", args: vec![i64::from(g)], text: format!("{call:?}").into_bytes() });
        Ok(false)
    }
    fn d_ai_execute_waypoint_queue(&mut self, obj: u32) {
        // (integration batch I28: was a cut) the unit's AI's `executeWaypointQueue()`
        self.ai_ask(obj as ObjectId, ModuleCall::AiExecuteWaypointQueue);
    }
    fn d_release_weapon_lock(&mut self, obj: u32, lock: i32) {
        // (integration batch I28: was a cut) `Object::releaseWeaponLock(lock)`
        crate::logic::world::object_weapons::release_weapon_lock(self, obj as ObjectId, lock);
    }
    fn d_ai_exit(&mut self, obj: u32, container: u32) {
        self.ai_ask(obj as ObjectId, ModuleCall::AiExit { container: container as ObjectId, command_source: CMD_FROM_PLAYER });
    }
    fn d_is_locally_controlled(&self, obj: u32) -> bool {
        self.is_locally_controlled(obj as ObjectId)
    }
    fn d_rally_path_exists(&mut self, obj: u32, pos: &Coord3D) -> bool {
        // (integration batch I28: was a cut answering true) `TheAI->pathfinder()->clientSafeQuickDoesPathExist(locomotorSet,
        // obj->getPosition(), &pos)`, the set of the one template `BasicHumanLocomotor` (GameLogicDispatch.cpp: "for now, just use the basic
        // human locomotor"); with `live_pathfinder` and a map
        if self.pf_ready().is_none() {
            self.dispatch_cut("Pathfinder::clientSafeQuickDoesPathExist", &[i64::from(obj), i64::from(pos.x.to_bits()), i64::from(pos.y.to_bits())]);
            return true;
        }
        let key = self.names.name_to_key(b"BasicHumanLocomotor");
        let surfaces = self.statics.locomotor_store.find_locomotor_template(key).map_or(0, |t| t.get_final_override().data.surfaces);
        let Some(from) = self.objects.find(obj as ObjectId).map(|o| o.get_position()) else { return false };
        self.with_pathfinder(|pf, w| pf.client_safe_quick_does_path_exist(w, surfaces as _, &from, pos)).unwrap_or(false)
    }
    fn d_drawable_selected(&self, obj: u32) -> bool {
        self.cuts.selected_drawables.contains(&(obj as ObjectId))
    }
    fn d_set_rally_point(&mut self, obj: u32, pos: &Coord3D) {
        if let Some(e) = self.get_object_exit_interface(obj as ObjectId) {
            self.module_call(obj as ObjectId, e, ModuleCall::SetRallyPoint(*pos));
        }
    }
    fn d_has_production(&self, obj: u32) -> bool {
        crate::logic::world::build_assistant::BuildAssistantHost::obj_has_production(self, obj as ObjectId)
    }
    fn d_cancel_upgrade(&mut self, obj: u32, key: i32) {
        if let Some(p) = self.get_production_update_interface(obj as ObjectId) {
            self.module_call(obj as ObjectId, p, ModuleCall::ProductionCancelUpgrade { upgrade: Some(key as u32) });
        }
    }
    fn d_queue_create_unit(&mut self, obj: u32, template_id: i32, production_id: i32) {
        if let Some(p) = self.get_production_update_interface(obj as ObjectId) {
            let template = u16::try_from(template_id).ok();
            self.module_call(obj as ObjectId, p, ModuleCall::ProductionQueueCreateUnit { template, production_id });
        }
    }
    fn d_cancel_unit_create(&mut self, obj: u32, production_id: i32) {
        if let Some(p) = self.get_production_update_interface(obj as ObjectId) {
            self.module_call(obj as ObjectId, p, ModuleCall::ProductionCancelUnitCreate { production_id });
        }
    }
    fn d_test_status(&self, obj: u32, status: StatusAsked) -> bool {
        let bit = match status {
            StatusAsked::UnderConstruction => OBJECT_STATUS_UNDER_CONSTRUCTION,
            StatusAsked::Reconstructing => OBJECT_STATUS_RECONSTRUCTING,
        };
        self.find_object(obj as ObjectId).is_some_and(|o| o.test_status(bit))
    }
    fn d_refund_build_cost(&mut self, player: i32, obj: u32) {
        let Some(t) = self.find_object(obj as ObjectId).map(|o| TemplateRef::from(o.template().get_template_id())) else { return };
        let cost = player_calc_cost_to_build(self, player, t).unwrap_or(0);
        let _ = player_money_deposit(self, player, cost as u32, true);
    }
    fn d_kill(&mut self, obj: u32) {
        self.kill(obj as ObjectId, crate::logic::damage::DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
    }
    fn d_destroy_object(&mut self, obj: u32) {
        self.destroy_object(obj as ObjectId);
    }
    fn d_select_object(&mut self, obj: u32, create_new_group: bool, player: i32) {
        select_object(Some(obj), create_new_group, World::dispatch_player_mask(player), false, self);
    }
    fn d_deselect_object(&mut self, obj: u32, player: i32) {
        deselect_object(Some(obj), World::dispatch_player_mask(player), false, self);
    }
    fn d_clear_selected_group(&mut self, player: i32) {
        let _ = player_set_currently_selected_ai_group(self, player, None);
    }
    fn d_upgrade_exists(&self, key: i32) -> bool {
        self.statics.upgrade_center.find_upgrade_by_key(NameKeyType(key as u32)).is_some()
    }
    fn d_template_exists(&self, id: i32) -> bool {
        u16::try_from(id).ok().and_then(|id| self.statics.thing_factory.find_by_template_id(id)).is_some()
    }
    fn d_build_object_now(&mut self, constructor: u32, template_id: i32, loc: &Coord3D, angle: f32) {
        let Some(owner) = self.get_controlling_player(constructor as ObjectId) else {
            // the constructor's controlling player is dereferenced upstream
            self.cuts.team_live_errors.push("BuildAssistant::buildObjectNow: no controlling player".to_string());
            return;
        };
        let c = Some(constructor as ObjectId).filter(|c| *c != INVALID_ID);
        if let Err(e) = crate::logic::world::build_assistant::build_assistant_build_object_now(self, c, template_id as TemplateRef, *loc, angle, owner) {
            self.cuts.team_live_errors.push(format!("BuildAssistant::buildObjectNow: {e:?}"));
        }
    }
    fn d_build_object_line_now(&mut self, constructor: u32, template_id: i32, start: &Coord3D, end: &Coord3D, angle: f32) {
        let Some(owner) = self.get_controlling_player(constructor as ObjectId) else {
            self.cuts.team_live_errors.push("BuildAssistant::buildObjectLineNow: no controlling player".to_string());
            return;
        };
        let c = Some(constructor as ObjectId).filter(|c| *c != INVALID_ID);
        let mut ba = std::mem::take(&mut self.build_assistant);
        let r = crate::logic::world::build_assistant::build_assistant_build_object_line_now(self, &mut ba, c, template_id as TemplateRef, *start, *end, angle, owner);
        self.build_assistant = ba;
        if let Err(e) = r {
            self.cuts.team_live_errors.push(format!("BuildAssistant::buildObjectLineNow: {e:?}"));
        }
    }
    fn d_clamp_to_terrain(&mut self, pos: &Coord3D) -> Coord3D {
        let Ok(extent) = self.terrain.extent() else { return *pos };
        if extent.is_in_region_no_z(pos) {
            return *pos;
        }
        self.terrain.find_closest_edge_point(pos).unwrap_or(*pos)
    }
    fn d_beacon_template(&self, player: i32) -> R<Option<i32>> {
        let t = self.players.get_nth(player).and_then(|p| p.template).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        let pt = self.statics.player_template_store.nth_player_template(t as i32).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        Ok(World::thing_factory_find_template(self, pt.beacon_template()).map(i32::from))
    }
    fn d_player_defeated(&self, player: i32) -> bool {
        self.victory.has_single_player_been_defeated(self, Some(player))
    }
    fn d_count_objects_of_template(&self, player: i32, template_id: i32) -> i32 {
        let mut count = [0];
        let _ = player_count_objects_by_thing_template(self, player, &[Some(template_id as TemplateRef)], false, &mut count, true);
        count[0]
    }
    fn d_max_beacons(&self) -> i32 {
        // (batch I56: the statics' settings; was `cuts.max_beacons_per_player`, the constructor's 3)
        self.statics.multiplayer_settings.get_max_beacons_per_player()
    }
    fn d_new_beacon(&mut self, template_id: i32, player: i32, pos: &Coord3D) -> R<u32> {
        let team = self.players.get_nth(player).and_then(|p| p.default_team);
        let t = u16::try_from(template_id).map_err(|_| IniError::undefined(Undefined::NullObject))?;
        let id = World::thing_factory_new_object(self, t, team, ObjectStatusMaskType::default()).map_err(|_| IniError::undefined(Undefined::NullObject))?;
        self.set_position(id, pos);
        self.set_producer(id, INVALID_ID);
        Ok(id as u32)
    }
    fn d_relationship(&self, a: i32, b: i32) -> i32 {
        let team = self.players.get_nth(b).and_then(|p| p.default_team);
        player_get_relationship(self, a, team).unwrap_or(Relationship::Neutral) as i32
    }
    fn d_local_is_observer(&self) -> R<bool> {
        let p = self.d_local_player().ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        Ok(crate::logic::world::player_ops::player_is_player_observer(self, p).unwrap_or(false))
    }
    fn d_is_controllers_beacon(&self, obj: u32) -> R<bool> {
        let o = self.find_object(obj as ObjectId).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        let p = self.get_controlling_player(obj as ObjectId).ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        let beacon = self.d_beacon_template(p)?.ok_or_else(|| IniError::undefined(Undefined::NullObject))?;
        Ok(self.thing_template_is_equivalent_to(beacon as u16, o.template().get_template_id()))
    }
    fn d_has_drawable(&self, obj: u32) -> bool {
        self.has_drawable(obj as ObjectId)
    }
    fn d_set_retaliation(&mut self, player: i32, enable: bool) {
        if let Some(p) = self.players.get_nth_mut(player) {
            p.logical_retaliation_mode_enabled = enable;
        }
    }
    fn d_transfer_assets(&mut self, to: i32, from: i32) {
        let _ = player_transfer_assets_from_that(self, to, from);
    }
    fn d_kill_player(&mut self, player: i32) {
        let _ = player_kill_player(self, player);
    }
    fn d_team_message(&mut self, kind: i32, player: i32, n: i32, msg: &GameMessage) {
        // the message's object ids, for the squad code's argument reads (`cuts.player_host.messages`)
        let ids: Vec<ObjectId> = (0..i32::from(msg.get_argument_count())).map(|i| msg.get_argument(i).object_id() as ObjectId).collect();
        let r = self.cuts.player_host.messages.keys().next_back().map_or(1, |k| k + 1);
        self.cuts.player_host.messages.insert(r, ids);
        let _ = match kind {
            0 => player_process_create_team_game_message(self, player, n, r),
            1 => player_process_select_team_game_message(self, player, n, r),
            _ => player_process_add_team_game_message(self, player, n, r),
        };
        self.cuts.player_host.messages.remove(&r);
    }
    fn d_purchase_science(&mut self, player: i32, science: i32) {
        let _ = player_attempt_to_purchase_science(self, player, ScienceType(science));
    }
    fn d_prepare_new_game(&mut self, mode: i32, diff: i32, rank_points: i32) {
        // batch I7c: `GameLogic::prepareNewGame` (GameLogicDispatch.cpp); the background window and the shell are the
        // client's
        self.dispatch_cut("GameLogic::prepareNewGame (client: background, shell)", &[i64::from(mode), i64::from(diff), i64::from(rank_points)]);
        // `TheScriptEngine->setGlobalDifficulty(diff)` (integration batch I49: it set only the World's copy)
        self.set_global_difficulty(diff);
        self.cuts.game_loop.game_mode = mode;
        if !self.cuts.new_game.pending_file.is_empty() {
            self.cuts.new_game.map_name = std::mem::take(&mut self.cuts.new_game.pending_file);
        }
        // `m_rankPointsToAddAtGameStart`: `GameLogic`'s member, held by the frame now; written when the frame ends,
        // before the next frame's `startNewGame` reads it (see `World::logic_update`)
        self.cuts.pending_rank_points = Some(rank_points);
        self.set_start_new_game(false);
    }
    fn d_start_new_game(&mut self) {
        // batch I7c: `startNewGame(FALSE)` right after `prepareNewGame` (no game pending): its first call only marks the
        // game pending (and makes a single-player game's load screen), touching none of `GameLogic`'s members
        debug_assert!(!self.start_new_game_pending());
        let mut untouched = crate::logic::world::game_logic_update::GameLogicState::new();
        if let Err(e) = crate::logic::world::game_logic_new_game::start_new_game(&mut untouched, false, self) {
            self.cuts.team_live_errors.push(format!("GameLogic::startNewGame: {e:?}"));
        }
    }
    fn d_clear_game_data(&mut self) {
        self.dispatch_cut("GameLogic::clearGameData", &[]);
    }
    fn d_global_fps_limit(&self) -> i32 {
        self.statics.global_data.the_global_data().map_or(30, |g| g.frames_per_second_limit)
    }
    fn d_has_stats_collector(&self) -> bool {
        self.cuts.game_loop.has_stats_collector
    }
    fn d_has_in_game_ui(&self) -> bool {
        false
    }
    fn d_recorder_playback(&self) -> bool {
        // (batch I55: the live recorder when made)
        match &self.cuts.game_loop.recorder {
            Some(r) => r.mode == crate::common::recorder::RecorderMode::Playback,
            None => self.cuts.game_loop.has_recorder && self.cuts.game_loop.recorder_playback,
        }
    }
    fn d_has_recorder(&self) -> bool {
        self.cuts.game_loop.recorder.is_some() || self.cuts.game_loop.has_recorder
    }
    fn d_recorder_handle_crc(&mut self, crc: u32, player: i32, from_playback: bool) -> R<()> {
        // (batch I55: the live recorder's `handleCRCMessage`; a logged call without one)
        match self.with_recorder(|r, w| r.handle_crc_message(crc, player, from_playback, w)) {
            Some(r) => r,
            None => {
                self.dispatch_cut("Recorder::handleCRCMessage", &[i64::from(crc), i64::from(player), i64::from(from_playback)]);
                Ok(())
            }
        }
    }
    fn d_replay_camera_follows(&self, _player: i32) -> bool {
        false
    }
    fn d_camera_movement_finished(&self) -> bool {
        self.cuts.game_loop.view_camera_movement_finished
    }
    fn d_mouse_moved_recently(&self) -> bool {
        false
    }
    fn d_has_network(&self) -> bool {
        self.cuts.game_loop.has_network
    }
    fn d_network_slot_is(&self, slot: i32, player: i32) -> bool {
        self.cuts.network_slot_players.get(&slot) == Some(&player)
    }
    fn d_network_connected(&self, slot: i32) -> bool {
        crate::logic::world::game_logic_update::GameLogicHost::u_network_player_connected(self, slot)
    }
    fn d_is_local_player(&self, player: i32) -> bool {
        player_is_local_player(self, player)
    }
    /// The cut log's record, and (GitHub #56) the call itself as a client event, for the client to make.
    fn d_client(&mut self, call: ClientCall) {
        self.cut(CutCall::Host { call: "client", args: Vec::new(), text: format!("{call:?}").into_bytes() });
        self.events.push(crate::logic::client_event::ClientEvent::Dispatch(call));
    }
}

// --- p1w13-sonnet --- (integration batch I49)
impl World {
    /// `TheScriptEngine->setGlobalDifficulty(diff)`: the engine's `m_gameDifficulty` (as upstream, the int as given) and the
    /// World's copy that the players and AI read while the engine is checked out (see `PlayerHostCuts::global_difficulty`).
    pub fn set_global_difficulty(&mut self, diff: i32) {
        self.script_engine.set_global_difficulty(diff);
        self.cuts.player_host.global_difficulty = crate::logic::world::player_init::difficulty_from_int(diff);
    }
}
// --- end p1w13-sonnet ---

#[cfg(test)]
mod tests {
    //! The dispatcher itself is checked against the compiled GameLogicDispatch.cpp by `tests/differential_dispatch.rs`
    //! with a recording host; this checks commands run on the world through the game logic's frame.
    use super::*;
    use crate::common::message_types::{MSG_ENABLE_RETALIATION_MODE, MSG_SELF_DESTRUCT};
    use crate::common::recorder::game_mode;
    use crate::logic::world::player_host_world::tests::{make, world};

    /// In a single-player game, `MSG_ENABLE_RETALIATION_MODE` (player 1, on) and then `MSG_SELF_DESTRUCT` (player 1, not
    /// to an ally) from player 1 are dispatched in one frame: player 1's retaliation mode is on, its tank is killed, and
    /// the command list is empty afterwards.
    #[test]
    fn commands_run_on_the_world_in_the_frame() {
        let (mut w, team) = world();
        w.cuts.live_teams = true;
        w.cuts.game_loop.game_mode = game_mode::SINGLE_PLAYER;
        let tank = make(&mut w, b"Tank", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
        w.push_command(MSG_ENABLE_RETALIATION_MODE, 1, |m| {
            m.append_integer_argument(1);
            m.append_boolean_argument(true);
        });
        w.push_command(MSG_SELF_DESTRUCT, 1, |m| m.append_boolean_argument(false));
        assert!(!w.players.get_nth(1).unwrap().logical_retaliation_mode_enabled);
        w.logic_update().unwrap();
        assert!(w.players.get_nth(1).unwrap().logical_retaliation_mode_enabled);
        assert!(w.find_object(tank).is_none_or(|o| o.is_effectively_dead()));
        assert!(w.cuts.game_loop.messages.first_message(crate::common::message_stream::Messages::COMMAND_LIST).is_none());
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
