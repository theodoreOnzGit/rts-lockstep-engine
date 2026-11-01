// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AI.cpp (`createGroup`, `destroyGroup`, `findGroup`, `getNextGroupID`,
//     `getNextFormationID`, 447-503; the groups in `AI::crc`, 1017-1025)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGroup.cpp (`~AIGroup`, 91-112)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AI.h (`AICommandInterface`'s inline builders, for the groups' commands)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp (`leaveGroup`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! Integration batch I9c-2: `TheAI`'s groups in the World ([`AiGroups`]), live with `cuts.live_ai` (without it the groups
//! stay the cut table `cuts.player_host.groups`, as before).
//!
//! **The list.** `m_groupList` in creation order; `createGroup` appends a group with the next id, `destroyGroup` erases it and
//! deletes it (`~AIGroup`: each member `leaveGroup`s). A group runs checked out of the list ([`World::with_group`]); a member
//! that leaves a running group (`AIGroup::remove`'s own `obj->leaveGroup()`) finds it out of the list and does nothing, which
//! is what upstream's `remove` answers there (the member is gone already). A group destroyed while it runs is not put back.
//!
//! **The commands** a group gives its members' AIs ([`GroupAiCommand`]) are `AICommandParms` built as `AICommandInterface`'s
//! inline builders build them, then the member's `aiDoCommand` ([`World::group_ai_command`]).
//!
//! `p1w13-sonnet`, integration batch I9c-2, 2026-11-01.

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::xfer::{Xfer, XferResult};
use crate::logic::ai::ai_commands::*;
use crate::logic::ai::ai_group::{AiGroup, GroupAiCommand, NO_FORMATION_ID};
use crate::logic::ai::ai_update_types::CommandSourceType;
use crate::logic::world::game_logic::World;
use crate::common::base_type::{Coord2D, Coord3D, Region3D};
use crate::common::game_common::PlayerType;
use crate::common::game_common::AbleToAttackType;
use crate::common::kind_of::KindOfType;
use crate::common::object_status_types::ObjectStatusTypes;
use crate::logic::ai::ai_group::{AiGroupHost, GroupButtonCall};
use crate::logic::ai::ai_update_types::{AttitudeType, CanAttackResultValue};
use crate::logic::module::body_module::BodyDamageType;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object_cuts::CutCall;

/// `TheAI`'s groups: `m_groupList` in order, `m_nextGroupID`, `m_nextFormationID`, and the ids of the groups running now
/// (checked out of the list), with those destroyed while they ran.
#[derive(Clone, Debug)]
pub struct AiGroups {
    pub list: Vec<AiGroup>,
    pub next_group_id: u32,
    pub next_formation_id: u32,
    pub running: Vec<u32>,
    pub destroyed_while_running: Vec<u32>,
}

impl Default for AiGroups {
    /// `AI::reset()`'s: no groups, the next group id 0, the next formation id one past `NO_FORMATION_ID`.
    fn default() -> Self {
        AiGroups { list: Vec::new(), next_group_id: 0, next_formation_id: NO_FORMATION_ID + 1, running: Vec::new(), destroyed_while_running: Vec::new() }
    }
}

impl World {
    /// `TheAI->createGroup()`: a new group with the next id, at the end of the list; its id.
    pub fn ai_create_group(&mut self) -> u32 {
        let g = AiGroup::new(self);
        let id = g.get_id();
        self.ai_groups.list.push(g);
        id
    }

    /// `TheAI->findGroup(id) != NULL` (a running group counts).
    pub fn ai_group_exists(&self, id: u32) -> bool {
        self.ai_groups.list.iter().any(|g| g.get_id() == id) || self.ai_groups.running.contains(&id)
    }

    /// `TheAI->destroyGroup(group)`: out of the list, then deleted (`~AIGroup`: each member leaves it). Nothing for a group not in
    /// the list; a running group is marked, and not put back when it returns.
    pub fn ai_destroy_group_live(&mut self, id: u32) {
        let Some(i) = self.ai_groups.list.iter().position(|g| g.get_id() == id) else {
            if self.ai_groups.running.contains(&id) && !self.ai_groups.destroyed_while_running.contains(&id) {
                self.ai_groups.destroyed_while_running.push(id);
            }
            return;
        };
        let mut g = self.ai_groups.list.remove(i);
        // `~AIGroup`: "disassociate each member from the group" (each `leaveGroup` calls this group's `remove`, now out of the
        // list: its own `destroyGroup` finds nothing)
        while let Some(&o) = g.members.first() {
            if let Some(obj) = self.objects.find_mut(o) {
                obj.group = 0;
            }
            if !g.remove(self, o) && g.members.first() == Some(&o) {
                g.members.remove(0);
            }
        }
    }

    /// Runs `f` on group `id`, checked out of the list; `None` when there is none (or it runs already).
    pub fn with_group<R>(&mut self, id: u32, f: impl FnOnce(&mut AiGroup, &mut World) -> R) -> Option<R> {
        let i = self.ai_groups.list.iter().position(|g| g.get_id() == id)?;
        let mut g = self.ai_groups.list.remove(i);
        self.ai_groups.running.push(id);
        let r = f(&mut g, self);
        self.ai_groups.running.pop();
        if let Some(k) = self.ai_groups.destroyed_while_running.iter().position(|&d| d == id) {
            self.ai_groups.destroyed_while_running.remove(k);
        } else {
            let at = i.min(self.ai_groups.list.len());
            self.ai_groups.list.insert(at, g);
        }
        Some(r)
    }

    /// The members of group `id` (`getAllIDs`); none for no group.
    pub fn ai_group_members(&self, id: u32) -> Vec<ObjectId> {
        self.ai_groups.list.iter().find(|g| g.get_id() == id).map(AiGroup::get_all_ids).unwrap_or_default()
    }

    /// `obj->leaveGroup()` with live groups: `m_group = NULL`, then `group->remove(this)` (nothing when the group is running:
    /// it is the one removing the member).
    pub(crate) fn leave_group_live(&mut self, o: ObjectId, group: u32) {
        self.with_group(group, |g, w| g.remove(w, o));
    }

    /// The groups' part of `AI::crc`: each group's CRC after its marker, in list order.
    pub fn ai_groups_crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        for g in self.ai_groups.list.iter_mut() {
            let mut marker = b"MARKER:AIGroup".to_vec();
            xfer.xfer_ascii_string(&mut marker)?;
            g.crc(xfer)?;
        }
        Ok(())
    }

    /// A group's command to its member `o`'s AI: the `AICommandParms` `AICommandInterface`'s inline builder makes, then
    /// `aiDoCommand` (nothing for a member without an AI). `GoProne`'s damage is a handle the group carries from its caller:
    /// the command goes with no damage (logged).
    pub(crate) fn group_ai_command(&mut self, o: ObjectId, cmd: GroupAiCommand, src: CommandSourceType) {
        let p = |c, f: &dyn Fn(&mut AiCommandParms)| {
            let mut p = AiCommandParms::new(c, src);
            f(&mut p);
            p
        };
        let parms = match cmd {
            GroupAiCommand::Idle => p(AICMD_IDLE, &|_| {}),
            GroupAiCommand::AttackObject { victim, max_shots, forced } => {
                p(if forced { AICMD_FORCE_ATTACK_OBJECT } else { AICMD_ATTACK_OBJECT }, &|q| {
                    q.obj = victim;
                    q.int_value = max_shots;
                })
            }
            GroupAiCommand::AttackTeam { team, max_shots } => p(AICMD_ATTACK_TEAM, &|q| {
                q.team = Some(team);
                q.int_value = max_shots;
            }),
            GroupAiCommand::AttackPosition { pos, max_shots } => p(AICMD_ATTACK_POSITION, &|q| {
                q.pos = pos;
                q.int_value = max_shots;
            }),
            GroupAiCommand::AttackMoveToPosition { pos, max_shots } => p(AICMD_ATTACKMOVE_TO_POSITION, &|q| {
                q.pos = pos;
                q.int_value = max_shots;
            }),
            GroupAiCommand::MoveToPosition { pos } => p(AICMD_MOVE_TO_POSITION, &|q| q.pos = pos),
            GroupAiCommand::Hunt => p(AICMD_HUNT, &|_| {}),
            GroupAiCommand::Repair { obj } => p(AICMD_REPAIR, &|q| q.obj = obj),
            GroupAiCommand::ResumeConstruction { obj } => p(AICMD_RESUME_CONSTRUCTION, &|q| q.obj = obj),
            GroupAiCommand::GetHealed { depot } => p(AICMD_GET_HEALED, &|q| q.obj = depot),
            GroupAiCommand::GetRepaired { depot } => p(AICMD_GET_REPAIRED, &|q| q.obj = depot),
            GroupAiCommand::Enter { obj } => p(AICMD_ENTER, &|q| q.obj = obj),
            GroupAiCommand::Dock { obj } => p(AICMD_DOCK, &|q| q.obj = obj),
            GroupAiCommand::Exit { obj } => p(AICMD_EXIT, &|q| q.obj = obj),
            GroupAiCommand::MoveToAndEvacuate { pos } => p(AICMD_MOVE_TO_POSITION_AND_EVACUATE, &|q| q.pos = pos),
            GroupAiCommand::Evacuate { expose_stealth_units } => p(AICMD_EVACUATE, &|q| q.int_value = i32::from(expose_stealth_units)),
            GroupAiCommand::FollowPathAppend { pos } => p(AICMD_FOLLOW_PATH_APPEND, &|q| q.pos = pos),
            GroupAiCommand::TightenToPosition { pos } => p(AICMD_TIGHTEN_TO_POSITION, &|q| q.pos = pos),
            GroupAiCommand::FollowWaypointPath { way, exact, as_team } => {
                let c = match (as_team, exact) {
                    (false, false) => AICMD_FOLLOW_WAYPOINT_PATH,
                    (false, true) => AICMD_FOLLOW_WAYPOINT_PATH_EXACT,
                    (true, false) => AICMD_FOLLOW_WAYPOINT_PATH_AS_TEAM,
                    (true, true) => AICMD_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT,
                };
                p(c, &|q| q.waypoint = Some(way))
            }
            GroupAiCommand::MoveToAndEvacuateAndExit { pos } => p(AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT, &|q| q.pos = pos),
            GroupAiCommand::ExecuteRailedTransport => p(AICMD_EXECUTE_RAILED_TRANSPORT, &|_| {}),
            GroupAiCommand::GoProne { damage } => {
                self.cut(crate::logic::world::object_cuts::CutCall::Host {
                    call: "AIGroup::groupGoProne",
                    args: vec![i64::from(o), i64::from(damage)],
                    text: Vec::new(),
                });
                p(AICMD_GO_PRONE, &|_| {})
            }
            GroupAiCommand::GuardPosition { pos, mode } => p(AICMD_GUARD_POSITION, &|q| {
                q.pos = pos;
                q.int_value = mode;
            }),
            GroupAiCommand::GuardObject { obj, mode } => p(AICMD_GUARD_OBJECT, &|q| {
                q.obj = obj;
                q.int_value = mode;
            }),
            GroupAiCommand::GuardArea { area, mode } => p(AICMD_GUARD_AREA, &|q| {
                q.polygon = Some(area);
                q.int_value = mode;
            }),
            GroupAiCommand::AttackArea { area } => p(AICMD_ATTACK_AREA, &|q| q.polygon = Some(area)),
            GroupAiCommand::HackInternet => p(AICMD_HACK_INTERNET, &|_| {}),
            GroupAiCommand::CombatDrop { target, pos } => p(AICMD_COMBATDROP, &|q| {
                q.obj = target;
                q.pos = pos;
            }),
        };
        if o != INVALID_ID {
            self.ai_ask(o, ModuleCall::AiDoCommandParms(parms));
        }
    }
}

/// A path's nodes' positions in their optimised order (`getFirstNode`, then `getNextOptimized` to the end), as the group keeps
/// its ground path.
fn path_optimized_positions(p: &crate::logic::ai::path::Path) -> Vec<Coord3D> {
    let mut v = Vec::new();
    let mut n = p.first_node();
    while let Some(i) = n {
        v.push(*p.node(i).position());
        n = p.node(i).next_optimized().0;
    }
    v
}

impl World {
    /// Logs one of the groups' calls into a unit the world does not hold yet (the member's contain and spawn behaviour, special
    /// powers, command buttons, weapon locks, upgrades, the drawable).
    fn grp_cut(&mut self, call: &'static str) {
        self.cut(CutCall::Host { call, args: Vec::new(), text: Vec::new() });
    }
}

/// The World as the AI groups' host (batch I9c-2): generated from the trait's signatures with a table of live bodies; the
/// others are cut points that log and answer a default.
impl AiGroupHost for World {
    fn ai_next_group_id(&mut self) -> u32 {
        let id = self.ai_groups.next_group_id;
        self.ai_groups.next_group_id = id.wrapping_add(1);
        id
    }
    fn ai_next_formation_id(&mut self) -> u32 {
        let id = self.ai_groups.next_formation_id;
        self.ai_groups.next_formation_id = id.wrapping_add(1);
        id
    }
    fn ai_destroy_group(&mut self, group: u32) {
        self.ai_destroy_group_live(group);
    }
    fn frame(&mut self) -> u32 {
        crate::common::state_machine::StateContext::frame(self)
    }
    fn logic_random_value(&mut self, lo: i32, hi: i32) -> i32 {
        self.random.game_logic_random_value(lo, hi)
    }
    fn movement_penalty_damage_state(&mut self) -> BodyDamageType {
        self.statics.global_data.the_global_data().map_or(crate::logic::module::body_module::BODY_REALLYDAMAGED, |g| g.movement_penalty_damage_state)
    }
    fn has_ai(&mut self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.ai.is_some())
    }
    fn is_kind_of(&mut self, o: ObjectId, k: KindOfType) -> bool {
        self.find_object(o).is_some_and(|x| x.is_kind_of(k))
    }
    fn status_test(&mut self, o: ObjectId, s: ObjectStatusTypes) -> bool {
        self.find_object(o).is_some_and(|x| x.test_status(s))
    }
    fn is_held(&mut self, o: ObjectId) -> bool {
        self.find_object(o).is_some_and(|x| x.is_disabled_by_type(crate::common::disabled_types::DISABLED_HELD))
    }
    fn is_effectively_dead(&mut self, o: ObjectId) -> bool {
        self.find_object(o).is_none_or(|x| x.is_effectively_dead())
    }
    fn is_able_to_attack(&mut self, o: ObjectId) -> bool {
        World::is_able_to_attack(self, o)
    }
    fn is_airborne_target(&mut self, o: ObjectId) -> bool {
        self.objects.find(o).is_some_and(|x| x.status.test(crate::common::object_status_types::OBJECT_STATUS_AIRBORNE_TARGET))
    }
    fn position(&mut self, o: ObjectId) -> Coord3D {
        self.find_object(o).map(|x| x.get_position()).unwrap_or_default()
    }
    fn controlling_player(&mut self, o: ObjectId) -> Option<u32> {
        self.get_controlling_player(o).map(|p| p as u32)
    }
    fn enter_group(&mut self, o: ObjectId, group: u32) {
        World::enter_group(self, o, group);
    }
    fn leave_group(&mut self, o: ObjectId) {
        World::leave_group(self, o);
    }
    fn formation_id(&mut self, o: ObjectId) -> u32 {
        self.find_object(o).map_or(NO_FORMATION_ID, |x| x.formation_id as u32)
    }
    fn set_formation_id(&mut self, o: ObjectId, id: u32) {
        if let Some(x) = self.objects.find_mut(o) {
            x.formation_id = id as _;
        }
    }
    fn set_formation_offset(&mut self, o: ObjectId, offset: Coord2D) {
        if let Some(x) = self.objects.find_mut(o) {
            x.formation_offset = offset;
        }
    }
    fn damage_state(&mut self, _o: ObjectId) -> BodyDamageType {
        self.grp_cut("damage_state");
        Default::default()
    }
    fn cur_locomotor_speed(&mut self, o: ObjectId) -> f32 {
        self.with_ai_lent(o, |w| crate::logic::ai::ai_states::with_ai(w, |a, w| a.get_cur_locomotor_speed(w))).unwrap_or(0.0)
    }
    fn able_to_attack_specific_object(&mut self, o: ObjectId, t: AbleToAttackType, victim: ObjectId, src: CommandSourceType) -> CanAttackResultValue {
        match crate::logic::world::object_weapons::get_able_to_attack_specific_object(self, o, t, victim, crate::common::game_common::CommandSourceType::from_i32(src), -1) {
            Ok(r) => r as CanAttackResultValue,
            Err(_) => crate::logic::weapon_host::CanAttackResult::NotPossible as CanAttackResultValue,
        }
    }
    fn able_to_use_weapon_against_position(&mut self, _o: ObjectId, _t: AbleToAttackType, _pos: &Coord3D, _src: CommandSourceType) -> CanAttackResultValue {
        self.grp_cut("able_to_use_weapon_against_position");
        Default::default()
    }
    fn ai_command(&mut self, o: ObjectId, cmd: GroupAiCommand, src: CommandSourceType) {
        self.group_ai_command(o, cmd, src);
    }
    fn ai_set_attitude(&mut self, o: ObjectId, a: AttitudeType) {
        self.ai_ask(o, ModuleCall::AiSetAttitude(a));
    }
    fn ai_is_idle(&mut self, o: ObjectId) -> bool {
        self.ai_ask(o, ModuleCall::AiIsIdle).is_some_and(|r| r.as_bool())
    }
    fn ai_is_busy(&mut self, o: ObjectId) -> bool {
        self.ai_ask(o, ModuleCall::AiIsBusy).is_some_and(|r| r.as_bool())
    }
    fn ai_can_auto_acquire(&mut self, o: ObjectId) -> bool {
        self.ai_peek(o).is_some_and(|a| a.can_auto_acquire())
    }
    fn ai_can_auto_acquire_while_stealthed(&mut self, o: ObjectId) -> bool {
        let a = self.ai_peek(o).cloned();
        a.is_some_and(|a| self.with_ai_self(o, |w| a.can_auto_acquire_while_stealthed(w)))
    }
    fn ai_set_next_mood_check_time(&mut self, o: ObjectId, t: u32) {
        self.with_ai_lent(o, |w| crate::logic::ai::ai_states::with_ai(w, |a, _| a.set_next_mood_check_time(t)));
    }
    fn stealth_delay(&mut self, _o: ObjectId) -> Option<u32> {
        self.grp_cut("stealth_delay");
        Default::default()
    }
    fn has_contain(&mut self, o: ObjectId) -> bool {
        self.get_contain(o).is_some()
    }
    fn contained(&mut self, _o: ObjectId) -> Vec<ObjectId> {
        self.grp_cut("contained");
        Default::default()
    }
    fn contain_passengers_allowed_to_fire(&mut self, _o: ObjectId) -> bool {
        self.grp_cut("contain_passengers_allowed_to_fire");
        Default::default()
    }
    fn contain_order_all_passengers_to_exit(&mut self, _o: ObjectId, _src: CommandSourceType) {
        self.grp_cut("contain_order_all_passengers_to_exit");
    }
    fn has_spawn(&mut self, _o: ObjectId) -> bool {
        self.grp_cut("has_spawn");
        Default::default()
    }
    fn spawn_slaves_have_freedom(&mut self, _o: ObjectId) -> bool {
        self.grp_cut("spawn_slaves_have_freedom");
        Default::default()
    }
    fn spawn_order_slaves_idle(&mut self, _o: ObjectId, _src: CommandSourceType) {
        self.grp_cut("spawn_order_slaves_idle");
    }
    fn spawn_order_slaves_attack_target(&mut self, _o: ObjectId, _victim: ObjectId, _max_shots: i32, _src: CommandSourceType) {
        self.grp_cut("spawn_order_slaves_attack_target");
    }
    fn spawn_order_slaves_attack_position(&mut self, _o: ObjectId, _pos: &Coord3D, _max_shots: i32, _src: CommandSourceType) {
        self.grp_cut("spawn_order_slaves_attack_position");
    }
    fn highest_layer_for_destination(&mut self, pos: &Coord3D) -> i32 {
        self.terrain_get_highest_layer_for_destination(pos)
    }
    fn layer_height(&mut self, x: f32, y: f32, layer: i32) -> f32 {
        self.terrain_get_layer_height(x, y, layer)
    }
    fn special_power_required_science(&mut self, _sp: u32) -> Option<i32> {
        self.grp_cut("special_power_required_science");
        Default::default()
    }
    fn player_has_science(&mut self, _o: ObjectId, _science: i32) -> bool {
        self.grp_cut("player_has_science");
        Default::default()
    }
    fn has_special_power_module(&mut self, _o: ObjectId, _sp: u32) -> bool {
        self.grp_cut("has_special_power_module");
        Default::default()
    }
    fn can_do_special_power(&mut self, _o: ObjectId, _sp: u32, _options: u32) -> bool {
        self.grp_cut("can_do_special_power");
        Default::default()
    }
    fn can_do_special_power_at_location(&mut self, _o: ObjectId, _sp: u32, _pos: &Coord3D, _object_in_way: ObjectId, _options: u32) -> bool {
        self.grp_cut("can_do_special_power_at_location");
        Default::default()
    }
    fn can_do_special_power_at_object(&mut self, _o: ObjectId, _sp: u32, _target: ObjectId, _options: u32) -> bool {
        self.grp_cut("can_do_special_power_at_object");
        Default::default()
    }
    fn do_special_power(&mut self, _o: ObjectId, _sp: u32, _options: u32) {
        self.grp_cut("do_special_power");
    }
    fn do_special_power_at_location(&mut self, _o: ObjectId, _sp: u32, _pos: &Coord3D, _angle: f32, _options: u32) {
        self.grp_cut("do_special_power_at_location");
    }
    fn do_special_power_at_object(&mut self, _o: ObjectId, _sp: u32, _target: ObjectId, _options: u32) {
        self.grp_cut("do_special_power_at_object");
    }
    fn clear_undetected_defector(&mut self, o: ObjectId) {
        self.friend_set_undetected_defector(o, false);
    }
    fn override_special_power_destination(&mut self, _o: ObjectId, _sp_type: i32, _loc: &Coord3D) {
        self.grp_cut("override_special_power_destination");
    }
    fn set_cheering(&mut self, _o: ObjectId, _frames: i32) {
        self.grp_cut("set_cheering");
    }
    fn sell(&mut self, _o: ObjectId) {
        self.grp_cut("sell");
    }
    fn toggle_overcharge(&mut self, _o: ObjectId) {
        self.grp_cut("toggle_overcharge");
    }
    fn do_command_button(&mut self, _o: ObjectId, _button: u32, _call: GroupButtonCall, _src: CommandSourceType) {
        self.grp_cut("do_command_button");
    }
    fn set_weapon_set_flag(&mut self, o: ObjectId, wst: i32) {
        self.set_weapon_set_flag(o, wst);
    }
    fn clear_weapon_set_flag(&mut self, o: ObjectId, wst: i32) {
        self.clear_weapon_set_flag(o, wst);
    }
    fn template_has_weapon_set(&mut self, _o: ObjectId, _wst: i32) -> bool {
        self.grp_cut("template_has_weapon_set");
        Default::default()
    }
    fn set_weapon_lock(&mut self, _o: ObjectId, _slot: i32, _lock: i32) -> bool {
        self.grp_cut("set_weapon_lock");
        Default::default()
    }
    fn release_weapon_lock(&mut self, _o: ObjectId, _lock: i32) {
        self.grp_cut("release_weapon_lock");
    }
    fn can_afford_upgrade(&mut self, _o: ObjectId, _upgrade: u32) -> bool {
        self.grp_cut("can_afford_upgrade");
        Default::default()
    }
    fn upgrade_is_object_type(&mut self, _upgrade: u32) -> bool {
        self.grp_cut("upgrade_is_object_type");
        Default::default()
    }
    fn has_upgrade(&mut self, _o: ObjectId, _upgrade: u32) -> bool {
        self.grp_cut("has_upgrade");
        Default::default()
    }
    fn affected_by_upgrade(&mut self, _o: ObjectId, _upgrade: u32) -> bool {
        self.grp_cut("affected_by_upgrade");
        Default::default()
    }
    fn can_produce_upgrade(&mut self, _o: ObjectId, _upgrade: u32) -> bool {
        self.grp_cut("can_produce_upgrade");
        Default::default()
    }
    fn production_queue_full_for_upgrade(&mut self, _o: ObjectId, _upgrade: u32) -> Option<bool> {
        self.grp_cut("production_queue_full_for_upgrade");
        Default::default()
    }
    fn production_queue_upgrade(&mut self, _o: ObjectId, _upgrade: u32) {
        self.grp_cut("production_queue_upgrade");
    }
    fn command_set_has_button_type(&mut self, _o: ObjectId, _command_type: i32) -> Option<bool> {
        self.grp_cut("command_set_has_button_type");
        Default::default()
    }
    fn set_emoticon(&mut self, _o: ObjectId, _name: &[u8], _duration: i32) {
        self.grp_cut("set_emoticon");
    }
}

/// The World as the AI groups' moves' host (batch I9c-2), generated as [`AiGroupHost`]'s impl is.
impl crate::logic::ai::ai_group_moves::AiGroupMoveHost for World {
    fn debug_ai_terrain(&mut self) -> bool {
        false
    }
    fn group_move_click_to_gather_factor(&mut self) -> f32 {
        self.statics.global_data.the_global_data().map_or(0.0, |g| g.group_move_click_to_gather_factor)
    }
    fn ai_distance_requires_group(&mut self) -> f32 {
        self.statics.ai_data.data.distance_requires_group
    }
    fn ai_min_distance_for_group(&mut self) -> f32 {
        self.statics.ai_data.data.min_distance_for_group
    }
    fn ai_min_infantry_for_group(&mut self) -> i32 {
        self.statics.ai_data.data.min_infantry_for_group
    }
    fn ai_min_vehicles_for_group(&mut self) -> i32 {
        self.statics.ai_data.data.min_vehicles_for_group
    }
    fn formation_offset(&mut self, o: ObjectId) -> Coord2D {
        self.find_object(o).map(|x| x.formation_offset).unwrap_or_default()
    }
    fn bounding_circle_radius(&mut self, o: ObjectId) -> f32 {
        self.find_object(o).map_or(0.0, |x| x.geometry_info.get_bounding_circle_radius())
    }
    fn major_radius(&mut self, o: ObjectId) -> f32 {
        self.find_object(o).map_or(0.0, |x| x.geometry_info.get_major_radius())
    }
    fn player_type(&mut self, o: ObjectId) -> Option<PlayerType> {
        let p = self.get_controlling_player(o)?;
        self.players.get_nth(p).map(|x| x.player_type)
    }
    fn ai_is_doing_ground_movement(&mut self, o: ObjectId) -> bool {
        self.ai_ask(o, ModuleCall::AiIsDoingGroundMovement).is_some_and(|r| r.as_bool())
    }
    fn ai_tmp_value(&mut self, o: ObjectId) -> i32 {
        self.ai_peek(o).map_or(0, |a| a.tmp_int)
    }
    fn ai_set_tmp_value(&mut self, o: ObjectId, v: i32) {
        self.with_ai_lent(o, |w| crate::logic::ai::ai_states::with_ai(w, |a, _| a.tmp_int = v));
    }
    fn ai_cur_locomotor_priority(&mut self, o: ObjectId) -> Option<i32> {
        self.ai_peek(o)?.get_cur_locomotor().map(|l| l.get_move_priority() as i32)
    }
    fn ai_follow_path(&mut self, o: ObjectId, path: &[Coord3D], src: CommandSourceType) {
        self.ai_ask(o, ModuleCall::AiFollowPath { path: path.to_vec(), ignore_object: INVALID_ID, command_source: src });
    }
    fn pf_remove_goal(&mut self, o: ObjectId) {
        if self.pf_ready().is_some() {
            self.with_pathfinder(|pf, w| pf.remove_goal(w, o));
        }
    }
    fn pf_adjust_destination(&mut self, o: ObjectId, dest: &mut Coord3D, group_dest: Option<&Coord3D>) {
        let mut p = *dest;
        let loco = self.ai_locomotor_set(o);
        let gd = group_dest.copied();
        if self.pathfinder_ask(o, |pf, w, obj| pf.adjust_destination(w, obj, loco, &mut p, gd.as_ref())).is_some() {
            *dest = p;
        }
    }
    fn pf_update_goal(&mut self, o: ObjectId, dest: &Coord3D) {
        self.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::UpdateGoal(o, *dest, crate::common::game_type::LAYER_GROUND));
    }
    fn pf_update_pos(&mut self, o: ObjectId, pos: &Coord3D) {
        self.pathfinder_op(crate::logic::world::pathfinder_host_world::PfOp::UpdatePos(o, *pos));
    }
    fn pf_is_line_passable(&mut self, o: ObjectId, from: &Coord3D, to: &Coord3D) -> bool {
        let loco = self.ai_locomotor_set(o);
        let (Some(facts), true) = (self.pf_object_facts(o), self.pf_ready().is_some()) else { return false };
        let layer = facts.layer;
        self.with_pathfinder(|pf, w| pf.is_line_passable(w, Some(&facts), loco.valid_surfaces, layer, from, to, false, true)).unwrap_or(false)
    }
    fn pf_find_ground_path(&mut self, from: &Coord3D, to: &Coord3D) -> Option<Vec<Coord3D>> {
        self.pf_ready()?;
        let path = self.with_pathfinder(|pf, w| pf.find_ground_path(w, from, to, crate::logic::ai::ai_group_moves::PATH_DIAMETER_IN_CELLS, false))??;
        Some(path_optimized_positions(&path))
    }
    fn layer_for_destination(&mut self, pos: &Coord3D) -> i32 {
        <World as crate::logic::ai::path::PathHost>::layer_for_destination(self, pos)
    }
    fn ground_height(&mut self, x: f32, y: f32) -> f32 {
        self.terrain_get_ground_height(x, y)
    }
    fn map_extent(&mut self) -> Region3D {
        <World as crate::logic::ai::queue::AlliesHost>::terrain_extent(self)
    }
    fn max_pathfind_extent(&mut self) -> Region3D {
        <World as crate::logic::ai::pathfinder::PathfinderHost>::maximum_pathfind_extent(self)
    }
}

impl World {
    /// A group command the dispatcher gives (batch I9c-2, with live groups): the `AIGroup` member it names, the player's
    /// command (`CMD_FROM_PLAYER`); `SetWeaponLock` and `IsIdle` answer, the others answer false. Nothing for no group.
    pub(crate) fn live_group_call(&mut self, g: u32, call: crate::logic::world::game_logic_dispatch::GroupCall) -> bool {
        use crate::logic::world::game_logic_dispatch::GroupCall as C;
        let src: CommandSourceType = crate::logic::ai::ai_update_types::CMD_FROM_PLAYER;
        let id = |o: u32| o as ObjectId;
        self.with_group(g, |gr, w| match call {
            C::SetWeaponLock { slot, lock } => gr.set_weapon_lock_for_group(w, slot, lock),
            C::ReleaseWeaponLock { lock } => {
                gr.release_weapon_lock_for_group(w, lock);
                false
            }
            C::AttackPosition { pos, max_shots } => {
                gr.group_attack_position(w, pos.as_ref(), max_shots, src);
                false
            }
            C::AttackObject { target, max_shots } => {
                gr.group_attack_object(w, id(target), max_shots, src);
                false
            }
            C::ForceAttackObject { target, max_shots } => {
                gr.group_force_attack_object(w, id(target), max_shots, src);
                false
            }
            C::AttackMoveTo { pos, max_shots } => {
                gr.group_attack_move_to_position(w, &pos, max_shots, src);
                false
            }
            C::MoveTo { pos, add_waypoint } => {
                gr.group_move_to_position(w, &pos, add_waypoint, src);
                false
            }
            C::CombatDrop { target, pos } => {
                gr.group_command(w, GroupAiCommand::CombatDrop { target: target.map_or(INVALID_ID, id), pos }, src);
                false
            }
            C::MineClearingDetail => {
                gr.set_mine_clearing_detail(w, true, crate::logic::weapon_types::WEAPONSET_MINE_CLEARING_DETAIL);
                false
            }
            C::SpecialPower { id: sp, options } => {
                gr.group_do_special_power(w, sp, options);
                false
            }
            C::SpecialPowerAtLocation { id: sp, pos, angle, object_in_way, options } => {
                gr.group_do_special_power_at_location(w, sp, &pos, angle, object_in_way.map_or(INVALID_ID, id), options);
                false
            }
            C::SpecialPowerAtObject { id: sp, target, options } => {
                gr.group_do_special_power_at_object(w, sp, id(target), options);
                false
            }
            C::OverrideSpecialPowerDestination { sp_type, pos } => {
                gr.group_override_special_power_destination(w, sp_type, &pos);
                false
            }
            C::GuardPosition { pos, mode } => {
                gr.group_guard(w, Some(GroupAiCommand::GuardPosition { pos, mode }), src);
                false
            }
            C::GuardObject { obj, mode } => {
                gr.group_guard(w, Some(GroupAiCommand::GuardObject { obj: id(obj), mode }), src);
                false
            }
            C::Idle => {
                gr.group_idle(w, src);
                false
            }
            C::Scatter => {
                gr.group_scatter(w, src);
                false
            }
            C::CreateFormation => {
                gr.group_create_formation(w);
                false
            }
            C::Cheer => {
                gr.group_cheer(w);
                false
            }
            C::Enter { obj } => {
                gr.group_command(w, GroupAiCommand::Enter { obj: id(obj) }, src);
                false
            }
            C::Evacuate => {
                gr.group_evacuate(w, src);
                false
            }
            C::ExecuteRailedTransport => {
                gr.group_command(w, GroupAiCommand::ExecuteRailedTransport, src);
                false
            }
            C::HackInternet => {
                gr.group_command(w, GroupAiCommand::HackInternet, src);
                false
            }
            C::GetRepaired { obj } => {
                gr.group_command(w, GroupAiCommand::GetRepaired { depot: id(obj) }, src);
                false
            }
            C::Dock { obj } => {
                gr.group_command(w, GroupAiCommand::Dock { obj: id(obj) }, src);
                false
            }
            C::GetHealed { obj } => {
                gr.group_command(w, GroupAiCommand::GetHealed { depot: id(obj) }, src);
                false
            }
            C::Repair { obj } => {
                gr.group_command(w, GroupAiCommand::Repair { obj: id(obj) }, src);
                false
            }
            C::ResumeConstruction { obj } => {
                gr.group_command(w, GroupAiCommand::ResumeConstruction { obj: id(obj) }, src);
                false
            }
            C::IsIdle => gr.is_idle(w),
            C::QueueUpgrade { key } => {
                gr.queue_upgrade(w, Some(key as u32));
                false
            }
            C::Sell => {
                gr.group_sell(w);
                false
            }
            C::ToggleOvercharge => {
                gr.group_toggle_overcharge(w);
                false
            }
        })
        .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::world::game_logic_dispatch::GroupCall;
    use crate::logic::world::pathfinder_host_world::PfOp;
    use crate::logic::world::player_host_world::tests::{make, world};

    /// `TheAI`'s groups in the World. A group made gets id 0 (`m_nextGroupID` from 0); three runners join it in order, each
    /// object's group the group. The player's move order (`groupMoveToPosition` to (300, 300)) moves them all: after 150 frames
    /// each is past (100, 100). The group is in the logic's CRC. `destroyGroup` takes it out of the list and each member leaves
    /// it. A second group whose last member leaves destroys itself (`AIGroup::remove`).
    #[test]
    fn groups_live_in_the_world() {
        let (mut w, team) = world();
        w.cuts.live_ai = true;
        w.cuts.live_teams = true;
        w.cuts.live_pathfinder = true;
        let runners: Vec<ObjectId> =
            (0..3).map(|i| make(&mut w, b"Runner", team, Coord3D { x: 40.0, y: 40.0 + 30.0 * i as f32, z: 0.0 })).collect();
        assert!(w.pathfinder_op(PfOp::NewMap));
        let g = w.ai_create_group();
        assert_eq!(g, 0);
        for &r in &runners {
            w.with_group(g, |gr, w| gr.add(w, r));
        }
        assert_eq!(w.ai_group_members(g), runners);
        assert!(runners.iter().all(|&r| w.find_object(r).unwrap().group == g));
        assert!(!w.live_group_call(g, GroupCall::MoveTo { pos: Coord3D { x: 300.0, y: 300.0, z: 0.0 }, add_waypoint: false }));
        for _ in 0..150 {
            w.logic_update().unwrap();
        }
        for &r in &runners {
            let p = w.find_object(r).unwrap().get_position();
            assert!(p.x > 100.0 && p.y > 100.0, "runner {r} at {p:?}");
        }
        let with_group = w.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).unwrap();
        w.ai_destroy_group_live(g);
        assert!(w.ai_groups.list.is_empty());
        assert!(runners.iter().all(|&r| w.find_object(r).unwrap().group == 0));
        let without = w.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).unwrap();
        assert_ne!(with_group, without);
        let g2 = w.ai_create_group();
        assert_eq!(g2, 1);
        w.with_group(g2, |gr, w| gr.add(w, runners[0]));
        assert_eq!(w.with_group(g2, |gr, w| gr.remove(w, runners[0])), Some(true));
        assert!(!w.ai_group_exists(g2));
        assert!(w.cuts.team_live_errors.is_empty(), "{:?}", w.cuts.team_live_errors);
    }
}
