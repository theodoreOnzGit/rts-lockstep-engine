//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AIStateMachine::setGoalSquad, setGoalAIGroup)
//!
//! Sweep 2 findings for this file:
//! - `AIStateMachine::setGoalPath` (AIStates.cpp:832), `getGoalWaypoint` (:1019), `getGoalSquad` (:1093): translated
//!   as field access, `crate::logic::ai::ai_update::AiMachineExtras::{goal_path, goal_waypoint, goal_squad}` (written
//!   in `crate::logic::ai::ai_commands`, e.g. `ai(h).machine.goal_path = path.to_vec()`).
//! - `AIStateMachine::getCurrentStateName` (:842): excluded, compiled only with `STATE_MACHINE_DEBUG` (`_DEBUG` or
//!   `_INTERNAL`, StateMachine.h:50), a debug name for the on-screen state dump.
//! - `AIStateMachine::setGoalTeam` is translated: `crate::logic::ai::ai_commands::aism_set_goal_team`.

use crate::logic::ai::ai_update::AiMachineExtras;
use crate::logic::world::player::Squad;
use crate::logic::world::team_ops::GroupRef;
use crate::logic::world::player_squads::{squad_from_ai_group, PlayerSquadHost};

/// `AIStateMachine::setGoalSquad(squad)` (AIStates.cpp:1073): "if (m_goalSquad == NULL) m_goalSquad =
/// newInstance(Squad); (*m_goalSquad) = (*squad);". The verified port has no setter: its callers write
/// `machine.goal_squad` directly.
pub fn aism_set_goal_squad(machine: &mut AiMachineExtras, squad: &Squad) {
    // (*m_goalSquad) = (*squad): Squad::operator= copies the id list; the cache is the target's own and is copied as well
    machine.goal_squad = Some(squad.clone());
}

/// `AIStateMachine::setGoalAIGroup(group)` (AIStates.cpp:1083): the goal squad (made if none) takes the group's ids,
/// `m_goalSquad->squadFromAIGroup(group, true)`.
pub fn aism_set_goal_ai_group<H: PlayerSquadHost>(machine: &mut AiMachineExtras, h: &H, group: Option<GroupRef>) {
    if machine.goal_squad.is_none() {
        machine.goal_squad = Some(Squad::default());
    }
    if let Some(s) = machine.goal_squad.as_mut() {
        squad_from_ai_group(h, s, group, true);
    }
}
