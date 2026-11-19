//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_hosts.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGroup.cpp (AIGroup::groupDoCommandButtonUsingWaypoints)
//!
//! Sweep 2 findings for this file:
//! - `AIGroup::groupSurrender` (AIGroup.cpp:2760), `groupPickUpPrisoner` (:2845), `groupReturnToPrison` (:2869):
//!   excluded, compiled only with `ALLOW_SURRENDER`, which no build defines (`crate::logic::ai::ai_group` says so too).

use crate::common::game_type::ObjectId;
use crate::logic::modules::plant;

/// What the group asks of a member: `source->doCommandButtonUsingWaypoints(commandButton, way, cmdSource)`
/// (Object.cpp), by the member's id.
pub trait AiGroupCommandHost {
    /// The command button, by its place in the World's button list; the waypoint by id; the source as the port's enum.
    fn object_do_command_button_using_waypoints(
        &mut self,
        member: ObjectId,
        command_button: usize,
        waypoint: Option<u32>,
        cmd_source: crate::common::game_common::CommandSourceType,
    );
}

/// `AIGroup::groupDoCommandButtonUsingWaypoints(commandButton, way, cmdSource)` (AIGroup.cpp:2954): every member
/// does the command button using the waypoint path, in member order. `members` is `m_memberList`.
pub fn group_do_command_button_using_waypoints<H: AiGroupCommandHost>(
    h: &mut H,
    members: &[ObjectId],
    command_button: usize,
    way: Option<u32>,
    cmd_source: crate::common::game_common::CommandSourceType,
) {
    let mut order: Vec<ObjectId> = members.to_vec();
    if plant(15729) {
        order.reverse();
    }
    if plant(15730) {
        order.pop();
    }
    for &source in &order {
        // source->doCommandButtonUsingWaypoints( commandButton, way, cmdSource );
        h.object_do_command_button_using_waypoints(source, command_button, if plant(15731) { None } else { way }, if plant(15732) { crate::common::game_common::CommandSourceType::FromPlayer } else { cmd_source });
    }
}
