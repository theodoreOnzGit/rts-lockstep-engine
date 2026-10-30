// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (aiDoCommand and the private* commands,
//     setGoalPositionClipped, isAllowedToRespondToAiCommands, doQuickExit, transferAttack)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AI.h (AICommandType, AICommandParms, AICommandInterface's ai* helpers)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AICommandParms's constructor; AIStateMachine's squad setters)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's commands (`p1w13-sonnet`, layer 10, batch A5): `AIUpdateInterface::aiDoCommand` and the `private*` members it
//! dispatches to, which set the machine's goal and state.
//!
//! The commands run on the lent AI ([`crate::logic::ai::ai_states::with_ai`]) and its running machine, as the machine's states do:
//! a state that gives a command (the idle state's crate pick-up and mood attack) calls [`ai_do_command`] with its machine, and the
//! world calls [`crate::logic::ai::ai_states::AiUpdateModule::ai_do_command`]. `aiDoCommand` is virtual upstream (the AI
//! subclasses override it); this is `AIUpdateInterface`'s.
//!
//! What the commands reach beyond the AI and its machine (the containers, the action manager, the weapon's shot count, the
//! partition's search of `privateAttackPosition`, the move-away path, the command set, the special modules a command hands over
//! to) is [`AiStateHost`]'s.
//!
//! # Upstream facts kept
//!
//! - `privateMoveToPosition` of a non-idle AI from the AI is a 20-second temporary move (the state is not cleared).
//! - `privateAttackPosition` with a continue range retargets the closest attackable object (ignoring stealth for the search),
//!   else fires once.
//! - The guard commands clear a retaliating machine first ("units move to zero (bottom left corner)").
//! - `privateRepair`, `privateResumeConstruction` and `privateExecuteRailedTransport` do nothing; `privateCombatDrop` is "default
//!   implementation, should never be called" and empties the container.
//! - The command buttons only answer `GUI_COMMAND_STOP` (idle) and, with an object, `GUI_COMMAND_COMBATDROP`.

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{DISABLED_HELD, DISABLED_SUBDUED};
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{KINDOF_AIRCRAFT, KINDOF_NO_COLLIDE, KINDOF_PROJECTILE};
use crate::common::object_status_types::OBJECT_STATUS_IGNORING_STEALTH;
use crate::common::state_machine::{SmResult, StateMachine};
use crate::logic::ai::ai_states::{
    ai, aism_add_to_goal_path, aism_clear, aism_set_state, aism_set_temporary_state, with_ai, AiState, AiStateHost, PassengerCommand,
};
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::ai_update::{AiMachineQuery, AiSubclass};
use crate::common::disabled_types::{DISABLED_EMP, DISABLED_HACKED, DISABLED_PARALYZED};
use crate::common::kind_of::KINDOF_PORTABLE_STRUCTURE;
use crate::logic::modules::plant;
use crate::logic::world::player::Squad;

/// `enum AICommandType` ("Stored in save file, do not reorder/renumber").
pub type AiCommandType = i32;
pub const AICMD_NO_COMMAND: AiCommandType = -1;
pub const AICMD_MOVE_TO_POSITION: AiCommandType = 0;
pub const AICMD_MOVE_TO_OBJECT: AiCommandType = 1;
pub const AICMD_TIGHTEN_TO_POSITION: AiCommandType = 2;
pub const AICMD_MOVE_TO_POSITION_AND_EVACUATE: AiCommandType = 3;
pub const AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT: AiCommandType = 4;
pub const AICMD_IDLE: AiCommandType = 5;
pub const AICMD_FOLLOW_WAYPOINT_PATH: AiCommandType = 6;
pub const AICMD_FOLLOW_WAYPOINT_PATH_AS_TEAM: AiCommandType = 7;
pub const AICMD_FOLLOW_USER_PATH: AiCommandType = 8;
pub const AICMD_FOLLOW_PATH: AiCommandType = 9;
pub const AICMD_FOLLOW_EXITPRODUCTION_PATH: AiCommandType = 10;
pub const AICMD_ATTACK_OBJECT: AiCommandType = 11;
pub const AICMD_FORCE_ATTACK_OBJECT: AiCommandType = 12;
pub const AICMD_ATTACK_TEAM: AiCommandType = 13;
pub const AICMD_ATTACK_POSITION: AiCommandType = 14;
pub const AICMD_ATTACKMOVE_TO_POSITION: AiCommandType = 15;
pub const AICMD_ATTACKFOLLOW_WAYPOINT_PATH: AiCommandType = 16;
pub const AICMD_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM: AiCommandType = 17;
pub const AICMD_HUNT: AiCommandType = 18;
pub const AICMD_REPAIR: AiCommandType = 19;
pub const AICMD_RESUME_CONSTRUCTION: AiCommandType = 20;
pub const AICMD_GET_HEALED: AiCommandType = 21;
pub const AICMD_GET_REPAIRED: AiCommandType = 22;
pub const AICMD_ENTER: AiCommandType = 23;
pub const AICMD_DOCK: AiCommandType = 24;
pub const AICMD_EXIT: AiCommandType = 25;
pub const AICMD_EVACUATE: AiCommandType = 26;
pub const AICMD_EXECUTE_RAILED_TRANSPORT: AiCommandType = 27;
pub const AICMD_GO_PRONE: AiCommandType = 28;
pub const AICMD_GUARD_POSITION: AiCommandType = 29;
pub const AICMD_GUARD_OBJECT: AiCommandType = 30;
pub const AICMD_GUARD_AREA: AiCommandType = 31;
pub const AICMD_DEPLOY_ASSAULT_RETURN: AiCommandType = 32;
pub const AICMD_ATTACK_AREA: AiCommandType = 33;
pub const AICMD_HACK_INTERNET: AiCommandType = 34;
pub const AICMD_FACE_OBJECT: AiCommandType = 35;
pub const AICMD_FACE_POSITION: AiCommandType = 36;
pub const AICMD_RAPPEL_INTO: AiCommandType = 37;
pub const AICMD_COMBATDROP: AiCommandType = 38;
pub const AICMD_COMMANDBUTTON_POS: AiCommandType = 39;
pub const AICMD_COMMANDBUTTON_OBJ: AiCommandType = 40;
pub const AICMD_COMMANDBUTTON: AiCommandType = 41;
pub const AICMD_WANDER: AiCommandType = 42;
pub const AICMD_WANDER_IN_PLACE: AiCommandType = 43;
pub const AICMD_PANIC: AiCommandType = 44;
pub const AICMD_BUSY: AiCommandType = 45;
pub const AICMD_FOLLOW_WAYPOINT_PATH_EXACT: AiCommandType = 46;
pub const AICMD_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT: AiCommandType = 47;
pub const AICMD_MOVE_AWAY_FROM_UNIT: AiCommandType = 48;
pub const AICMD_FOLLOW_PATH_APPEND: AiCommandType = 49;
pub const AICMD_MOVE_TO_POSITION_EVEN_IF_SLEEPING: AiCommandType = 50;
pub const AICMD_GUARD_TUNNEL_NETWORK: AiCommandType = 51;
pub const AICMD_EVACUATE_INSTANTLY: AiCommandType = 52;
pub const AICMD_EXIT_INSTANTLY: AiCommandType = 53;
pub const AICMD_GUARD_RETALIATE: AiCommandType = 54;
pub const AICMD_NUM_COMMANDS: AiCommandType = 55;

/// `GUICommandType`'s two the command buttons answer (ControlBar.h; `ALLOW_SURRENDER` undefined).
pub const GUI_COMMAND_STOP: i32 = 12;
pub const GUI_COMMAND_COMBATDROP: i32 = 25;

/// `struct AICommandParms`: objects by id (`INVALID_ID` for `NULL`), the team, waypoint, polygon trigger and command button by
/// the host's ids. (`m_damage` is the host's, for `privateGoProne`; `m_path` no command reads.)
#[derive(Clone, Debug, PartialEq)]
pub struct AiCommandParms {
    pub cmd: AiCommandType,
    pub cmd_source: CommandSourceType,
    pub pos: Coord3D,
    pub obj: ObjectId,
    pub other_obj: ObjectId,
    pub team: Option<u32>,
    pub coords: Vec<Coord3D>,
    pub waypoint: Option<u32>,
    pub polygon: Option<i32>,
    pub int_value: i32,
    pub command_button: Option<u32>,
}

impl AiCommandParms {
    /// `AICommandParms(cmd, cmdSource)`: everything else empty.
    pub fn new(cmd: AiCommandType, cmd_source: CommandSourceType) -> Self {
        AiCommandParms {
            cmd,
            cmd_source,
            pos: Coord3D::default(),
            obj: INVALID_ID,
            other_obj: INVALID_ID,
            team: None,
            coords: Vec::new(),
            waypoint: None,
            polygon: None,
            int_value: 0,
            command_button: None,
        }
    }
}

type Sm<H> = StateMachine<AiState<H>>;

/// `isAllowedToRespondToAiCommands(parms)`: not the dead, not a sleeping AI-controlled unit (but for the even-if-sleeping move),
/// not the player where the data forbids it.
pub fn is_allowed_to_respond_to_ai_commands<H: AiStateHost>(sm: &Sm<H>, h: &mut H, parms: &AiCommandParms) -> bool {
    // "the dead don't listen very well"
    if h.is_effectively_dead() {
        return false;
    }
    // "AI Units that are asleep actually ignore all commands."
    let mood = with_ai(h, |a, h| a.get_mood_matrix_value(&*h, Some(sm)));
    if mood & MM_CONTROLLER_AI != 0 && mood & MM_MOOD_SLEEP != 0 && parms.cmd != AICMD_MOVE_TO_POSITION_EVEN_IF_SLEEPING {
        return false;
    }
    let forbidden = ai(h).data.forbid_player_commands;
    if parms.cmd_source == CMD_FROM_PLAYER && forbidden && !plant(3250) {
        return false;
    }
    true
}

/// `setGoalPositionClipped(in, cmdSource)`: a player's goal kept half a partition cell (an aircraft's, its preferred height) inside
/// the map.
pub fn set_goal_position_clipped<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: Option<&Coord3D>, cmd_source: CommandSourceType) {
    let Some(p) = pos else {
        sm.set_goal_position(None);
        return;
    };
    let mut tmp = *p;
    if cmd_source == CMD_FROM_PLAYER {
        let mut fudge = h.partition_cell_size() * 0.5;
        if h.is_kind_of(KINDOF_AIRCRAFT) && h.is_significantly_above_terrain() {
            if let Some(ph) = ai(h).get_cur_locomotor().map(|l| l.get_preferred_height()) {
                // "aircraft must stay further away from the map edges, to prevent getting "lost""
                fudge = if fudge > ph { fudge } else { ph };
            }
        }
        let (lo, hi) = h.terrain_extent();
        if tmp.x < lo.x + fudge {
            tmp.x = lo.x + fudge;
        }
        if tmp.x > hi.x - fudge {
            tmp.x = hi.x - fudge;
        }
        if tmp.y < lo.y + fudge {
            tmp.y = lo.y + fudge;
        }
        if tmp.y > hi.y - fudge && !plant(3251) {
            tmp.y = hi.y - fudge;
        }
    }
    sm.set_goal_position(Some(&tmp));
}

fn set_goal_object<H: AiStateHost>(sm: &mut Sm<H>, h: &H, obj: ObjectId) {
    let o = h.find_object(obj);
    sm.set_goal_object(o.as_ref());
}

fn unblock<H: AiStateHost>(h: &mut H) {
    let a = ai(h);
    a.blocked_frames = 0;
    a.is_blocked = false;
    a.is_blocked_and_stuck = false;
}

fn set_last_command_source<H: AiStateHost>(h: &mut H, src: CommandSourceType) {
    ai(h).set_last_command_source(src);
}

/// `AIStateMachine::setGoalTeam(team)`: the goal squad (made if none) from the team's members.
pub fn aism_set_goal_team<H: AiStateHost>(h: &mut H, team: Option<u32>) {
    let members = team.map(|t| h.team_member_ids(t));
    let extras = &mut ai(h).machine;
    let squad = extras.goal_squad.get_or_insert_with(Squad::default);
    // Squad::squadFromTeam(team, TRUE)
    if let Some(members) = members {
        squad.object_ids.clear();
        squad.object_ids.extend(members);
    }
}

/// `aiDoCommand(parms)`: refused if not allowed to respond, else dispatched to its `private*` member (an unknown command does
/// nothing: a `DEBUG_CRASH`).
pub fn ai_do_command<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, parms: &AiCommandParms) -> SmResult<()> {
    // batch A34: `HackInternetAIUpdate::aiDoCommand`: the base's check, a command while hacking or packing kept, then the base's
    if matches!(ai(h).subclass, AiSubclass::HackInternet(_))
        && (!is_allowed_to_respond_to_ai_commands(sm, h, parms) || crate::logic::ai::hack_internet_ai_update::hack_internet_do_command(sm, h, parms)?)
    {
        return Ok(());
    }
    // batch A33: `RailedTransportAIUpdate::aiDoCommand`: the base's check, its own filter, then the base's
    if matches!(ai(h).subclass, AiSubclass::RailedTransport(_))
        && (!is_allowed_to_respond_to_ai_commands(sm, h, parms) || !crate::logic::ai::railed_transport_ai_update::railed_transport_allows(parms))
    {
        return Ok(());
    }
    // batch A32: `AssaultTransportAIUpdate::aiDoCommand` (the virtual every command reaches): its own part, then the base's
    if matches!(ai(h).subclass, AiSubclass::AssaultTransport(_)) {
        crate::logic::ai::assault_transport_ai_update::assault_transport_do_command(h, parms);
    }
    // batch A31: `DeployStyleAIUpdate::aiDoCommand` (the virtual every command reaches): its own check, then the base's
    if matches!(ai(h).subclass, AiSubclass::DeployStyle(_)) && !is_allowed_to_respond_to_ai_commands(sm, h, parms) {
        return Ok(());
    }
    if !is_allowed_to_respond_to_ai_commands(sm, h, parms) {
        return Ok(());
    }
    let p = parms;
    let src = p.cmd_source;
    match p.cmd {
        AICMD_MOVE_TO_POSITION | AICMD_MOVE_TO_POSITION_EVEN_IF_SLEEPING => private_move_to_position(sm, h, &p.pos, src),
        AICMD_MOVE_TO_OBJECT => private_move_to_object(sm, h, p.obj, src),
        AICMD_TIGHTEN_TO_POSITION => private_tighten_to_position(sm, h, &p.pos, src),
        AICMD_MOVE_TO_POSITION_AND_EVACUATE => private_move_to_and_evacuate(sm, h, &p.pos, src, false),
        AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT => private_move_to_and_evacuate(sm, h, &p.pos, src, true),
        AICMD_IDLE => {
            // (batch A35: `SupplyTruckAIUpdate::privateIdle`: the player's idle forces it busy)
            if let AiSubclass::SupplyTruck(s) = &mut ai(h).subclass {
                if src == CMD_FROM_PLAYER && !plant(3664) {
                    s.forced_busy_pending = true;
                }
            }
            private_idle(sm, h, src)
        }
        AICMD_FOLLOW_WAYPOINT_PATH => private_follow_waypoint_path(sm, h, p.waypoint, src, AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS),
        AICMD_FOLLOW_WAYPOINT_PATH_AS_TEAM => private_follow_waypoint_path(sm, h, p.waypoint, src, AI_FOLLOW_WAYPOINT_PATH_AS_TEAM),
        AICMD_FOLLOW_WAYPOINT_PATH_EXACT => private_follow_waypoint_path(sm, h, p.waypoint, src, AI_FOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS_EXACT),
        AICMD_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT => private_follow_waypoint_path(sm, h, p.waypoint, src, AI_FOLLOW_WAYPOINT_PATH_AS_TEAM_EXACT),
        AICMD_FOLLOW_PATH => private_follow_path(sm, h, &p.coords, p.obj, src, false),
        AICMD_FOLLOW_PATH_APPEND => private_follow_path_append(sm, h, &p.pos, src),
        AICMD_FOLLOW_EXITPRODUCTION_PATH => private_follow_path(sm, h, &p.coords, p.obj, src, true),
        AICMD_ATTACK_OBJECT => private_attack_object(sm, h, p.obj, p.int_value, src, AI_ATTACK_OBJECT),
        AICMD_FORCE_ATTACK_OBJECT => private_attack_object(sm, h, p.obj, p.int_value, src, AI_FORCE_ATTACK_OBJECT),
        AICMD_GUARD_RETALIATE => private_guard_retaliate(sm, h, p.obj, &p.pos, p.int_value, src),
        AICMD_ATTACK_TEAM => private_attack_team(sm, h, p.team, p.int_value, src),
        AICMD_ATTACK_POSITION => private_attack_position(sm, h, &p.pos, p.int_value, src),
        AICMD_ATTACKMOVE_TO_POSITION => private_attack_move_to_position(sm, h, &p.pos, p.int_value, src),
        AICMD_ATTACKFOLLOW_WAYPOINT_PATH => private_attack_follow_waypoint_path(sm, h, p.waypoint, p.int_value, false, src),
        AICMD_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM => private_attack_follow_waypoint_path(sm, h, p.waypoint, p.int_value, true, src),
        AICMD_HUNT => private_hunt(sm, h, src),
        AICMD_ATTACK_AREA => private_attack_area(sm, h, p.polygon, src),
        // privateRepair, privateResumeConstruction: `return;`
        AICMD_REPAIR | AICMD_RESUME_CONSTRUCTION => Ok(()),
        AICMD_GET_HEALED => {
            if !h.action_can_get_healed_at(p.obj, src) {
                return Ok(());
            }
            ai_enter(sm, h, p.obj, src)
        }
        AICMD_GET_REPAIRED => {
            if !h.action_can_get_repaired_at(p.obj, src) {
                return Ok(());
            }
            ai_dock(sm, h, p.obj, src)
        }
        AICMD_ENTER => private_enter(sm, h, p.obj, src),
        AICMD_DOCK => {
            private_goal_object_state(sm, h, p.obj, src, AI_DOCK, false)?;
            // (batch A35: `SupplyTruckAIUpdate::privateDock`: the player's dock is preferred)
            let dock = if h.object_exists(p.obj) { p.obj } else { INVALID_ID };
            if let AiSubclass::SupplyTruck(s) = &mut ai(h).subclass {
                if src == CMD_FROM_PLAYER && dock != INVALID_ID && !plant(3665) {
                    s.preferred_dock = dock;
                }
            }
            Ok(())
        }
        AICMD_EXIT => private_exit(sm, h, p.obj, src, AI_EXIT),
        AICMD_EXIT_INSTANTLY => private_exit(sm, h, p.obj, src, AI_EXIT_INSTANTLY),
        AICMD_EVACUATE if matches!(ai(h).subclass, AiSubclass::RailedTransport(_)) => {
            // (batch A33: `RailedTransportAIUpdate::privateEvacuate`)
            crate::logic::ai::railed_transport_ai_update::private_evacuate(h);
            Ok(())
        }
        AICMD_EVACUATE => private_evacuate(h, p.int_value, src, false),
        AICMD_EVACUATE_INSTANTLY => private_evacuate(h, p.int_value, src, true),
        // privateExecuteRailedTransport: empty
        // (batch A33: `RailedTransportAIUpdate::privateExecuteRailedTransport`; the base's does nothing)
        AICMD_EXECUTE_RAILED_TRANSPORT if matches!(ai(h).subclass, AiSubclass::RailedTransport(_)) => {
            crate::logic::ai::railed_transport_ai_update::private_execute_railed_transport(sm, h)
        }
        AICMD_EXECUTE_RAILED_TRANSPORT => Ok(()),
        AICMD_GO_PRONE => {
            h.go_prone();
            Ok(())
        }
        AICMD_GUARD_POSITION | AICMD_GUARD_OBJECT | AICMD_GUARD_TUNNEL_NETWORK | AICMD_GUARD_AREA => {
            // "If you were retaliating and ordered to enter guard mode, the state needs to be cleared before doing so"
            if sm.get_current_state_id() == AI_GUARD_RETALIATE && !plant(3252) {
                aism_clear(sm, h)?;
            }
            match p.cmd {
                AICMD_GUARD_POSITION => private_guard_position(sm, h, &p.pos, p.int_value, src),
                AICMD_GUARD_OBJECT => private_guard_object(sm, h, p.obj, p.int_value, src),
                AICMD_GUARD_TUNNEL_NETWORK => private_guard_tunnel_network(sm, h, p.int_value, src),
                _ => private_guard_area(sm, h, p.polygon, p.int_value, src),
            }
        }
        AICMD_HACK_INTERNET => {
            aism_clear(sm, h)?;
            set_last_command_source(h, src);
            // (batch A34: `findUpdateModule("HackInternetAIUpdate")` is this AI for a hacker, and none for another class: a
            // `DEBUG_CRASH`, nothing)
            if matches!(ai(h).subclass, AiSubclass::HackInternet(_)) {
                return crate::logic::ai::hack_internet_ai_update::hack_internet(sm, h);
            }
            Ok(())
        }
        AICMD_FACE_OBJECT => private_goal_object_state(sm, h, p.obj, src, AI_FACE_OBJECT, true),
        AICMD_FACE_POSITION => {
            if !h.is_mobile() {
                return Ok(());
            }
            aism_clear(sm, h)?;
            set_goal_position_clipped(sm, h, Some(&p.pos), src);
            unblock(h);
            set_last_command_source(h, src);
            aism_set_state(sm, h, AI_FACE_POSITION).map(|_| ())
        }
        AICMD_RAPPEL_INTO => {
            aism_clear(sm, h)?;
            set_goal_object(sm, h, p.obj);
            set_goal_position_clipped(sm, h, Some(&p.pos), src);
            unblock(h);
            set_last_command_source(h, src);
            aism_set_state(sm, h, AI_RAPPEL_INTO).map(|_| ())
        }
        AICMD_COMBATDROP => {
            // "default implementation, should never be called"
            if h.has_contain() {
                h.contain_remove_all_contained();
            }
            Ok(())
        }
        AICMD_COMMANDBUTTON => private_command_button(sm, h, p.command_button, None, src),
        AICMD_COMMANDBUTTON_OBJ => private_command_button(sm, h, p.command_button, Some(p.obj), src),
        AICMD_COMMANDBUTTON_POS => {
            // privateCommandButtonPosition: every command type is "not implemented" (a DEBUG_ASSERTCRASH)
            Ok(())
        }
        AICMD_WANDER => private_waypoint_state(sm, h, p.waypoint, src, AI_WANDER),
        AICMD_WANDER_IN_PLACE => {
            if !h.is_mobile() {
                return Ok(());
            }
            aism_clear(sm, h)?;
            set_last_command_source(h, src);
            aism_set_state(sm, h, AI_WANDER_IN_PLACE).map(|_| ())
        }
        AICMD_PANIC => private_waypoint_state(sm, h, p.waypoint, src, AI_PANIC),
        AICMD_BUSY => {
            aism_clear(sm, h)?;
            set_last_command_source(h, src);
            aism_set_state(sm, h, AI_BUSY).map(|_| ())
        }
        AICMD_MOVE_AWAY_FROM_UNIT => private_move_away_from_unit(sm, h, p.obj, src),
        // "unhandled AI command!" (DEBUG_CRASH)
        _ => Ok(()),
    }
}

/// `aiIdle(cmdSource)`.
pub fn ai_idle<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, src: CommandSourceType) -> SmResult<()> {
    ai_do_command(sm, h, &AiCommandParms::new(AICMD_IDLE, src))
}

/// `aiMoveToPosition(pos, cmdSource)`.
pub fn ai_move_to_position<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: &Coord3D, src: CommandSourceType) -> SmResult<()> {
    let mut parms = AiCommandParms::new(AICMD_MOVE_TO_POSITION, src);
    parms.pos = *pos;
    ai_do_command(sm, h, &parms)
}

/// `joinTeam()` (batch A23): the machine cleared, then the AI joins the first other member of its team with an AI that is not
/// held: moving to it when it is idle; else taking its goal (object, else position) and setting its own current state again
/// ("Match the state": its own, upstream's, which the clear has left the default).
pub fn join_team<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    // "the dead don't listen very well"
    if ai(h).is_ai_dead {
        return Ok(());
    }
    if !h.is_mobile() {
        return Ok(());
    }
    with_ai(h, |a, h| a.choose_locomotor_set(h, LOCOMOTORSET_NORMAL));
    aism_clear(sm, h)?;
    // (getStateMachine()->setGoalWaypoint(NULL))
    ai(h).machine.goal_waypoint = None;
    let me = h.object_id();
    let mut other = INVALID_ID;
    for o in h.own_team_member_ids() {
        if o == me {
            continue; // "it's us."
        }
        if h.object_ai_goal_object(o).is_some() && !h.object_is_disabled_by_type(o, DISABLED_HELD) {
            other = o;
            break;
        }
    }
    if other != INVALID_ID {
        if h.object_ai_is_idle(other) {
            let pos = h.object_position(other);
            return ai_move_to_position(sm, h, &pos, CMD_FROM_AI);
        }
        match h.object_ai_goal_object(other) {
            Some(g) if g != INVALID_ID && !plant(3520) => {
                let o = h.find_object(g);
                sm.set_goal_object(o.as_ref());
            }
            _ => {
                let p = h.object_ai_goal_position(other);
                sm.set_goal_position(Some(&p));
            }
        }
        let state = sm.get_current_state_id();
        set_last_command_source(h, CMD_FROM_AI);
        // "Match the state."
        aism_set_state(sm, h, state)?;
    }
    Ok(())
}

/// `aiMoveToObject(obj, cmdSource)`.
pub fn ai_move_to_object<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, obj: ObjectId, src: CommandSourceType) -> SmResult<()> {
    let mut parms = AiCommandParms::new(AICMD_MOVE_TO_OBJECT, src);
    parms.obj = obj;
    ai_do_command(sm, h, &parms)
}

/// `aiAttackObject(victim, maxShotsToFire, cmdSource)`.
pub fn ai_attack_object<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, victim: ObjectId, max_shots: i32, src: CommandSourceType) -> SmResult<()> {
    let mut parms = AiCommandParms::new(AICMD_ATTACK_OBJECT, src);
    parms.obj = victim;
    parms.int_value = max_shots;
    ai_do_command(sm, h, &parms)
}

/// `aiFollowPath(path, ignoreObject, cmdSource)` (batch A15).
pub fn ai_follow_path<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, path: &[Coord3D], ignore: ObjectId, src: CommandSourceType) -> SmResult<()> {
    let mut parms = AiCommandParms::new(AICMD_FOLLOW_PATH, src);
    parms.coords = path.to_vec();
    parms.obj = ignore;
    ai_do_command(sm, h, &parms)
}

/// `aiAttackObject(victim, maxShotsToFire, cmdSource)` given while the AI's machine is locked and lent (batch A18: from the enter
/// state inside a guard machine, whose update locks the AI's machine; `m` is the lent view of it): the machine's own `clear`,
/// `setGoalObject` and `setState` do nothing, the command's other effects are kept (`aiDoCommand`'s checks, `AIStateMachine::clear`'s
/// members and notice, the command source, the notice of a different state, the shot count).
pub fn ai_attack_object_on_locked_machine<H: AiStateHost, M: AiMachineQuery>(
    h: &mut H,
    m: &M,
    victim: ObjectId,
    max_shots: i32,
    src: CommandSourceType,
) -> SmResult<()> {
    // (batch A34: a hacker's aiDoCommand: no command waits any more (its machine is not hacking or packing here: it runs a guard))
    if let AiSubclass::HackInternet(k) = &mut ai(h).subclass {
        k.has_pending_command = false;
    }
    // (batch A33: a railed transport's aiDoCommand ignores the player's attack)
    if matches!(ai(h).subclass, AiSubclass::RailedTransport(_)) && src == CMD_FROM_PLAYER {
        return Ok(());
    }
    // (batch A32: an assault transport's aiDoCommand first)
    if matches!(ai(h).subclass, AiSubclass::AssaultTransport(_)) {
        let mut parms = AiCommandParms::new(AICMD_ATTACK_OBJECT, src);
        parms.obj = victim;
        parms.int_value = max_shots;
        crate::logic::ai::assault_transport_ai_update::assault_transport_do_command(h, &parms);
    }
    // isAllowedToRespondToAiCommands
    if h.is_effectively_dead() {
        return Ok(());
    }
    let mood = with_ai(h, |a, h| a.get_mood_matrix_value(&*h, Some(m)));
    if mood & MM_CONTROLLER_AI != 0 && mood & MM_MOOD_SLEEP != 0 {
        return Ok(());
    }
    if src == CMD_FROM_PLAYER && ai(h).data.forbid_player_commands {
        return Ok(());
    }
    // privateAttackObject (a transport's: its passengers first; batch A31, found by the deployers' run)
    transport_passengers_attack(h, src, PassengerCommand::AttackObject { victim, max_shots, src });
    if !h.object_exists(victim) {
        return Ok(());
    }
    // AIStateMachine::clear(): the machine's clear does nothing; its members cleared, the AI told
    let extras = &mut ai(h).machine;
    extras.goal_path.clear();
    extras.goal_waypoint = None;
    extras.goal_squad = None;
    with_ai(h, |a, h| a.friend_notify_state_machine_changed(h));
    // (setGoalObject: nothing)
    set_last_command_source(h, src);
    // AIStateMachine::setState: the machine's does nothing; the AI told when the state asked for differs
    if m.current_state_id() != AI_ATTACK_OBJECT {
        with_ai(h, |a, h| a.friend_notify_state_machine_changed(h));
    }
    h.weapon_set_max_shot_count(max_shots);
    Ok(())
}

/// `aiAttackMoveToPosition(pos, maxShotsToFire, cmdSource)`.
pub fn ai_attack_move_to_position<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: &Coord3D, max_shots: i32, src: CommandSourceType) -> SmResult<()> {
    let mut parms = AiCommandParms::new(AICMD_ATTACKMOVE_TO_POSITION, src);
    parms.pos = *pos;
    parms.int_value = max_shots;
    ai_do_command(sm, h, &parms)
}

/// `aiAttackFollowWaypointPath(way, maxShotsToFire, cmdSource)`, or `aiAttackFollowWaypointPathAsTeam` (`as_team`).
pub fn ai_attack_follow_waypoint_path<H: AiStateHost>(
    sm: &mut Sm<H>,
    h: &mut H,
    way: Option<u32>,
    max_shots: i32,
    as_team: bool,
    src: CommandSourceType,
) -> SmResult<()> {
    let mut parms = AiCommandParms::new(if as_team { AICMD_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM } else { AICMD_ATTACKFOLLOW_WAYPOINT_PATH }, src);
    parms.waypoint = way;
    parms.int_value = max_shots;
    ai_do_command(sm, h, &parms)
}

/// `aiEvacuate(exposeStealthUnits, cmdSource)`.
pub fn ai_evacuate<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, expose_stealth_units: i32, src: CommandSourceType) -> SmResult<()> {
    let mut parms = AiCommandParms::new(AICMD_EVACUATE, src);
    parms.int_value = expose_stealth_units;
    ai_do_command(sm, h, &parms)
}

/// `aiEnter(obj, cmdSource)`.
pub fn ai_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, obj: ObjectId, src: CommandSourceType) -> SmResult<()> {
    let mut parms = AiCommandParms::new(AICMD_ENTER, src);
    parms.obj = obj;
    ai_do_command(sm, h, &parms)
}

/// `aiDock(obj, cmdSource)`.
pub fn ai_dock<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, obj: ObjectId, src: CommandSourceType) -> SmResult<()> {
    let mut parms = AiCommandParms::new(AICMD_DOCK, src);
    parms.obj = obj;
    ai_do_command(sm, h, &parms)
}

/// `privateMoveToPosition(pos, cmdSource)`.
fn private_move_to_position<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: &Coord3D, src: CommandSourceType) -> SmResult<()> {
    if !h.is_mobile() {
        return Ok(());
    }
    let idle = ai(h).is_idle(&*sm);
    if !idle && src == CMD_FROM_AI && !plant(3253) {
        // "This is an internally generated move to, and we are in a non-idle state. ... Move for 20 seconds"
        set_goal_position_clipped(sm, h, Some(pos), src);
        unblock(h);
        aism_set_temporary_state(sm, h, AI_MOVE_TO, LOGICFRAMES_PER_SECOND * 20)?;
    } else {
        // "Normal user or script command, just do it."
        aism_clear(sm, h)?;
        set_goal_position_clipped(sm, h, Some(pos), src);
        unblock(h);
        set_last_command_source(h, src);
        aism_set_state(sm, h, AI_MOVE_TO)?;
    }
    Ok(())
}

/// `privateMoveToObject(obj, cmdSource)`.
fn private_move_to_object<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, obj: ObjectId, src: CommandSourceType) -> SmResult<()> {
    // "the dead don't listen very well"
    if ai(h).is_ai_dead {
        return Ok(());
    }
    private_goal_object_state(sm, h, obj, src, AI_MOVE_TO, true)
}

/// The commands that set the goal object and a state (`privateFaceObject`, `privateDock`, the rest of `privateMoveToObject`):
/// mobile only; with the blocked flags reset when `unblocks`.
fn private_goal_object_state<H: AiStateHost>(
    sm: &mut Sm<H>,
    h: &mut H,
    obj: ObjectId,
    src: CommandSourceType,
    state: u32,
    unblocks: bool,
) -> SmResult<()> {
    if !h.is_mobile() {
        return Ok(());
    }
    aism_clear(sm, h)?;
    set_goal_object(sm, h, obj);
    if unblocks {
        unblock(h);
    }
    set_last_command_source(h, src);
    aism_set_state(sm, h, state).map(|_| ())
}

/// `privateMoveToAndEvacuate(pos, cmdSource)` and `privateMoveToAndEvacuateAndExit` (whose payload a `DeliverPayloadAIUpdate`
/// delivers, if the object has one).
fn private_move_to_and_evacuate<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: &Coord3D, src: CommandSourceType, and_exit: bool) -> SmResult<()> {
    if !h.is_mobile() {
        return Ok(());
    }
    aism_clear(sm, h)?;
    set_goal_position_clipped(sm, h, Some(pos), src);
    unblock(h);
    set_last_command_source(h, src);
    if !and_exit {
        aism_set_state(sm, h, AI_MOVE_AND_EVACUATE)?;
    } else if !h.deliver_payload_via_module_data(pos) {
        aism_set_state(sm, h, AI_MOVE_AND_EVACUATE_AND_EXIT)?;
    }
    Ok(())
}

/// `privateIdle(cmdSource)`: idle, and so is every contained object with an AI.
fn private_idle<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, src: CommandSourceType) -> SmResult<()> {
    if h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(());
    }
    aism_clear(sm, h)?;
    aism_set_state(sm, h, AI_IDLE)?;
    set_last_command_source(h, src);
    h.contain_idle_passengers(src);
    Ok(())
}

/// `privateTightenToPosition(pos, cmdSource)`.
fn private_tighten_to_position<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: &Coord3D, src: CommandSourceType) -> SmResult<()> {
    if !h.is_mobile() {
        return Ok(());
    }
    aism_clear(sm, h)?;
    sm.set_goal_object(None);
    set_goal_position_clipped(sm, h, Some(pos), src);
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_MOVE_AND_TIGHTEN).map(|_| ())
}

/// `privateMoveAwayFromUnit(unit, cmdSource)`: a path out of the unit's way (and the previous one's), followed as the temporary
/// state for ten seconds.
fn private_move_away_from_unit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, unit: ObjectId, _src: CommandSourceType) -> SmResult<()> {
    // (isAllowedToMoveAwayFromUnit: the base class's TRUE)
    if ai(h).is_ai_in_dead_state() || !h.is_mobile() {
        return Ok(());
    }
    let id = unit;
    if ai(h).machine.temporary_state == AI_MOVE_OUT_OF_THE_WAY {
        for other in [ai(h).move_out_of_way1, ai(h).move_out_of_way2] {
            if other == id {
                if ai(h).is_blocked {
                    // "cheat for 2 seconds."
                    let now = sm_frame(h);
                    ai(h).set_ignore_collision_time(now, LOGICFRAMES_PER_SECOND * 2);
                }
                return Ok(());
            }
        }
    }
    let a = ai(h);
    a.move_out_of_way2 = a.move_out_of_way1;
    a.move_out_of_way1 = id;
    let obj2 = a.move_out_of_way2;
    let obj2 = if h.object_exists(obj2) { obj2 } else { INVALID_ID };
    let path2 = obj2 != INVALID_ID && h.object_ai_has_path(obj2);
    // "Path* unitPath = unit->getAI()->getPath()"
    if !h.object_ai_has_path(id) {
        return Ok(());
    }
    let mut new_path = h.pf_move_away_from_path(id, obj2, path2);
    if new_path.is_none() && !ai(h).can_path_through_units {
        ai(h).set_can_path_through_units(true);
        new_path = h.pf_move_away_from_path(id, obj2, path2);
    }
    if let Some(path) = new_path {
        let a = ai(h);
        a.destroy_path();
        a.path = Some(path);
        with_ai(h, |a, h| a.wake_up_now(h));
        aism_set_temporary_state(sm, h, AI_MOVE_OUT_OF_THE_WAY, 10 * LOGICFRAMES_PER_SECOND)?;
        // "If I don't collide with things, I don't need to tell them to get out of the way"
        if ai(h).path.is_some() && !h.is_kind_of(KINDOF_NO_COLLIDE) {
            let path = ai(h).path.take().expect("the path");
            h.pf_move_allies(&path);
            ai(h).path = Some(path);
        }
    }
    Ok(())
}

fn sm_frame<H: AiStateHost>(h: &H) -> u32 {
    crate::common::state_machine::StateContext::frame(h)
}

/// The four `privateFollowWaypointPath*` (mobile only: the waypoint, then the state).
fn private_follow_waypoint_path<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, way: Option<u32>, src: CommandSourceType, state: u32) -> SmResult<()> {
    if !h.is_mobile() {
        return Ok(());
    }
    aism_clear(sm, h)?;
    ai(h).machine.goal_waypoint = way;
    set_last_command_source(h, src);
    aism_set_state(sm, h, state).map(|_| ())
}

/// `privateWander`, `privatePanic` (the source, then the waypoint).
fn private_waypoint_state<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, way: Option<u32>, src: CommandSourceType, state: u32) -> SmResult<()> {
    if !h.is_mobile() {
        return Ok(());
    }
    aism_clear(sm, h)?;
    set_last_command_source(h, src);
    ai(h).machine.goal_waypoint = way;
    aism_set_state(sm, h, state).map(|_| ())
}

/// `privateFollowPathAppend(pos, cmdSource)`: appended to the path being followed, else a new path (from the goal position if
/// moving).
fn private_follow_path_append<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: &Coord3D, src: CommandSourceType) -> SmResult<()> {
    let a = ai(h);
    let effectively_moving = a.is_moving(&*sm) || a.waiting_for_path;
    if a.get_ai_state_type(&*sm) == AI_FOLLOW_PATH && !a.machine.goal_path.is_empty() && effectively_moving {
        aism_add_to_goal_path(&mut ai(h).machine, pos);
        Ok(())
    } else if effectively_moving && !plant(3254) {
        let path = vec![*sm.goal_position(), *pos];
        private_follow_path(sm, h, &path, INVALID_ID, src, false)
    } else {
        private_follow_path(sm, h, &[*pos], INVALID_ID, src, false)
    }
}

/// `privateFollowPath(path, ignoreObject, cmdSource, exitProduction)`.
fn private_follow_path<H: AiStateHost>(
    sm: &mut Sm<H>,
    h: &mut H,
    path: &[Coord3D],
    ignore_object: ObjectId,
    src: CommandSourceType,
    exit_production: bool,
) -> SmResult<()> {
    if !h.is_mobile() {
        return Ok(());
    }
    aism_clear(sm, h)?;
    if let Some(goal) = path.last() {
        sm.set_goal_position(Some(goal));
    }
    ai(h).machine.goal_path = path.to_vec();
    set_last_command_source(h, src);
    let ignore = if h.object_exists(ignore_object) { ignore_object } else { INVALID_ID };
    ai(h).ignore_obstacle(ignore);
    aism_set_state(sm, h, if exit_production { AI_FOLLOW_EXITPRODUCTION_PATH } else { AI_FOLLOW_PATH }).map(|_| ())
}

/// A transport's passengers' share of an attack (batch A30): `TransportAIUpdate`'s `privateAttackObject`,
/// `privateForceAttackObject` and `privateAttackPosition` give the command to each passenger with an AI first, when the
/// transport's contain lets its passengers fire and the command is the player's or a script's (a portable structure not when
/// hacked, EMP-ed, subdued or paralyzed).
fn transport_passengers_attack<H: AiStateHost>(h: &mut H, src: CommandSourceType, cmd: PassengerCommand) {
    if ai(h).subclass != AiSubclass::Transport {
        return;
    }
    if !(h.has_contain() && h.contain_passenger_allowed_to_fire()) {
        return;
    }
    if !(src == CMD_FROM_PLAYER || src == CMD_FROM_SCRIPT) && !plant(3611) {
        return;
    }
    let me = h.object_id();
    for passenger in h.contain_items(me) {
        if h.object_is_kind_of(passenger, KINDOF_PORTABLE_STRUCTURE)
            && (h.object_is_disabled_by_type(passenger, DISABLED_HACKED)
                || h.object_is_disabled_by_type(passenger, DISABLED_EMP)
                || h.object_is_disabled_by_type(passenger, DISABLED_SUBDUED)
                || h.object_is_disabled_by_type(passenger, DISABLED_PARALYZED))
            && !plant(3612)
        {
            continue;
        }
        if h.object_has_ai(passenger) {
            h.passenger_ai_command(passenger, cmd);
        }
    }
}

/// `privateAttackObject`, `privateForceAttackObject`: the victim (none: nothing), the state, the weapon's shot count. (A
/// transport's passengers first: [`transport_passengers_attack`].)
fn private_attack_object<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, victim: ObjectId, max_shots: i32, src: CommandSourceType, state: u32) -> SmResult<()> {
    let cmd = if state == AI_FORCE_ATTACK_OBJECT {
        PassengerCommand::ForceAttackObject { victim, max_shots, src }
    } else {
        PassengerCommand::AttackObject { victim, max_shots, src }
    };
    transport_passengers_attack(h, src, cmd);
    if !h.object_exists(victim) {
        return Ok(());
    }
    aism_clear(sm, h)?;
    set_goal_object(sm, h, victim);
    set_last_command_source(h, src);
    aism_set_state(sm, h, state)?;
    h.weapon_set_max_shot_count(max_shots);
    Ok(())
}

/// `privateGuardRetaliate(victim, pos, maxShotsToFire, cmdSource)`.
fn private_guard_retaliate<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, victim: ObjectId, pos: &Coord3D, max_shots: i32, src: CommandSourceType) -> SmResult<()> {
    if !h.object_exists(victim) {
        return Ok(());
    }
    aism_clear(sm, h)?;
    set_goal_object(sm, h, victim);
    set_goal_position_clipped(sm, h, Some(pos), src);
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_GUARD_RETALIATE)?;
    h.weapon_set_max_shot_count(max_shots);
    Ok(())
}

/// `privateAttackTeam(team, maxShotsToFire, cmdSource)`.
fn private_attack_team<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, team: Option<u32>, max_shots: i32, src: CommandSourceType) -> SmResult<()> {
    aism_clear(sm, h)?;
    aism_set_goal_team(h, team);
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_ATTACK_SQUAD)?;
    h.weapon_set_max_shot_count(max_shots);
    Ok(())
}

/// `privateAttackPosition(pos, maxShotsToFire, cmdSource)`: a weapon with a continue range attacks the closest attackable object
/// there instead (else fires once); a contact weapon goes to a reachable place around an unreachable position.
fn private_attack_position<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: &Coord3D, mut max_shots: i32, src: CommandSourceType) -> SmResult<()> {
    transport_passengers_attack(h, src, PassengerCommand::AttackPosition { pos: *pos, max_shots, src });
    let mut local_pos = *pos;
    let continue_range = h.weapon_continue_attack_range();
    if continue_range > 0.0 {
        h.set_status(OBJECT_STATUS_IGNORING_STEALTH, true);
        let victim = h.partition_closest_attack_target(&local_pos, continue_range, src);
        h.set_status(OBJECT_STATUS_IGNORING_STEALTH, false);
        if victim != INVALID_ID {
            return ai_attack_object(sm, h, victim, max_shots, src);
        }
        max_shots = 1;
    }
    if h.weapon_is_contact() && !h.is_path_available(&local_pos) {
        if let Some(tmp) = h.find_position_around(&local_pos, 0.0, 100.0) {
            local_pos = tmp;
        }
    }
    aism_clear(sm, h)?;
    ai(h).destroy_path();
    set_goal_position_clipped(sm, h, Some(&local_pos), src);
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_ATTACK_POSITION)?;
    sm.set_goal_object(None);
    h.weapon_set_max_shot_count(max_shots);
    Ok(())
}

/// `privateAttackMoveToPosition(pos, maxShotsToFire, cmdSource)`.
fn private_attack_move_to_position<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: &Coord3D, max_shots: i32, src: CommandSourceType) -> SmResult<()> {
    if ai(h).is_ai_dead || !h.is_mobile() {
        return Ok(());
    }
    aism_clear(sm, h)?;
    set_goal_position_clipped(sm, h, Some(pos), src);
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_ATTACK_MOVE_TO)?;
    h.weapon_set_max_shot_count(max_shots);
    Ok(())
}

/// `privateAttackFollowWaypointPath(way, maxShotsToFire, asTeam, cmdSource)`.
fn private_attack_follow_waypoint_path<H: AiStateHost>(
    sm: &mut Sm<H>,
    h: &mut H,
    way: Option<u32>,
    max_shots: i32,
    as_team: bool,
    src: CommandSourceType,
) -> SmResult<()> {
    if ai(h).is_ai_dead || !h.is_mobile() {
        return Ok(());
    }
    aism_clear(sm, h)?;
    ai(h).machine.goal_waypoint = way;
    set_last_command_source(h, src);
    aism_set_state(sm, h, if as_team { AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_TEAM } else { AI_ATTACKFOLLOW_WAYPOINT_PATH_AS_INDIVIDUALS })?;
    h.weapon_set_max_shot_count(max_shots);
    Ok(())
}

/// `privateHunt(cmdSource)`.
fn private_hunt<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, src: CommandSourceType) -> SmResult<()> {
    if !h.is_mobile() || h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(());
    }
    aism_clear(sm, h)?;
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_HUNT).map(|_| ())
}

/// `privateAttackArea(areaToGuard, cmdSource)`.
fn private_attack_area<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, area: Option<i32>, src: CommandSourceType) -> SmResult<()> {
    if !h.is_mobile() || h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(());
    }
    ai(h).area_to_guard = area;
    aism_clear(sm, h)?;
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_ATTACK_AREA).map(|_| ())
}

/// `privateEnter(obj, cmdSource)`: if the action manager allows it.
fn private_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, obj: ObjectId, src: CommandSourceType) -> SmResult<()> {
    if !h.is_mobile() {
        return Ok(());
    }
    if h.action_can_enter_object(obj, src) {
        aism_clear(sm, h)?;
        set_goal_object(sm, h, obj);
        set_last_command_source(h, src);
        aism_set_state(sm, h, AI_ENTER)?;
    }
    Ok(())
}

/// `privateExit`, `privateExitInstantly`: out of the given object, else of the container (none: nothing; a subdued one: nothing).
fn private_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, obj: ObjectId, src: CommandSourceType, state: u32) -> SmResult<()> {
    let mut object_to_exit = if h.object_exists(obj) { obj } else { INVALID_ID };
    if object_to_exit == INVALID_ID {
        object_to_exit = h.contained_by();
    }
    if object_to_exit == INVALID_ID {
        return Ok(());
    }
    if h.object_is_disabled_by_type(object_to_exit, DISABLED_SUBDUED) && !plant(3255) {
        return Ok(());
    }
    aism_clear(sm, h)?;
    set_goal_object(sm, h, object_to_exit);
    set_last_command_source(h, src);
    aism_set_state(sm, h, state).map(|_| ())
}

/// `doQuickExit(path)`: the exit path as the temporary state for ten seconds, the lock kept.
pub fn do_quick_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, path: &[Coord3D]) -> SmResult<()> {
    let locked = sm.is_locked();
    sm.unlock();
    ai(h).machine.goal_path = path.to_vec();
    aism_set_temporary_state(sm, h, AI_FOLLOW_EXITPRODUCTION_PATH, 10 * LOGICFRAMES_PER_SECOND)?;
    if locked {
        sm.lock();
    }
    Ok(())
}

/// `privateEvacuate`, `privateEvacuateInstantly`: not when subdued; the container's passengers (detected first if asked) exit.
fn private_evacuate<H: AiStateHost>(h: &mut H, expose_stealth_units: i32, src: CommandSourceType, instantly: bool) -> SmResult<()> {
    if h.is_disabled_by_type(DISABLED_SUBDUED) {
        return Ok(());
    }
    if h.has_contain() {
        if expose_stealth_units != 0 {
            h.contain_mark_all_passengers_detected();
        }
        h.contain_order_all_passengers_to_exit(src, instantly);
    }
    Ok(())
}

/// `privateGuardPosition(pos, guardMode, cmdSource)`: a player's position off the map is the closest edge point.
fn private_guard_position<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, pos: &Coord3D, guard_mode: GuardMode, src: CommandSourceType) -> SmResult<()> {
    if !h.is_mobile() || h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(());
    }
    note_guard_target(h, GUARDTARGET_LOCATION);
    let mut adj_pos = *pos;
    if src == CMD_FROM_PLAYER {
        let (lo, hi) = h.terrain_extent();
        // Region3D::isInRegionNoZ: strictly inside
        let inside = lo.x < adj_pos.x && adj_pos.x < hi.x && lo.y < adj_pos.y && adj_pos.y < hi.y;
        if !inside {
            adj_pos = h.terrain_find_closest_edge_point(&adj_pos);
        }
    }
    let a = ai(h);
    a.location_to_guard = adj_pos;
    a.guard_mode = guard_mode;
    aism_clear(sm, h)?;
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_GUARD).map(|_| ())
}

/// The guard target types: the second slot if empty, else the first.
fn note_guard_target<H: AiStateHost>(h: &mut H, t: GuardTargetType) {
    let a = ai(h);
    if a.guard_target_type[1] == GUARDTARGET_NONE && !plant(3256) {
        a.guard_target_type[1] = t;
    } else {
        a.guard_target_type[0] = t;
    }
}

/// `privateGuardTunnelNetwork(guardMode, cmdSource)`.
fn private_guard_tunnel_network<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, guard_mode: GuardMode, src: CommandSourceType) -> SmResult<()> {
    if !h.is_mobile() || h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(());
    }
    ai(h).guard_mode = guard_mode;
    aism_clear(sm, h)?;
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_GUARD_TUNNEL_NETWORK).map(|_| ())
}

/// `privateGuardObject(objectToGuard, guardMode, cmdSource)`.
fn private_guard_object<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, obj: ObjectId, guard_mode: GuardMode, src: CommandSourceType) -> SmResult<()> {
    if !h.is_mobile() || h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(());
    }
    note_guard_target(h, GUARDTARGET_OBJECT);
    let a = ai(h);
    a.guard_mode = guard_mode;
    a.object_to_guard = obj;
    aism_clear(sm, h)?;
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_GUARD).map(|_| ())
}

/// `privateGuardArea(areaToGuard, guardMode, cmdSource)`: the area's centre is the location to guard.
fn private_guard_area<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, area: Option<i32>, guard_mode: GuardMode, src: CommandSourceType) -> SmResult<()> {
    if !h.is_mobile() || h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(());
    }
    note_guard_target(h, GUARDTARGET_AREA);
    let center = h.polygon_center(area.expect("an area to guard (upstream dereferences it)"));
    let a = ai(h);
    a.area_to_guard = area;
    a.guard_mode = guard_mode;
    a.location_to_guard = center;
    // "just in case."
    a.object_to_guard = INVALID_ID;
    aism_clear(sm, h)?;
    set_last_command_source(h, src);
    aism_set_state(sm, h, AI_GUARD).map(|_| ())
}

/// `privateCommandButton(commandButton, cmdSource)` and `privateCommandButtonObject` (with `obj`): each slot of the object's command
/// set holding the button: STOP idles; with an object, COMBATDROP drops onto it; the others are "not implemented".
fn private_command_button<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, button: Option<u32>, obj: Option<ObjectId>, src: CommandSourceType) -> SmResult<()> {
    let Some(button) = button else { return Ok(()) };
    if h.is_kind_of(KINDOF_PROJECTILE) {
        return Ok(());
    }
    let (matches, command_type) = h.command_set_button_matches(button);
    for _ in 0..matches {
        match (obj, command_type) {
            (None, GUI_COMMAND_STOP) => ai_idle(sm, h, src)?,
            (Some(o), GUI_COMMAND_COMBATDROP) => {
                let pos = h.find_object(o).expect("the drop's object (upstream dereferences it)").position;
                let mut parms = AiCommandParms::new(AICMD_COMBATDROP, src);
                parms.obj = o;
                parms.pos = pos;
                ai_do_command(sm, h, &parms)?;
            }
            _ => {}
        }
    }
    Ok(())
}
