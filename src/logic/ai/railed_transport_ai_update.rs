// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/RailedTransportAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/RailedTransportAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `RailedTransportAIUpdate` (`p1w13-sonnet`, layer 10, batch A33): a transport on rails (the train of the "Railed transport"
//! maps) that shuttles along the map's waypoint paths named `<prefix>StartNN` to `<prefix>EndNN`, its dock open only while it
//! stands at a path's end.
//!
//! The module's state is [`RailedTransport`], held in [`AiSubclass::RailedTransport`]. [`AiUpdateModule::update`] takes
//! [`railed_transport_update`] around the base's update, [`commands::ai_do_command`] its `aiDoCommand` filter
//! ([`railed_transport_allows`]: the player may only start a transit or unload), and its `privateExecuteRailedTransport` and
//! `privateEvacuate` overrides; [`AiUpdateModule::xfer`] its `xfer` (version 1).
//!
//! # Upstream facts kept
//!
//! - The paths are stored at their number's slot (`m_path[i]` for `StartNN`/`EndNN` with `NN = i + 1`) but counted from 0: with a
//!   gap in the numbering, the slots below the count include an empty one (waypoint id 0) and miss the last path.
//! - The initial path is the one whose end is closest (strictly, the first of equals); the transit ends within 5 of the end
//!   waypoint or when the AI is idle; the next transit takes the next path, wrapping (to 0 even with no paths).
//! - `update` sets the current locomotor ultra-accurate after the base's update, every update.
//! - Following a path with no locomotor (the update follows it after the base's update, whose states may change the locomotor
//!   set) dereferences it in the waypoint state: refused (`StateMachineError::Undefined`) before the command.
//! - A path chosen with no end waypoint found (`closestPath` left `INVALID_PATH`, an assertion upstream) is indexed at -1 by the
//!   next update: not reached by maps whose named waypoints exist; refused here (the transit check is skipped).
//!
//! [`AiUpdateModule::update`]: crate::logic::ai::ai_states::AiUpdateModule::update
//! [`AiUpdateModule::xfer`]: crate::logic::ai::ai_states::AiUpdateModule::xfer
//! [`commands::ai_do_command`]: crate::logic::ai::ai_commands::ai_do_command

use crate::common::base_type::Coord3D;
use crate::common::state_machine::{SmResult, StateMachine, StateMachineError};
use crate::common::xfer::{Xfer, XferResult, XferStatus};
use crate::logic::ai::ai_commands::{self as commands, AiCommandParms, AICMD_EVACUATE, AICMD_EXECUTE_RAILED_TRANSPORT, AICMD_FOLLOW_WAYPOINT_PATH};
use crate::logic::ai::ai_states::{ai, AiState, AiStateHost};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::{CommandSourceType, CMD_FROM_AI, CMD_FROM_PLAYER};
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `MAX_WAYPOINT_PATHS`.
pub const MAX_WAYPOINT_PATHS: usize = 32;
/// `INVALID_PATH`.
pub const INVALID_PATH: i32 = -1;

/// `RailedTransportAIUpdateModuleData`'s own field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RailedTransportData {
    /// `PathPrefixName`: "prefix to use for waypont start and end points we'll look for".
    pub path_prefix_name: String,
}

/// `RailedTransportAIUpdate`'s members, with its module data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailedTransport {
    /// The module data's own field.
    pub data: RailedTransportData,
    /// `m_inTransit`.
    pub in_transit: bool,
    /// `m_path`: each path's start and end waypoint ids.
    pub path: [(u32, u32); MAX_WAYPOINT_PATHS],
    /// `m_numPaths`.
    pub num_paths: i32,
    /// `m_currentPath`.
    pub current_path: i32,
    /// `m_waypointDataLoaded`.
    pub waypoint_data_loaded: bool,
}

impl RailedTransport {
    /// `RailedTransportAIUpdate(thing, moduleData)`: no paths, none current, not loaded, not in transit.
    pub fn new(data: RailedTransportData) -> Self {
        RailedTransport {
            data,
            in_transit: false,
            path: [(0, 0); MAX_WAYPOINT_PATHS],
            num_paths: 0,
            current_path: INVALID_PATH,
            waypoint_data_loaded: false,
        }
    }
}

/// The running AI's `RailedTransportAIUpdate` members (the AI is one).
fn rt<H: AiStateHost>(h: &mut H) -> &mut RailedTransport {
    match &mut ai(h).subclass {
        AiSubclass::RailedTransport(r) => r,
        _ => unreachable!("a RailedTransportAIUpdate"),
    }
}

/// `aiFollowWaypointPath(way, cmdSource)`. A mobile AI without a locomotor would enter the follow-waypoint-path state, whose
/// `onEnter` dereferences the locomotor (undefined upstream: `StateMachineError::Undefined`, before anything is done).
fn ai_follow_waypoint_path<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, way: Option<u32>, src: CommandSourceType) -> SmResult<()> {
    if h.is_mobile() && ai(h).cur_locomotor.is_none() {
        return Err(StateMachineError::Undefined("a waypoint path followed without a locomotor"));
    }
    let mut parms = AiCommandParms::new(AICMD_FOLLOW_WAYPOINT_PATH, src);
    parms.waypoint = way;
    commands::ai_do_command(sm, h, &parms)
}

/// `RailedTransportAIUpdate::aiDoCommand(parms)`'s own filter (the base's `isAllowedToRespondToAiCommands` asked first by the
/// caller): "we ignore all commands from the player except the one to start a transit and to unload".
pub fn railed_transport_allows(parms: &AiCommandParms) -> bool {
    !(parms.cmd_source == CMD_FROM_PLAYER && parms.cmd != AICMD_EXECUTE_RAILED_TRANSPORT && parms.cmd != AICMD_EVACUATE && !plant(3640))
}

/// `loadWaypointData()`: "find all the possible waypoint paths we can use".
fn load_waypoint_data<H: AiStateHost>(h: &mut H) {
    let prefix = rt(h).data.path_prefix_name.clone();
    for i in 0..MAX_WAYPOINT_PATHS {
        // "find start waypoint", "find end waypoint"
        let start = h.waypoint_by_name(format!("{}Start{:02}", prefix, i + 1).as_bytes());
        let end = h.waypoint_by_name(format!("{}End{:02}", prefix, i + 1).as_bytes());
        // "if we have a start and an end, we have a valid path"
        if let (Some(start), Some(end)) = (start, end) {
            let ids = (h.waypoint_id(start), h.waypoint_id(end));
            let r = rt(h);
            r.path[if plant(3641) { r.num_paths as usize } else { i }] = ids;
            r.num_paths += 1;
        }
    }
    // "waypoint data is loaded"
    rt(h).waypoint_data_loaded = true;
}

/// `pickAndMoveToInitialLocation()`: "select the path with the closest ending waypoint to our location", follow it to its end.
fn pick_and_move_to_initial_location<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    let our_pos = h.position();
    let mut closest_end_waypoint = None;
    let mut closest_path = INVALID_PATH;
    let mut closest_dist = 99_999_999.9f32;
    for i in 0..rt(h).num_paths.max(0) as usize {
        let end_id = rt(h).path[i].1;
        if let Some(waypoint) = h.waypoint_by_id(end_id) {
            let loc = h.waypoint_location(waypoint);
            // "vector from us to waypoint"
            let v = Coord3D { x: loc.x - our_pos.x, y: loc.y - our_pos.y, z: loc.z - our_pos.z };
            let dist = v.length();
            // "if this distance is smaller, use this one"
            if dist < closest_dist || (plant(3642) && dist == closest_dist) {
                closest_dist = dist;
                closest_path = i as i32;
                closest_end_waypoint = Some(waypoint);
            }
        }
    }
    // "follow the waypoint path to its destination end point"
    ai_follow_waypoint_path(sm, h, closest_end_waypoint, CMD_FROM_AI)?;
    // "this is now our current path", "we are now 'in transit'"
    rt(h).current_path = closest_path;
    set_in_transit(h, true);
    Ok(())
}

/// `setInTransit(inTransit)`: the object's dock open when not in transit.
fn set_in_transit<H: AiStateHost>(h: &mut H, in_transit: bool) {
    h.railed_set_dock_open(!in_transit);
    rt(h).in_transit = in_transit;
}

/// `RailedTransportAIUpdate::update()`'s part before the base's update (the AI lent): "load the waypoint data if not loaded".
pub fn railed_transport_pre_update<H: AiStateHost>(h: &mut H) {
    if !rt(h).waypoint_data_loaded {
        load_waypoint_data(h);
    }
}

/// `RailedTransportAIUpdate::update()`'s part after the base's update (the AI lent): the locomotor made ultra-accurate, the
/// initial path picked, the transit's end found.
pub fn railed_transport_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    // "railed transports move ultra accurate like"
    if !plant(3643) {
        let a = ai(h);
        if let Some(cur) = a.cur_locomotor {
            if let Some(l) = a.locomotor_set.get_mut(cur) {
                l.set_ultra_accurate(true);
            }
        }
    }
    // "if we have no current path selected pick one and move to the end waypoint of that path"
    if rt(h).current_path == INVALID_PATH && rt(h).num_paths > 0 {
        pick_and_move_to_initial_location(sm, h)?;
    }
    // "if we're in transit, see if we're close enough to the destination waypoint to be considered as 'there' and open up the dock"
    if rt(h).in_transit {
        let cur = rt(h).current_path;
        if cur < 0 || cur as usize >= MAX_WAYPOINT_PATHS {
            return Ok(()); // (upstream indexes out of its array: see the module's notes)
        }
        let end_id = rt(h).path[cur as usize].1;
        let Some(waypoint) = h.waypoint_by_id(end_id) else {
            return Ok(()); // (upstream dereferences NULL)
        };
        let start = h.position();
        let end = h.waypoint_location(waypoint);
        let v = Coord3D { x: end.x - start.x, y: end.y - start.y, z: end.z - start.z };
        let dist = v.length();
        let idle = crate::logic::ai::ai_states::with_ai(h, |a, h| a.is_idle(&*sm, h));
        if dist <= 5.0 || idle {
            // "we are no longer in transit"
            set_in_transit(h, false);
        }
    }
    Ok(())
}

/// `privateExecuteRailedTransport(cmdSource)`: unless the railed dock is missing or busy, the next path's start followed, in
/// transit.
pub fn private_execute_railed_transport<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    // "if we've in the process of loading or unloading anything we can't do a transport sequence"
    if !h.railed_dock_present() || h.railed_dock_is_loading_or_unloading() {
        return Ok(());
    }
    // "pick the next path"
    let r = rt(h);
    r.current_path += 1;
    if r.current_path >= r.num_paths {
        r.current_path = 0;
    }
    // "find the start waypoint for our current path"
    let start_id = r.path[r.current_path as usize].0;
    let start = h.waypoint_by_id(start_id);
    ai_follow_waypoint_path(sm, h, start, CMD_FROM_AI)?;
    // "we are now in transit"
    set_in_transit(h, true);
    Ok(())
}

/// `privateEvacuate(exposeStealthUnits, cmdSource)`: the railed dock's `unloadAll`, unless missing, in transit or busy.
pub fn private_evacuate<H: AiStateHost>(h: &mut H) {
    if !h.railed_dock_present() {
        return;
    }
    // "can't unload when in transit"
    if rt(h).in_transit && !plant(3644) {
        return;
    }
    // "cannot evacuate when we're loading or unloading anything"
    if h.railed_dock_is_loading_or_unloading() {
        return;
    }
    // "start the manual undocking process"
    h.railed_dock_unload_all();
}

/// `RailedTransportAIUpdate::xfer`'s own members, after the base's.
pub fn railed_transport_xfer_members(r: &mut RailedTransport, xfer: &mut dyn Xfer) -> XferResult<()> {
    xfer.xfer_bool(&mut r.in_transit)?;
    xfer.xfer_int(&mut r.num_paths)?;
    if r.num_paths > MAX_WAYPOINT_PATHS as i32 {
        // "m_numPaths %d exceeds limit %d." (SC_INVALID_DATA)
        return Err(XferStatus::ErrorUnknown);
    }
    for i in 0..r.num_paths.max(0) as usize {
        xfer.xfer_unsigned_int(&mut r.path[i].0)?;
        xfer.xfer_unsigned_int(&mut r.path[i].1)?;
    }
    xfer.xfer_int(&mut r.current_path)?;
    if !plant(3645) {
        xfer.xfer_bool(&mut r.waypoint_data_loaded)?;
    }
    Ok(())
}
