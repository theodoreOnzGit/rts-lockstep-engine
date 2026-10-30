// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/ChinookAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ChinookAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `ChinookAIUpdate` (`p1w13-sonnet`, layer 10, batch A40): the transport helicopter, a supply truck that flies. Its AI's machine
//! (`ChinookAIStateMachine`) has fourteen more states ([`define_chinook_states`]): taking off and landing, moving to land, to
//! evacuate (landing, emptying, taking off again), to evacuate and leave the map, and the combat drop (moving over the target, then
//! dropping ropes its rappellers slide down). Its flight status ([`Chinook::flight_status`]) gates the commands: one given while it
//! takes off, lands or drops waits ([`Chinook::pending_command`]) and is given again once it is idle; one that needs it in the air
//! (or on the ground) takes off (or lands) first.
//!
//! The members are [`Chinook`] (the supply truck's, which `st` reaches, and its own), held in [`AiSubclass::Chinook`]; the supply
//! truck's machine is its own, as a supply truck's.
//!
//! # Upstream facts kept
//!
//! - `ChinookHeadOffMapState::onExit()` takes no exit status: it overrides nothing and never runs (the RIDER8 status it would clear
//!   stays). Its `onEnter` orders a move to the original position, which leaves the state at once: its `update` never runs.
//! - `isIdle` is never true with a command waiting, nor landed with passengers wanting in or out; the update asks the supply
//!   truck's (the base's) instead.
//! - The landing state loses every box; the combat drop state too.
//! - `ChinookMoveToBldgState::xfer` saves its own three heights only, not its move's members.
//! - A rope without a drawable leaves its speed and lengths uninitialised (`RopeInfo` on the stack): the drop state's update then
//!   reads them: undefined upstream (refused here).
//! - The update's rotor wash draws the client's random value (logged by the host): it never changes the logic.

use crate::common::base_type::Coord3D;
use crate::common::game_type::{ObjectId, INVALID_ID, LAYER_GROUND};
use crate::common::kind_of::{KINDOF_CAN_ATTACK, KINDOF_CAN_RAPPEL, KINDOF_FS_AIRFIELD, KINDOF_INFANTRY, KINDOF_PORTABLE_STRUCTURE, KINDOF_STRUCTURE};
use crate::common::object_status_types::OBJECT_STATUS_RIDER8;
use crate::common::state_machine::{SmResult, StateExitType, StateId, StateMachine, StateMachineError, StateReturn};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferVersion};
use crate::logic::ai::ai_commands::{
    self as commands, AiCommandParms, AICMD_BUSY, AICMD_EVACUATE, AICMD_EXIT, AICMD_FOLLOW_EXITPRODUCTION_PATH, AICMD_IDLE,
    AICMD_MOVE_TO_POSITION, AICMD_MOVE_TO_POSITION_AND_EVACUATE, AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT,
};
use crate::logic::ai::ai_move_states::{self as move_states, MoveToData};
use crate::logic::ai::ai_states::{ai, with_ai, AiState, AiStateHost};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::{CommandSourceType, AI_IDLE, CMD_FROM_AI, CMD_FROM_PLAYER, CMD_FROM_SCRIPT};
use crate::logic::ai::assault_transport_ai_update::MemberCommand;
use crate::logic::ai::hack_internet_ai_update::AiCommandParmsStorage;
use crate::logic::ai::supply_truck_ai_update::{self as supply, SupplyTruck, SupplyTruckData};
use crate::logic::ai::ai_update_types::{LocomotorSetType, LOCOMOTORSET_NORMAL, LOCOMOTORSET_TAXIING};
use crate::logic::modules::plant;
use crate::common::disabled_types::{DISABLED_EMP, DISABLED_HACKED, DISABLED_PARALYZED, DISABLED_SUBDUED};
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::ww::matrix3d::Matrix3D;

type Sm<H> = StateMachine<AiState<H>>;

/// `CHINOOK_TAKING_OFF` ("Stored in save file, don't renumber.").
pub const CHINOOK_TAKING_OFF: i32 = 0;
/// `CHINOOK_FLYING`.
pub const CHINOOK_FLYING: i32 = 1;
/// `CHINOOK_DOING_COMBAT_DROP`.
pub const CHINOOK_DOING_COMBAT_DROP: i32 = 2;
/// `CHINOOK_LANDING`.
pub const CHINOOK_LANDING: i32 = 3;
/// `CHINOOK_LANDED`.
pub const CHINOOK_LANDED: i32 = 4;

/// `TAKING_OFF` ("note that these must be distinct (numerically) from AIStateType. ick.": from 1001).
pub const TAKING_OFF: StateId = 1001;
/// `LANDING`.
pub const LANDING: StateId = 1002;
/// `MOVE_TO_AND_LAND`.
pub const MOVE_TO_AND_LAND: StateId = 1003;
/// `MOVE_TO_AND_EVAC`.
pub const MOVE_TO_AND_EVAC: StateId = 1004;
/// `LAND_AND_EVAC`.
pub const LAND_AND_EVAC: StateId = 1005;
/// `EVAC_AND_TAKEOFF`.
pub const EVAC_AND_TAKEOFF: StateId = 1006;
/// `MOVE_TO_AND_EVAC_AND_EXIT`.
pub const MOVE_TO_AND_EVAC_AND_EXIT: StateId = 1007;
/// `LAND_AND_EVAC_AND_EXIT`.
pub const LAND_AND_EVAC_AND_EXIT: StateId = 1008;
/// `EVAC_AND_EXIT`.
pub const EVAC_AND_EXIT: StateId = 1009;
/// `TAKEOFF_AND_EXIT`.
pub const TAKEOFF_AND_EXIT: StateId = 1010;
/// `HEAD_OFF_MAP`.
pub const HEAD_OFF_MAP: StateId = 1011;
/// `MOVE_TO_COMBAT_DROP`.
pub const MOVE_TO_COMBAT_DROP: StateId = 1012;
/// `DO_COMBAT_DROP`.
pub const DO_COMBAT_DROP: StateId = 1013;
/// `MOVE_TO_AND_EVAC_AND_EXIT_INIT`.
pub const MOVE_TO_AND_EVAC_AND_EXIT_INIT: StateId = 1014;

/// `BIGNUM` (the file's: 99999).
const BIGNUM: f32 = 99999.0;
/// `ExitDoorType`'s `DOOR_NONE_AVAILABLE`.
const DOOR_NONE_AVAILABLE: i32 = crate::logic::ai::ai_face_exit_states::DOOR_NONE_AVAILABLE;

/// `ChinookAIUpdateModuleData`'s own fields (and the supply truck's). The rope's look (width, colour, wobble) goes to its drawable
/// only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChinookData {
    /// The supply truck's module data.
    pub supply: SupplyTruckData,
    /// `RopeName` names a template (`TheThingFactory->findTemplate`; default "GenericRope").
    pub rope_template: bool,
    /// `RotorWashParticleSystem` names a template (`TheParticleSystemManager->findTemplate`; default none).
    pub rotor_wash: bool,
    /// `RappelSpeed` (default `fabs(gravity) * LOGICFRAMES_PER_SECOND * 0.5`).
    pub rappel_speed: f32,
    /// `RopeDropSpeed` (default 1e10, "um, fast.").
    pub rope_drop_speed: f32,
    /// `RopeWidth` (default 0.5).
    pub rope_width: f32,
    /// `RopeFinalHeight` (default 0).
    pub rope_final_height: f32,
    /// `RopeWobbleLen` (default 10).
    pub rope_wobble_len: f32,
    /// `RopeWobbleAmplitude` (default 1).
    pub rope_wobble_amp: f32,
    /// `RopeWobbleRate` (default 0.1).
    pub rope_wobble_rate: f32,
    /// `RopeColor` (default 0.9, 0.8, 0.7).
    pub rope_color: [f32; 3],
    /// `NumRopes` (default 4).
    pub num_ropes: u32,
    /// `PerRopeDelayMin` (frames; default 0x7fffffff).
    pub per_rope_delay_min: u32,
    /// `PerRopeDelayMax` (frames; default 0x7fffffff).
    pub per_rope_delay_max: u32,
    /// `MinDropHeight` (default 30).
    pub min_drop_height: f32,
    /// `WaitForRopesToDrop` (default true).
    pub wait_for_ropes_to_drop: bool,
    /// `UpgradedSupplyBoost` (default 0).
    pub upgraded_supply_boost: i32,
}

impl ChinookData {
    /// `ChinookAIUpdateModuleData()`'s defaults, the gravity given (the supply truck's data as given).
    pub fn new(supply: SupplyTruckData, gravity: f32) -> Self {
        ChinookData {
            supply,
            rope_template: false,
            rotor_wash: false,
            rappel_speed: gravity.abs() * LOGICFRAMES_PER_SECOND as f32 * 0.5,
            rope_drop_speed: 1e10,
            rope_width: 0.5,
            rope_final_height: 0.0,
            rope_wobble_len: 10.0,
            rope_wobble_amp: 1.0,
            rope_wobble_rate: 0.1,
            rope_color: [0.9, 0.8, 0.7],
            num_ropes: 4,
            per_rope_delay_min: 0x7fff_ffff,
            per_rope_delay_max: 0x7fff_ffff,
            min_drop_height: 30.0,
            wait_for_ropes_to_drop: true,
            upgraded_supply_boost: 0,
        }
    }
}

/// `ChinookAIUpdate`'s members (the supply truck's, and its own), with its module data.
#[derive(Clone, Debug, PartialEq)]
pub struct Chinook {
    /// The supply truck's members and data.
    pub supply: SupplyTruck,
    /// The module data's own fields.
    pub data: ChinookData,
    /// `m_pendingCommand`.
    pub pending_command: AiCommandParmsStorage,
    /// `m_flightStatus` (`CHINOOK_FLYING` at first, "even if we start 'on ground'").
    pub flight_status: i32,
    /// `m_airfieldForHealing`.
    pub airfield_for_healing: ObjectId,
    /// `m_originalPos`.
    pub original_pos: Coord3D,
    /// `m_hasPendingCommand`.
    pub has_pending_command: bool,
}

impl Chinook {
    /// `ChinookAIUpdate(thing, moduleData)`'s members.
    pub fn new(data: ChinookData) -> Self {
        Chinook {
            supply: SupplyTruck::new(data.supply),
            data,
            pending_command: AiCommandParmsStorage::default(),
            flight_status: CHINOOK_FLYING,
            airfield_for_healing: INVALID_ID,
            original_pos: Coord3D::default(),
            has_pending_command: false,
        }
    }
}

/// A rope of the combat drop (`RopeInfo`): its drawable (none: no rope template, or none made), the drop's start, its speed and
/// lengths (none: never set, a rope without a drawable), the next drop's frame, its rappellers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RopeInfo {
    /// `ropeDrawable` (its id).
    pub rope_drawable: Option<u32>,
    /// `dropStartMtx`.
    pub drop_start_mtx: Matrix3D,
    /// `ropeSpeed`, `ropeLen`, `ropeLenMax`.
    pub rope: Option<(f32, f32, f32)>,
    /// `nextDropTime`.
    pub next_drop_time: u32,
    /// `rappellerIDs`.
    pub rappeller_ids: Vec<ObjectId>,
}

/// A rope drawable's calls (`RopeDrawInterface`'s on each of its draw modules, and the drawable's own).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RopeCall {
    /// `setPosition(pos)`.
    SetPosition(Coord3D),
    /// `initRopeParms(length, width, color, wobbleLen, wobbleAmp, wobbleRate)`.
    InitParms { length: f32, width: f32, color: [f32; 3], wobble_len: f32, wobble_amp: f32, wobble_rate: f32 },
    /// `setRopeCurLen(length)`.
    SetCurLen(f32),
    /// `setRopeSpeed(curSpeed, maxSpeed, accel)`.
    SetSpeed { cur: f32, max: f32, accel: f32 },
    /// `setExpirationDate(frame)`.
    SetExpirationDate(u32),
}

/// The running AI's chinook members.
pub fn ck<H: AiStateHost>(h: &mut H) -> &mut Chinook {
    match &mut ai(h).subclass {
        AiSubclass::Chinook(c) => c,
        _ => unreachable!("a ChinookAIUpdate"),
    }
}

/// `ChinookAIStateMachine(owner, name)`'s own states (after the AI's).
pub fn define_chinook_states<H: AiStateHost>(sm: &mut Sm<H>) -> SmResult<()> {
    let landing = |landing| AiState::ChinookTakeoffOrLanding { dest_loc: Coord3D::default(), landing };
    let move_to = || AiState::MoveTo { mv: MoveToData::default(), is_move_to: true };
    sm.define_state(TAKING_OFF, landing(false), AI_IDLE, AI_IDLE, &[])?;
    sm.define_state(LANDING, landing(true), AI_IDLE, AI_IDLE, &[])?;
    sm.define_state(
        MOVE_TO_COMBAT_DROP,
        AiState::ChinookMoveToBldg { mv: MoveToData::default(), old_preferred_height: 0.0, new_preferred_height: 0.0, dest_z: 0.0 },
        DO_COMBAT_DROP,
        AI_IDLE,
        &[],
    )?;
    sm.define_state(DO_COMBAT_DROP, AiState::ChinookCombatDrop { ropes: Vec::new() }, AI_IDLE, AI_IDLE, &[])?;
    sm.define_state(MOVE_TO_AND_LAND, move_to(), LANDING, AI_IDLE, &[])?;
    sm.define_state(MOVE_TO_AND_EVAC, move_to(), LAND_AND_EVAC, AI_IDLE, &[])?;
    sm.define_state(LAND_AND_EVAC, landing(true), EVAC_AND_TAKEOFF, AI_IDLE, &[])?;
    sm.define_state(EVAC_AND_TAKEOFF, AiState::ChinookEvacuate, TAKING_OFF, AI_IDLE, &[])?;
    sm.define_state(MOVE_TO_AND_EVAC_AND_EXIT_INIT, AiState::ChinookRecordCreation, MOVE_TO_AND_EVAC_AND_EXIT, AI_IDLE, &[])?;
    sm.define_state(MOVE_TO_AND_EVAC_AND_EXIT, move_to(), LAND_AND_EVAC_AND_EXIT, AI_IDLE, &[])?;
    sm.define_state(LAND_AND_EVAC_AND_EXIT, landing(true), EVAC_AND_EXIT, AI_IDLE, &[])?;
    sm.define_state(EVAC_AND_EXIT, AiState::ChinookEvacuate, TAKEOFF_AND_EXIT, AI_IDLE, &[])?;
    sm.define_state(TAKEOFF_AND_EXIT, landing(false), HEAD_OFF_MAP, AI_IDLE, &[])?;
    sm.define_state(HEAD_OFF_MAP, AiState::ChinookHeadOffMap, AI_IDLE, AI_IDLE, &[])?;
    Ok(())
}

/// `calcDistSqr(a, b)` (3-D).
fn calc_dist_sqr(a: &Coord3D, b: &Coord3D) -> f32 {
    let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
    dx * dx + dy * dy + dz * dz
}

/// `getPotentialRappeller(obj)`: the first passenger that can rappel.
fn potential_rappeller<H: AiStateHost>(h: &mut H) -> ObjectId {
    if !h.has_contain() {
        return INVALID_ID;
    }
    let me = h.object_id();
    h.contain_items(me).into_iter().find(|&rider| h.object_is_kind_of(rider, KINDOF_CAN_RAPPEL)).unwrap_or(INVALID_ID)
}

/// The current locomotor, which upstream uses unchecked (none: undefined).
fn cur_loco_checked<H: AiStateHost>(h: &mut H, what: &'static str) -> SmResult<()> {
    if ai(h).get_cur_locomotor().is_none() {
        return Err(StateMachineError::Undefined(what));
    }
    Ok(())
}

/// The evacuating states' `onEnter`: the contain emptied, the team made active ("why? I don't know."); a success.
pub fn evacuate_on_enter<H: AiStateHost>(h: &mut H) -> StateReturn {
    if h.has_contain() {
        h.contain_remove_all_contained();
    }
    h.team_set_active();
    StateReturn::SUCCESS
}

/// The heading-off state's `onEnter`: RIDER8 set, a move to the original position (which leaves this state), invalid positions
/// allowed for the locomotor (unchecked).
pub fn head_off_map_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<StateReturn> {
    h.set_status_bits(&[OBJECT_STATUS_RIDER8], true);
    let pos = ck(h).original_pos;
    commands::ai_move_to_position(sm, h, &pos, CMD_FROM_AI)?;
    cur_loco_checked(h, "a chinook heading off the map without a locomotor")?;
    if let Some(l) = ai(h).get_cur_locomotor_mut() {
        l.set_allow_invalid_position(true);
    }
    Ok(StateReturn::CONTINUE)
}

/// The heading-off state's `update`: off the map (the extent with its border, in 2-D), destroyed (a success).
pub fn head_off_map_update<H: AiStateHost>(h: &mut H) -> StateReturn {
    let (lo, hi) = h.terrain_extent_including_border();
    let p = h.position();
    if !(p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y) && !plant(3750) {
        h.destroy_object();
        return StateReturn::SUCCESS;
    }
    StateReturn::CONTINUE
}

/// The taking-off and landing states' `onEnter`: the flight status; landing, every box lost; the drift scrubbed; the normal
/// locomotor set, precise and ultra accurate; the destination the highest layer's height under it (landing: a clear spot around,
/// adjusted for landing, on the layer at the chinook's height there; taking off: the preferred height above, on the ground layer).
pub fn takeoff_or_landing_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let Some(AiState::ChinookTakeoffOrLanding { landing, .. }) = sm.state(id) else { unreachable!("the taking-off or landing state") };
    let landing = *landing;
    ck(h).flight_status = if landing { CHINOOK_LANDING } else { CHINOOK_TAKING_OFF };
    if landing {
        // "A chinook given transport duty loses his supplies."
        lose_all_boxes(h);
    }
    // "kill any drift..."
    if !h.has_physics() {
        return Err(StateMachineError::Undefined("a chinook taking off or landing without physics"));
    }
    h.scrub_velocity_2d(0.0);
    with_ai(h, |a, h| a.choose_locomotor_set(h, LOCOMOTORSET_NORMAL));
    cur_loco_checked(h, "a chinook taking off or landing without a locomotor")?;
    let preferred_height = {
        let l = ai(h).get_cur_locomotor_mut().expect("the locomotor");
        l.set_use_precise_z_pos(true);
        l.set_ultra_accurate(true);
        l.get_preferred_height()
    };
    let mut dest = h.position();
    let layer_at_dest = h.highest_layer_for_healthy_destination(&dest);
    dest.z = h.layer_height(dest.x, dest.y, layer_at_dest);
    if landing {
        let max_radius = h.bounding_circle_radius() * 100.0;
        if let Some(tmp) = h.find_position_around_no_source(&dest, 0.0, max_radius) {
            dest = tmp;
            h.pf_adjust_to_landing_destination(&mut dest);
        }
        // "recalc, since it may have changed."
        let mut tmp = dest;
        tmp.z = h.position().z;
        let layer_at_dest = h.highest_layer_for_healthy_destination(&tmp);
        dest.z = h.layer_height(dest.x, dest.y, layer_at_dest);
        h.object_set_layer(layer_at_dest);
    } else {
        dest.z += preferred_height;
        h.object_set_layer(LAYER_GROUND);
    }
    if let Some(AiState::ChinookTakeoffOrLanding { dest_loc, .. }) = sm.state_mut(id) {
        *dest_loc = dest;
    }
    Ok(StateReturn::CONTINUE)
}

/// The taking-off and landing states' `update`: dead, a failure; else the locomotor's goal the destination, a success within 3.
pub fn takeoff_or_landing_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> StateReturn {
    if h.is_effectively_dead() {
        return StateReturn::FAILURE;
    }
    let Some(AiState::ChinookTakeoffOrLanding { dest_loc, .. }) = sm.state(id) else { unreachable!("the taking-off or landing state") };
    let dest = *dest_loc;
    ai(h).set_locomotor_goal_position_explicit(&dest);
    const THRESH: f32 = 3.0;
    if calc_dist_sqr(&h.position(), &dest) <= THRESH * THRESH && !plant(3751) {
        return StateReturn::SUCCESS;
    }
    StateReturn::CONTINUE
}

/// The taking-off and landing states' `onExit`: landed (or flying); the locomotor (if any) no longer precise nor ultra accurate,
/// its lift restored unless dead; landed, the taxiing set; flown, the ground layer.
pub fn takeoff_or_landing_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let Some(AiState::ChinookTakeoffOrLanding { landing, .. }) = sm.state(id) else { unreachable!("the taking-off or landing state") };
    let landing = *landing;
    ck(h).flight_status = if landing { CHINOOK_LANDED } else { CHINOOK_FLYING };
    let dead = h.is_effectively_dead();
    if let Some(l) = ai(h).get_cur_locomotor_mut() {
        l.set_use_precise_z_pos(false);
        l.set_ultra_accurate(false);
        // "don't restore lift if dead -- this may fight with JetSlowDeathBehavior!"
        if !dead {
            l.set_max_lift(BIGNUM);
        }
    }
    if landing {
        with_ai(h, |a, h| a.choose_locomotor_set(h, LOCOMOTORSET_TAXIING));
    } else {
        // "when takeoff is complete, always go back to layer-ground, rather than some bridge layer."
        h.object_set_layer(LAYER_GROUND);
    }
}

/// `while( ai->loseOneBox() );`.
fn lose_all_boxes<H: AiStateHost>(h: &mut H) {
    loop {
        let mut s = ck(h).supply;
        let lost = supply::lose_one_box(h, &mut s);
        ck(h).supply = s;
        if !lost {
            break;
        }
    }
}

/// The record-creation state's `onEnter`: the original position recorded; a success.
pub fn record_creation_on_enter<H: AiStateHost>(h: &mut H) -> StateReturn {
    let p = h.position();
    ck(h).original_pos = p;
    StateReturn::SUCCESS
}

/// The move-to-building state's `onEnter`: ultra accurate; over a live structure (the goal) the preferred height its top plus the
/// minimum drop height (not lower than before), else over the goal position; the destination's height; then the move's.
pub fn move_to_bldg_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    cur_loco_checked(h, "a chinook moving to its drop without a locomotor")?;
    let old = {
        let l = ai(h).get_cur_locomotor_mut().expect("the locomotor");
        l.set_ultra_accurate(true);
        l.get_preferred_height()
    };
    let mut new = old;
    let bldg = sm.goal_object(h).map_or(INVALID_ID, |o| o.id);
    let dest_pos = if bldg != INVALID_ID && !h.object_is_effectively_dead(bldg) && h.object_is_kind_of(bldg, KINDOF_STRUCTURE) {
        let min_drop_height = ck(h).data.min_drop_height;
        new = h.object_max_height_above_position(bldg) + min_drop_height;
        if new < old {
            new = old;
        }
        h.object_position(bldg)
    } else {
        *sm.goal_position()
    };
    ai(h).get_cur_locomotor_mut().expect("the locomotor").set_preferred_height(new);
    let dest_z = h.ground_height(dest_pos.x, dest_pos.y) + new;
    if let Some(AiState::ChinookMoveToBldg { old_preferred_height, new_preferred_height, dest_z: z, .. }) = sm.state_mut(id) {
        *old_preferred_height = old;
        *new_preferred_height = new;
        *z = dest_z;
    }
    move_states::move_to_on_enter(sm, h, id)
}

/// The move-to-building state's `update`: the move's, kept going while more than 3 off the destination's height ("the normal
/// moveto state will bail when 2d pos matches; we need z, too").
pub fn move_to_bldg_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let mut status = move_states::move_to_update(sm, h, id, true)?;
    let dest_z = match sm.state(id) {
        Some(AiState::ChinookMoveToBldg { dest_z, .. }) => *dest_z,
        _ => unreachable!("the move-to-building state"),
    };
    const THRESH: f32 = 3.0;
    if status != StateReturn::CONTINUE && (h.position().z - dest_z).abs() > THRESH && !plant(3752) {
        status = StateReturn::CONTINUE;
    }
    Ok(status)
}

/// The move-to-building state's `onExit`: the old preferred height back, not ultra accurate; then the move's.
pub fn move_to_bldg_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, status: StateExitType) -> SmResult<()> {
    let old = match sm.state(id) {
        Some(AiState::ChinookMoveToBldg { old_preferred_height, .. }) => *old_preferred_height,
        _ => unreachable!("the move-to-building state"),
    };
    cur_loco_checked(h, "a chinook leaving its move to the drop without a locomotor")?;
    let l = ai(h).get_cur_locomotor_mut().expect("the locomotor");
    l.set_preferred_height(old);
    l.set_ultra_accurate(false);
    move_states::on_exit(sm, h, id, status);
    Ok(())
}

/// The combat drop state's `onEnter`: no drawable, a failure; else held, dropping, every box lost; a rope per rope start and end
/// bone (at most `NumRopes`; none, a failure), each with its drawable (if the rope's template is found: placed at its start,
/// its length to the layer below less the final height) and its first drop's frame (random).
pub fn combat_drop_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if !h.has_drawable() {
        return Ok(StateReturn::FAILURE);
    }
    let d = ck(h).data;
    h.set_disabled_held(true);
    ck(h).flight_status = CHINOOK_DOING_COMBAT_DROP;
    // "A chinook given combat drop duty also loses his supplies."
    lose_all_boxes(h);
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    const MAX_BONES: usize = 32;
    let rope_pos = h.drawable_pristine_bone_positions(b"RopeStart", MAX_BONES);
    let drop_mtx = h.drawable_pristine_bone_transforms(b"RopeEnd", MAX_BONES);
    let mut num_ropes = d.num_ropes as i32;
    num_ropes = num_ropes.min(rope_pos.len() as i32).min(drop_mtx.len() as i32);
    if num_ropes <= 0 {
        return Ok(StateReturn::FAILURE);
    }
    let mut ropes = Vec::new();
    for i in 0..num_ropes as usize {
        let mut info = RopeInfo { drop_start_mtx: h.own_convert_bone_transform_to_world(&drop_mtx[i]), ..Default::default() };
        info.rope_drawable = if d.rope_template { h.new_rope_drawable() } else { None };
        if let Some(rope) = info.rope_drawable {
            let pos = h.own_convert_bone_pos_to_world_pos(&rope_pos[i]);
            h.rope_call(rope, RopeCall::SetPosition(pos));
            let layer_at_dest = h.highest_layer_for_healthy_destination(&pos);
            let rope_len_max = pos.z - h.layer_height(pos.x, pos.y, layer_at_dest) - d.rope_final_height;
            info.rope = Some((0.0, 1.0, rope_len_max));
            h.rope_call(
                rope,
                RopeCall::InitParms {
                    length: rope_len_max,
                    width: d.rope_width,
                    color: d.rope_color,
                    wobble_len: d.rope_wobble_len,
                    wobble_amp: d.rope_wobble_amp,
                    wobble_rate: d.rope_wobble_rate,
                },
            );
        }
        let delay = h.logic_random_value(d.per_rope_delay_min as i32, d.per_rope_delay_max as i32) as u32;
        info.next_drop_time = now.wrapping_add(delay).wrapping_sub(d.per_rope_delay_min);
        ropes.push(info);
    }
    if let Some(AiState::ChinookCombatDrop { ropes: r }) = sm.state_mut(id) {
        *r = ropes;
    }
    Ok(StateReturn::CONTINUE)
}

/// The combat drop state's `update`: dead, a failure; the done rappellers (gone, dead, or on the ground) dropped; each rope
/// lengthened while short (waiting for it to drop, its next drop later), and at its drop's frame the next rappeller exited onto it
/// (placed at its start, its AI at the rappel speed rappelling into the goal); done once no rope is in use and none can rappel.
pub fn combat_drop_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if h.is_effectively_dead() {
        return Ok(StateReturn::FAILURE);
    }
    let d = ck(h).data;
    let mut ropes = match sm.state_mut(id) {
        Some(AiState::ChinookCombatDrop { ropes }) => std::mem::take(ropes),
        _ => unreachable!("the combat drop state"),
    };
    let r = combat_drop_ropes(sm, h, &d, &mut ropes);
    if let Some(AiState::ChinookCombatDrop { ropes: r }) = sm.state_mut(id) {
        *r = ropes;
    }
    r
}

/// [`combat_drop_update`]'s ropes (the state's, taken out).
fn combat_drop_ropes<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, d: &ChinookData, ropes: &mut [RopeInfo]) -> SmResult<StateReturn> {
    // "first, eliminate "done" rappellers"
    for rope in ropes.iter_mut() {
        rope.rappeller_ids.retain(|&r| h.object_exists(r) && !h.object_is_effectively_dead(r) && h.object_is_above_terrain(r));
    }
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    let gravity = crate::logic::locomotor::LocomotorGlobals::gravity(h);
    let mut num_ropes_in_use = 0;
    for rope in ropes.iter_mut() {
        let Some((speed, len, len_max)) = rope.rope.as_mut() else {
            return Err(StateMachineError::Undefined("a chinook's rope without a drawable, its lengths uninitialised"));
        };
        if *len < *len_max {
            *speed += gravity.abs();
            if *speed > d.rope_drop_speed {
                *speed = d.rope_drop_speed;
            }
            *len += *speed;
            let cur_len = *len;
            if let Some(drawable) = rope.rope_drawable {
                h.rope_call(drawable, RopeCall::SetCurLen(cur_len));
            }
            if d.wait_for_ropes_to_drop {
                // "can't use this rope till it's dropped all the way"
                rope.next_drop_time = rope.next_drop_time.wrapping_add(1);
                continue;
            }
        }
        if now >= rope.next_drop_time && !plant(3753) {
            let rappeller = potential_rappeller(h);
            if rappeller != INVALID_ID {
                let door = h.own_exit_reserve_door(rappeller);
                if door != DOOR_NONE_AVAILABLE {
                    h.own_exit_object_via_door(rappeller, door);
                }
                // (else "rappeller is not free to exit... what?": a DEBUG_CRASH)
                h.object_set_transform_matrix(rappeller, &rope.drop_start_mtx);
                if h.object_has_ai(rappeller) {
                    h.member_ai_command(rappeller, MemberCommand::SetDesiredSpeed(d.rappel_speed));
                    let target = sm.goal_object(h).map_or(INVALID_ID, |o| o.id);
                    let pos = *sm.goal_position();
                    h.member_ai_command(rappeller, MemberCommand::RappelInto { target, pos, src: CMD_FROM_AI });
                }
                rope.rappeller_ids.push(rappeller);
                let delay = h.logic_random_value(d.per_rope_delay_min as i32, d.per_rope_delay_max as i32) as u32;
                rope.next_drop_time = now.wrapping_add(delay);
            }
        }
        if !rope.rappeller_ids.is_empty() {
            num_ropes_in_use += 1;
        }
    }
    if num_ropes_in_use == 0 && potential_rappeller(h) == INVALID_ID {
        // "we're done!"
        return Ok(StateReturn::SUCCESS);
    }
    Ok(StateReturn::CONTINUE)
}

/// The combat drop state's `onExit`: no longer held, flying; dead, its rappellers idle ("oops. drop the rangers."); each rope's
/// drawable given a kick and five seconds to expire; the ropes forgotten.
pub fn combat_drop_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let d = ck(h).data;
    h.set_disabled_held(false);
    ck(h).flight_status = CHINOOK_FLYING;
    let ropes = match sm.state_mut(id) {
        Some(AiState::ChinookCombatDrop { ropes }) => std::mem::take(ropes),
        _ => unreachable!("the combat drop state"),
    };
    if h.is_effectively_dead() {
        for rope in &ropes {
            for &r in &rope.rappeller_ids {
                if h.object_exists(r) && h.object_has_ai(r) {
                    h.member_ai_command(r, MemberCommand::Idle { src: CMD_FROM_AI });
                }
            }
        }
    }
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    let gravity = crate::logic::locomotor::LocomotorGlobals::gravity(h);
    for rope in &ropes {
        if let Some(drawable) = rope.rope_drawable {
            const ROPE_EXPIRATION_TIME: u32 = LOGICFRAMES_PER_SECOND as u32 * 5;
            // "give it a little kick"
            let initial_speed = gravity * 30.0;
            h.rope_call(drawable, RopeCall::SetSpeed { cur: initial_speed, max: d.rope_drop_speed, accel: gravity });
            h.rope_call(drawable, RopeCall::SetExpirationDate(now.wrapping_add(ROPE_EXPIRATION_TIME)));
        }
    }
}

/// The combat drop state's `xfer` (version 2): the rope count, then each rope (its drawable's id (`INVALID_DRAWABLE_ID` none),
/// start, speed and lengths, next drop, rappellers). A load into ropes is refused (`SC_INVALID_DATA`: `ErrorUnknown`, as the
/// other states'); a loaded rope's drawable is
/// found again by `loadPostProcess`, here the id kept.
pub fn combat_drop_xfer(ropes: &mut Vec<RopeInfo>, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 2;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    let mut num_ropes = ropes.len() as i32;
    xfer.xfer_int(&mut num_ropes)?;
    if version >= 2 {
        let load = xfer.get_xfer_mode() == XferMode::Load;
        if load {
            if !ropes.is_empty() {
                // "ChinookCombatDropState - ropes should be empty"
                return Err(crate::common::xfer::XferStatus::ErrorUnknown);
            }
            ropes.resize(num_ropes.max(0) as usize, RopeInfo::default());
        }
        for rope in ropes.iter_mut() {
            let mut rope_id = rope.rope_drawable.map_or(0, |d| d as i32);
            xfer.xfer_drawable_id(&mut rope_id)?;
            xfer.xfer_matrix3d(&mut rope.drop_start_mtx)?;
            let (mut speed, mut len, mut len_max) = rope.rope.unwrap_or_default();
            xfer.xfer_real(&mut speed)?;
            xfer.xfer_real(&mut len)?;
            xfer.xfer_real(&mut len_max)?;
            xfer.xfer_unsigned_int(&mut rope.next_drop_time)?;
            xfer.xfer_stl_object_id_list(&mut rope.rappeller_ids)?;
            if load {
                rope.rope_drawable = (rope_id != 0).then_some(rope_id as u32);
                rope.rope = Some((speed, len, len_max));
            }
        }
    }
    Ok(())
}

/// `isIdle()` (the chinook's): never with a command waiting; landed, not while passengers want in or out.
pub fn chinook_is_idle<H: crate::logic::ai::ai_update::AiHost>(h: &mut H, c_pending: bool, flight_status: i32, base_idle: bool) -> bool {
    if c_pending && !plant(3754) {
        return false;
    }
    let mut result = base_idle;
    if result && flight_status == CHINOOK_LANDED && h.contain_has_objects_wanting_to_enter_or_exit() == Some(true) {
        // "if we are waiting to disgorge some folks, we aren't 'idle'"
        result = false;
    }
    result
}

/// `isAvailableForSupplying()` (the chinook's): not with passengers (wanting in or out, or aboard), nor in an overlord-style
/// container, nor without a contain.
pub fn is_available_for_supplying<H: AiStateHost>(h: &mut H) -> bool {
    let Some(wanting) = h.contain_has_objects_wanting_to_enter_or_exit() else { return false };
    !(wanting || h.contain_count() != 0 || h.contain_is_special_overlord_style_container())
}

/// `isAllowedToAdjustDestination()` (the chinook's): not landed, nor when its locomotor may leave the valid positions (the
/// locomotor unchecked: none, undefined).
pub fn is_allowed_to_adjust_destination<H: AiStateHost>(h: &mut H) -> SmResult<bool> {
    let landed = ck(h).flight_status == CHINOOK_LANDED;
    if landed && !plant(3755) {
        return Ok(false);
    }
    let Some(l) = ai(h).get_cur_locomotor() else {
        return Err(StateMachineError::Undefined("a chinook's destination adjusted without a locomotor"));
    };
    Ok(!l.is_invalid_position_allowed())
}

/// `getBuildingToNotPathAround()` (the chinook's): moving to or doing a combat drop, its goal object.
pub fn building_to_not_path_around<H: AiStateHost>(sm: &Sm<H>, h: &mut H) -> ObjectId {
    let state = sm.get_current_state_id();
    if state == MOVE_TO_COMBAT_DROP || state == DO_COMBAT_DROP {
        return sm.goal_object(h).map_or(INVALID_ID, |o| o.id);
    }
    INVALID_ID
}

/// `getAiFreeToExit(exiter)` (the chinook's): landed, or dropping a rappeller: free; else wait.
pub fn ai_free_to_exit<H: AiStateHost>(h: &mut H, exiter: ObjectId) -> i32 {
    let status = ck(h).flight_status;
    if status == CHINOOK_LANDED || (status == CHINOOK_DOING_COMBAT_DROP && h.object_is_kind_of(exiter, KINDOF_CAN_RAPPEL)) {
        return crate::logic::modules::transport_contain::FREE_TO_EXIT;
    }
    crate::logic::ai::ai_face_exit_states::WAIT_TO_EXIT
}

/// `chooseLocomotorSet(wst)`'s override: landed, the taxiing set.
pub fn locomotor_set_for(c: &Chinook, wst: LocomotorSetType) -> LocomotorSetType {
    if c.flight_status == CHINOOK_LANDED {
        return LOCOMOTORSET_TAXIING;
    }
    wst
}

/// `getUpgradedSupplyBoost()`: the boost once the player has the supply lines.
pub fn upgraded_supply_boost<H: AiStateHost>(h: &mut H) -> i32 {
    if h.player_has_upgrade_complete("Upgrade_AmericaSupplyLines") {
        return ck(h).data.upgraded_supply_boost;
    }
    0
}

/// `getPP(id)`: an airfield (alive, `KINDOF_FS_AIRFIELD`) with a parking place.
fn airfield_has_parking_place<H: AiStateHost>(h: &mut H, id: ObjectId) -> bool {
    if !h.object_exists(id) || h.object_is_effectively_dead(id) || !h.object_is_kind_of(id, KINDOF_FS_AIRFIELD) {
        return false;
    }
    h.parking_place_exists(id)
}

/// `setAirfieldForHealing(id)`: the current one told to stop healing us (if another), then the new one kept.
fn set_airfield_for_healing<H: AiStateHost>(h: &mut H, id: ObjectId) {
    let current = ck(h).airfield_for_healing;
    if current != INVALID_ID && current != id && airfield_has_parking_place(h, current) {
        h.parking_place_set_healee(current, false);
    }
    ck(h).airfield_for_healing = id;
}

/// `setMyState(cmd, target, pos, cmdSource)`: the machine cleared, its goals and the command source set, then the state.
fn set_my_state<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, cmd: StateId, target: ObjectId, pos: Option<&Coord3D>, src: CommandSourceType) -> SmResult<()> {
    crate::logic::ai::ai_states::aism_clear(sm, h)?;
    commands::set_goal_object(sm, h, target);
    // "yeah, null is ok here."
    commands::set_goal_position_clipped(sm, h, pos, src);
    commands::set_last_command_source(h, src);
    crate::logic::ai::ai_states::aism_set_state(sm, h, cmd).map(|_| ())
}

/// `update()`'s own part, before the supply truck's (the AI and its machine lent): the airfield healing us (landed and healed
/// with nothing waiting: off again); idle by the base's measure, a waiting command given, or a landing for passengers wanting in
/// or out, or (no longer wanted on the ground) a take off; every tenth frame the passengers allowed to fire attack our victim;
/// the client's rotor wash near the ground.
pub fn chinook_pre_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    let airfield = ck(h).airfield_for_healing;
    if airfield_has_parking_place(h, airfield) {
        let landed = ck(h).flight_status == CHINOOK_LANDED;
        let healed = h.body_health() == h.body_max_health();
        if landed && !ck(h).has_pending_command && healed && !plant(3756) {
            // "we're completely healed, so take off again"
            h.parking_place_set_healee(airfield, false);
            set_my_state(sm, h, TAKING_OFF, INVALID_ID, None, CMD_FROM_AI)?;
        } else {
            h.parking_place_set_healee(airfield, landed);
        }
    } else {
        set_airfield_for_healing(h, INVALID_ID);
    }
    // "have to call our parent's isIdle, because we override it to never return true when we have a pending command..."
    if h.has_contain() {
        if ai(h).base_is_idle(&*sm) {
            let waiting_to_enter_or_exit = h.contain_has_objects_wanting_to_enter_or_exit() == Some(true);
            let status = ck(h).flight_status;
            if ck(h).has_pending_command {
                // "values don't matter, will be wiped by next line"
                let mut parms = AiCommandParms::new(AICMD_MOVE_TO_POSITION, CMD_FROM_AI);
                let storage = ck(h).pending_command.clone();
                storage.reconstitute(h, &mut parms);
                ck(h).has_pending_command = false;
                commands::ai_do_command(sm, h, &parms)?;
            } else if waiting_to_enter_or_exit && status != CHINOOK_LANDED {
                set_my_state(sm, h, LANDING, INVALID_ID, None, CMD_FROM_AI)?;
            } else if !waiting_to_enter_or_exit && status == CHINOOK_LANDED && ck(h).airfield_for_healing == INVALID_ID && !plant(3757) {
                set_my_state(sm, h, TAKING_OFF, INVALID_ID, None, CMD_FROM_AI)?;
            }
        }
        if crate::logic::locomotor::LocomotorHost::frame(h) % 10 == 1 {
            let victim = ai(h).current_victim_id;
            if h.object_exists(victim) && h.contain_passenger_allowed_to_fire() {
                let me = h.object_id();
                for passenger in h.contain_items(me) {
                    if h.object_has_ai(passenger) && h.member_ai_current_victim(passenger) == INVALID_ID {
                        h.member_ai_command(passenger, MemberCommand::AttackObject { victim, max_shots: 999, src: CMD_FROM_AI });
                    }
                }
            }
        }
    }
    // "Just a handy spot to handle that groovy client effect of the rotor wash"
    if h.local_player_shroud_clear() {
        let status = ck(h).flight_status;
        if status == CHINOOK_LANDING || status == CHINOOK_TAKING_OFF || status == CHINOOK_LANDED {
            let mut pos = h.position();
            let mut chopper_elevation = pos.z;
            pos.z = h.ground_height(pos.x, pos.y) + 3.0;
            chopper_elevation -= pos.z;
            if h.client_random_value_real(0.0, chopper_elevation) < 5.0 && ck(h).data.rotor_wash {
                h.rotor_wash(&pos);
            }
        }
    }
    Ok(())
}

/// `privateGetRepaired(depot, cmdSource)` (the chinook's): not while landing or landed; if it may, the depot healing us, and a
/// move to a clear spot around it to land.
fn private_get_repaired<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, depot: ObjectId, src: CommandSourceType) -> SmResult<()> {
    let status = ck(h).flight_status;
    // "we are already landing. just ignore it."
    if status == CHINOOK_LANDING || status == CHINOOK_LANDED {
        return Ok(());
    }
    // "sanity, if we can't get repaired from here get out of here" (upstream's ActionManager refuses no depot; a host that
    // answers yes for none leaves the depot used unchecked: undefined)
    if !h.action_can_get_repaired_at(depot, src) {
        return Ok(());
    }
    if depot == INVALID_ID {
        return Err(StateMachineError::Undefined("a chinook repaired at no depot"));
    }
    set_airfield_for_healing(h, depot);
    let mut pos = h.object_position(depot);
    let max_radius = h.object_bounding_circle_radius(depot) * 100.0;
    if let Some(tmp) = h.find_position_around_no_source(&pos, 0.0, max_radius) {
        pos = tmp;
    }
    set_my_state(sm, h, MOVE_TO_AND_LAND, INVALID_ID, Some(&pos), src)
}

/// `privateCombatDrop(target, pos, cmdSource)` (the chinook's): a player's drop into what it may not enter refused; at a position,
/// a clear spot around it; then the move to drop.
fn private_combat_drop<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, target: ObjectId, pos: &Coord3D, src: CommandSourceType) -> SmResult<()> {
    if target != INVALID_ID && src == CMD_FROM_PLAYER && !h.action_can_enter_object_combat_drop(target, src) {
        return Ok(());
    }
    let mut local_pos = *pos;
    if target == INVALID_ID {
        // "if target is null, we are dropping at a pos, not into a bldg."
        let max_radius = h.bounding_circle_radius() * 100.0;
        if let Some(tmp) = h.find_position_around_no_source(&local_pos, 0.0, max_radius) {
            local_pos = tmp;
        }
    }
    // "start the combat drop process"
    set_my_state(sm, h, MOVE_TO_COMBAT_DROP, target, Some(&local_pos), src)
}

/// `aiDoCommand(parms)` (the chinook's): the airfield forgotten; if allowed, a command while taking off, landing or dropping
/// waits; idle, busy and the exit production path pass; a move to evacuate (from far, landed: take off first, the command
/// waiting) is its own; an exit or evacuation lands first (the command waiting); any other takes off first unless flying; else the
/// supply truck's (the base's, with its privateIdle and privateDock).
pub fn chinook_ai_do_command<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, parms: &AiCommandParms) -> SmResult<()> {
    // "this gets reset every time a command is issued."
    set_airfield_for_healing(h, INVALID_ID);
    if !commands::is_allowed_to_respond_to_ai_commands(sm, h, parms) {
        return Ok(());
    }
    let status = ck(h).flight_status;
    if status == CHINOOK_TAKING_OFF || status == CHINOOK_LANDING || status == CHINOOK_DOING_COMBAT_DROP {
        // "have to wait for takeoff or landing (or rappel) to complete, just store the sucker."
        store_pending(h, parms);
        return Ok(());
    }
    let mut pass_it_thru = true;
    match parms.cmd {
        // "don't need (or want) to take off for these..."
        AICMD_IDLE | AICMD_BUSY | AICMD_FOLLOW_EXITPRODUCTION_PATH => {}
        AICMD_MOVE_TO_POSITION_AND_EVACUATE | AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT => {
            const THRESH: f32 = 3.0;
            if calc_dist_sqr(&h.position(), &parms.pos) > THRESH * THRESH && status == CHINOOK_LANDED {
                // "gotta take off first!"
                store_pending(h, parms);
                set_my_state(sm, h, TAKING_OFF, INVALID_ID, None, CMD_FROM_AI)?;
            } else {
                // "do this INSTEAD of the standard stuff"
                let state = if parms.cmd == AICMD_MOVE_TO_POSITION_AND_EVACUATE { MOVE_TO_AND_EVAC } else { MOVE_TO_AND_EVAC_AND_EXIT_INIT };
                set_my_state(sm, h, state, INVALID_ID, Some(&parms.pos), CMD_FROM_AI)?;
            }
            pass_it_thru = false;
        }
        AICMD_EXIT | AICMD_EVACUATE => {
            if status != CHINOOK_LANDED && !plant(3758) {
                // "gotta land first!"
                store_pending(h, parms);
                set_my_state(sm, h, LANDING, INVALID_ID, None, CMD_FROM_AI)?;
                pass_it_thru = false;
            }
        }
        _ => {
            if status != CHINOOK_FLYING {
                // "gotta take off first!"
                store_pending(h, parms);
                set_my_state(sm, h, TAKING_OFF, INVALID_ID, None, CMD_FROM_AI)?;
                pass_it_thru = false;
            }
        }
    }
    if pass_it_thru {
        ck(h).has_pending_command = false;
        commands::base_ai_do_command(sm, h, parms)?;
    }
    Ok(())
}

/// `m_pendingCommand.store(*parms); m_hasPendingCommand = true;`.
fn store_pending<H: AiStateHost>(h: &mut H, parms: &AiCommandParms) {
    let mut storage = ck(h).pending_command.clone();
    storage.store(h, parms);
    let c = ck(h);
    c.pending_command = storage;
    c.has_pending_command = true;
}

/// The chinook's command members the base's switch reaches (`privateGetRepaired`, `privateCombatDrop`): `None` when the
/// command is not one of them.
pub fn chinook_private_command<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, parms: &AiCommandParms) -> Option<SmResult<()>> {
    match parms.cmd {
        crate::logic::ai::ai_commands::AICMD_GET_REPAIRED => {
            let depot = if h.object_exists(parms.obj) { parms.obj } else { INVALID_ID };
            Some(private_get_repaired(sm, h, depot, parms.cmd_source))
        }
        crate::logic::ai::ai_commands::AICMD_COMBATDROP => {
            let target = if h.object_exists(parms.obj) { parms.obj } else { INVALID_ID };
            Some(private_combat_drop(sm, h, target, &parms.pos, parms.cmd_source))
        }
        _ => None,
    }
}

/// `privateIdle`'s own part (the chinook's): the contain's rider told to idle first.
pub fn rider_idle<H: AiStateHost>(h: &mut H, src: CommandSourceType) {
    if !h.has_contain() {
        return;
    }
    let rider = h.contain_rider();
    if rider != INVALID_ID && h.object_has_ai(rider) {
        h.member_ai_command(rider, MemberCommand::Idle { src });
    }
}

/// Which attack a chinook's passengers (and rider) are told to make.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChinookAttack {
    /// `privateAttackObject`.
    Object { victim: ObjectId, max_shots: i32 },
    /// `privateForceAttackObject`.
    ForceObject { victim: ObjectId, max_shots: i32 },
    /// `privateAttackPosition`.
    Position { pos: Coord3D, max_shots: i32 },
}

/// A passenger's or the rider's command for the attack.
fn member_attack(attack: ChinookAttack, src: CommandSourceType) -> MemberCommand {
    match attack {
        ChinookAttack::Object { victim, max_shots } => MemberCommand::AttackObject { victim, max_shots, src },
        ChinookAttack::ForceObject { victim, max_shots } => MemberCommand::ForceAttackObject { victim, max_shots, src },
        ChinookAttack::Position { pos, max_shots } => MemberCommand::AttackPosition { pos, max_shots, src },
    }
}

/// Whether an object is disabled by hacking, EMP, subdual or paralysis.
fn disabled_for_attack<H: AiStateHost>(h: &mut H, o: ObjectId) -> bool {
    [DISABLED_HACKED, DISABLED_EMP, DISABLED_SUBDUED, DISABLED_PARALYZED].iter().any(|&t| h.object_is_disabled_by_type(o, t))
}

/// The chinook's attacks' own part, before the base's: false when it cannot attack (the base's then not reached); a player's or
/// a script's attack first given to each passenger allowed to fire that is infantry (a disabled portable structure not), then to
/// a portable structure riding (not disabled).
pub fn chinook_attack<H: AiStateHost>(h: &mut H, attack: ChinookAttack, src: CommandSourceType) -> bool {
    if !h.is_kind_of(KINDOF_CAN_ATTACK) && !plant(3759) {
        return false;
    }
    if !h.has_contain() {
        return true;
    }
    // "I may want to tell my passengers to attack too, but only if this is a direct command."
    if src == CMD_FROM_PLAYER || src == CMD_FROM_SCRIPT {
        let me = h.object_id();
        for passenger in h.contain_items(me) {
            if !h.contain_passenger_allowed_to_fire_id(passenger) {
                continue;
            }
            if !h.object_is_kind_of(passenger, KINDOF_INFANTRY) {
                continue;
            }
            // "If I am an overlord with a gattling upgrade, I do not tell it to fire if it is disabled"
            if h.object_is_kind_of(passenger, KINDOF_PORTABLE_STRUCTURE) && disabled_for_attack(h, passenger) {
                continue;
            }
            if h.object_has_ai(passenger) {
                h.member_ai_command(passenger, member_attack(attack, src));
            }
        }
        // "THE GATTLING UPGRADE OR THE GUYS IN THE BUNKER_NOT_A_BUNKER"
        let rider = h.contain_rider();
        if rider != INVALID_ID && h.object_is_kind_of(rider, KINDOF_PORTABLE_STRUCTURE) && !disabled_for_attack(h, rider) && h.object_has_ai(rider) {
            h.member_ai_command(rider, member_attack(attack, src));
        }
    }
    true
}

/// `ChinookAIUpdate::xfer`'s own members, after the supply truck's (version 2): the waiting command (if any), the flight status
/// (`xferUser`, 4 bytes), the airfield, the original position (version 2).
pub fn chinook_xfer_members<H: AiStateHost>(c: &mut Chinook, h: &mut H, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
    xfer.xfer_bool(&mut c.has_pending_command)?;
    if c.has_pending_command {
        c.pending_command.do_xfer(h, xfer)?;
    }
    let mut b = c.flight_status.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    c.flight_status = i32::from_le_bytes(b);
    xfer.xfer_object_id(&mut c.airfield_for_healing)?;
    if version >= 2 {
        xfer.xfer_coord3d(&mut c.original_pos)?;
    }
    Ok(())
}

/// The chinook's state's snapshot (`xfer`): taking off or landing (version 1: the destination, landing); moving to drop (version
/// 1: its three heights); the drop ([`combat_drop_xfer`]); recording (version 1); evacuating and heading off: nothing.
pub fn chinook_state_xfer<H: AiStateHost>(s: &mut AiState<H>, xfer: &mut dyn Xfer) -> XferResult<()> {
    let v1 = |xfer: &mut dyn Xfer| -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
    };
    match s {
        AiState::ChinookTakeoffOrLanding { dest_loc, landing } => {
            v1(xfer)?;
            xfer.xfer_coord3d(dest_loc)?;
            xfer.xfer_bool(landing)
        }
        AiState::ChinookMoveToBldg { old_preferred_height, new_preferred_height, dest_z, .. } => {
            v1(xfer)?;
            xfer.xfer_real(old_preferred_height)?;
            xfer.xfer_real(new_preferred_height)?;
            xfer.xfer_real(dest_z)
        }
        AiState::ChinookCombatDrop { ropes } => combat_drop_xfer(ropes, xfer),
        AiState::ChinookRecordCreation => v1(xfer),
        _ => Ok(()),
    }
}

/// The set the chinook's `chooseLocomotorSet` would choose (for `AiUpdate::choose_locomotor_set`).
pub fn chinook_locomotor_set(subclass: &AiSubclass, wst: LocomotorSetType) -> LocomotorSetType {
    match subclass {
        AiSubclass::Chinook(c) => locomotor_set_for(c, wst),
        _ => wst,
    }
}

