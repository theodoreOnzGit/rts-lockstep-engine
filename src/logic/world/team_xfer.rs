// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp
//   (`Team::xfer`, `Team::loadPostProcess`, `TeamPrototype::xfer`, `TeamFactory::xfer`,
//   `TeamFactory::loadPostProcess`)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Save, load and post-process of teams, prototypes and the factory. Wave 7, `p1w9-sonnet`.
//!
//! `Team`, `TeamPrototype` and `TeamFactory` reach the game through the [`TeamHost`] when they are saved or
//! loaded (the player list, the terrain's waypoints, the object table, `createTeamOnPrototype`), so they are
//! functions over a host rather than `Snapshot` implementations; `TeamRelationMap`, `PlayerRelationMap` and
//! `TeamTemplateInfo`, which reach nothing, are `Snapshot`s (in `team.rs` and `player.rs`).
//!
//! # How `xferSnapshot` of a nested snapshot is kept
//!
//! `Xfer::xferSnapshot(s)` is `s->xfer(this)`, and on a load it counts one post-process request
//! (`XferLoad::xfer_snapshot`). [`xfer_nested`] reaches that through the xfer's own `xfer_snapshot` by lending
//! it a one-off [`Snapshot`] that holds the host for the duration of the call: the one place in the translation
//! with a lifetime parameter, private to this file.
//!
//! # Where upstream throws
//!
//! `SC_INVALID_DATA` ([`TeamError::InvalidData`]): a team id that differs from the one in the file, a number of
//! generic scripts that is not 16, a prototype count or prototype id the factory does not have, a member id no
//! object has, a member that is not on the team's list after the load. A save of a prototype with no owner
//! dereferences `NULL` ([`TeamError::Null`]).
//!
//! # Upstream quirks kept
//!
//! - `TeamPrototype::xfer` assigns `m_owningPlayer` from the player list **on a save as well**.
//! - A load creates a missing team with `createTeamOnPrototype` (which allocates the next id, makes the team active,
//!   and sets `m_created`), then gives it the saved id; the factory's counter is rebuilt afterwards.
//! - `TeamFactory::xfer` writes `m_uniqueTeamID` but not the prototype counter.

use crate::common::game_type::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus};

use super::ids::{TeamId, TEAM_ID_INVALID};
use super::team::*;
use super::team_ops::*;

type TResult<T> = Result<T, TeamError>;

/// `xferSnapshot` of a snapshot that needs the host (see the module docs).
pub fn xfer_nested<H, F>(host: &mut H, xfer: &mut dyn Xfer, f: F) -> TResult<()>
where
    F: FnMut(&mut H, &mut dyn Xfer) -> TResult<()>,
{
    struct Nested<'a, H, F> {
        host: &'a mut H,
        f: F,
        err: Option<TeamError>,
    }
    impl<'a, H, F: FnMut(&mut H, &mut dyn Xfer) -> TResult<()>> Snapshot for Nested<'a, H, F> {
        fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
            Ok(())
        }
        fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
            match (self.f)(self.host, xfer) {
                Ok(()) => Ok(()),
                Err(e) => {
                    self.err = Some(e);
                    Err(XferStatus::ErrorUnknown)
                }
            }
        }
        fn load_post_process(&mut self) {}
    }
    let mut n = Nested { host, f, err: None };
    let r = xfer.xfer_snapshot(&mut n);
    match n.err.take() {
        Some(e) => Err(e),
        None => r.map_err(TeamError::Xfer),
    }
}

fn xfer_user_u32(xfer: &mut dyn Xfer, v: &mut u32) -> TResult<()> {
    let mut b = v.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    *v = u32::from_le_bytes(b);
    Ok(())
}

/// `Team::xfer(xfer)`, version 1. A load queues the member ids in `m_xferMemberIDList` for [`team_load_post_process`].
pub fn xfer_team<H: TeamHost>(h: &mut H, xfer: &mut dyn Xfer, team: TeamId) -> TResult<()> {
    let current_version = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    // xfer id, this is a sanity check as team id m_id should always be valid at this point
    let id = h.teams().team(team).ok_or(TeamError::Null)?.id;
    let mut team_id = id;
    xfer_user_u32(xfer, &mut team_id)?;
    if team_id != id {
        return Err(TeamError::InvalidData);
    }

    // member list count and data
    let mut member_count = h.teams().members(team).len() as u16;
    xfer.xfer_unsigned_short(&mut member_count)?;
    if xfer.get_xfer_mode() == XferMode::Save {
        // save all member info
        for mut member_id in h.teams().members(team) {
            xfer.xfer_object_id(&mut member_id)?;
        }
    } else {
        // load all members
        for _ in 0..member_count {
            let mut member_id: ObjectId = 0;
            xfer.xfer_object_id(&mut member_id)?;
            // put on pending list for later processing
            h.teams_mut().team_mut(team).ok_or(TeamError::Null)?.xfer_member_id_list.push(member_id);
        }
    }

    // state
    let mut state = h.teams().team(team).ok_or(TeamError::Null)?.state.clone();
    xfer.xfer_ascii_string(&mut state)?;
    h.teams_mut().team_mut(team).ok_or(TeamError::Null)?.state = state;

    let mut t = h.teams().team(team).ok_or(TeamError::Null)?.clone();
    // entered or exited, active status, created flag, check enemy sighted, see enemy, previous see enemy, was idle
    xfer.xfer_bool(&mut t.entered_or_exited)?;
    xfer.xfer_bool(&mut t.active)?;
    xfer.xfer_bool(&mut t.created)?;
    xfer.xfer_bool(&mut t.check_enemy_sighted)?;
    xfer.xfer_bool(&mut t.see_enemy)?;
    xfer.xfer_bool(&mut t.prev_see_enemy)?;
    xfer.xfer_bool(&mut t.was_idle)?;
    // destroy threshold, current units
    xfer.xfer_int(&mut t.destroy_threshold)?;
    xfer.xfer_int(&mut t.cur_units)?;

    // waypoint
    let mut current_waypoint_id: u32 = t.current_waypoint.unwrap_or(0);
    xfer.xfer_unsigned_int(&mut current_waypoint_id)?;
    if xfer.get_xfer_mode() == XferMode::Load {
        t.current_waypoint = h.terrain_waypoint_by_id(current_waypoint_id);
    }

    let mut should_attempt_count: u16 = MAX_GENERIC_SCRIPTS as u16;
    xfer.xfer_unsigned_short(&mut should_attempt_count)?;
    if should_attempt_count as usize != MAX_GENERIC_SCRIPTS {
        return Err(TeamError::InvalidData);
    }
    for i in 0..should_attempt_count as usize {
        xfer.xfer_bool(&mut t.should_attempt_generic_script[i])?;
    }

    // recruitability set, is recruitable
    xfer.xfer_bool(&mut t.is_recruitability_set)?;
    xfer.xfer_bool(&mut t.is_recruitable)?;

    // Common attack target.
    xfer.xfer_object_id(&mut t.common_attack_target)?;

    // team relations, player relations
    xfer.xfer_snapshot(&mut t.team_relations)?;
    xfer.xfer_snapshot(&mut t.player_relations)?;

    // the fields are written back at the end (a failure above leaves the team as the failed call left it upstream:
    // members already queued, the rest untouched is not reproduced; the error ends the load)
    let slot = h.teams_mut().team_mut(team).ok_or(TeamError::Null)?;
    let (member_head, xfer_list, state) = (slot.member_head, std::mem::take(&mut slot.xfer_member_id_list), std::mem::take(&mut slot.state));
    *slot = t;
    slot.member_head = member_head;
    slot.xfer_member_id_list = xfer_list;
    slot.state = state;
    Ok(())
}

/// `Team::loadPostProcess()`: the queued member ids must name objects that are on the team's list; the queue is
/// then cleared.
pub fn team_load_post_process<H: TeamHost>(h: &mut H, team: TeamId) -> TResult<()> {
    let ids = h.teams().team(team).ok_or(TeamError::Null)?.xfer_member_id_list.clone();
    for id in ids {
        // find object
        if !h.game_logic_find_object(id) {
            return Err(TeamError::InvalidData);
        }
        // the objects set their team during their own xfer; all that is checked here is that they did
        if !h.teams().is_in_member_list(team, id) {
            return Err(TeamError::InvalidData);
        }
    }
    // we're done with the xfer list now
    h.teams_mut().team_mut(team).ok_or(TeamError::Null)?.xfer_member_id_list.clear();
    Ok(())
}

/// `TeamPrototype::xfer(xfer)`, version 2.
pub fn xfer_team_prototype<H: TeamHost>(h: &mut H, xfer: &mut dyn Xfer, proto: u32) -> TResult<()> {
    let current_version = 2;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    // owning player index
    let mut owning_player_index: i32 = 0;
    if xfer.get_xfer_mode() == XferMode::Save {
        owning_player_index = h.teams().prototype(proto).ok_or(TeamError::Null)?.owning_player.ok_or(TeamError::Null)?;
    }
    xfer.xfer_int(&mut owning_player_index)?;
    let owner = h.player_list_nth(owning_player_index);
    h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.owning_player = owner;

    if version >= 2 {
        let mut name = h.teams().prototype(proto).ok_or(TeamError::Null)?.attack_priority_name.clone();
        xfer.xfer_ascii_string(&mut name)?;
        h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.attack_priority_name = name;
    }

    // production condition
    let mut always_false = h.teams().prototype(proto).ok_or(TeamError::Null)?.production_condition_always_false;
    xfer.xfer_bool(&mut always_false)?;
    h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.production_condition_always_false = always_false;

    // team template information
    let mut template = h.teams().prototype(proto).ok_or(TeamError::Null)?.team_template.clone();
    xfer.xfer_snapshot(&mut template)?;
    h.teams_mut().prototype_mut(proto).ok_or(TeamError::Null)?.team_template.production_priority = template.production_priority;

    // xfer team instance count
    let mut team_instance_count = h.teams().instances(proto).len() as u16;
    xfer.xfer_unsigned_short(&mut team_instance_count)?;

    // xfer team instances
    if xfer.get_xfer_mode() == XferMode::Save {
        // xfer each team instance
        for t in h.teams().instances(proto) {
            // write team id
            let mut team_id = t;
            xfer_user_u32(xfer, &mut team_id)?;
            // write team data
            xfer_nested(h, xfer, |h, x| xfer_team(h, x, t))?;
        }
    } else {
        // read each block
        for _ in 0..team_instance_count {
            // read team id
            let mut team_id: TeamId = TEAM_ID_INVALID;
            xfer_user_u32(xfer, &mut team_id)?;

            //
            // find this team, if it's not there create it ... note that there will in fact
            // be some teams in the instance when were are loading as they are creating during
            // the map load.  But since the team ids are reset on the map load, they are
            // created with exactly the same team IDs they had before
            //
            let team_instance = match factory_find_team_by_id(h, team_id) {
                Some(t) => t,
                None => {
                    // create team
                    let t = factory_create_team_on_prototype(h, Some(proto))?;
                    // restore original ID we read from the file
                    set_team_id(h, t, team_id)?;
                    team_id
                }
            };

            // xfer team data
            xfer_nested(h, xfer, |h, x| xfer_team(h, x, team_instance))?;
        }
    }
    Ok(())
}

/// `TeamFactory::xfer(xfer)`, version 1.
pub fn xfer_team_factory<H: TeamHost>(h: &mut H, xfer: &mut dyn Xfer) -> TResult<()> {
    let current_version = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    // unique team ID counter
    let mut unique_team_id = h.teams().last_team_id();
    xfer_user_u32(xfer, &mut unique_team_id)?;
    h.teams_mut().set_last_team_id(unique_team_id);

    // how many team prototypes of data do we have to write
    let mut prototype_count = h.teams().prototype_count() as u16;
    xfer.xfer_unsigned_short(&mut prototype_count)?;

    //
    // prototypes cannot change in number during run time so the count should be the
    // same as that already loaded into us from a map load
    //
    if prototype_count as usize != h.teams().prototype_count() {
        return Err(TeamError::InvalidData);
    }

    // xfer each of the prototype information
    if xfer.get_xfer_mode() == XferMode::Save {
        // iterate each prototype and xfer if it needs to be in the save file
        let all: Vec<_> = h.teams().by_key().map(|(_, p)| p).collect();
        for p in all {
            // xfer prototype id
            let mut id = h.teams().prototype(p).ok_or(TeamError::Null)?.id;
            xfer_user_u32(xfer, &mut id)?;
            // xfer prototype data
            xfer_nested(h, xfer, |h, x| xfer_team_prototype(h, x, p))?;
        }
    } else {
        // read all the team prototype info
        for _ in 0..prototype_count {
            // read prototype ID
            let mut id = 0u32;
            xfer_user_u32(xfer, &mut id)?;
            // find the prototype
            let p = factory_find_team_prototype_by_id(h, id).ok_or(TeamError::InvalidData)?;
            // xfer prototype data
            xfer_nested(h, xfer, |h, x| xfer_team_prototype(h, x, p))?;
        }
    }
    Ok(())
}

/// `TeamFactory::loadPostProcess()`: both counters become the highest id in use plus one.
pub fn factory_load_post_process<H: TeamHost>(h: &mut H) {
    h.teams_mut().recover_counters();
}
