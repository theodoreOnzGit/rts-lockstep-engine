// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/JetAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/JetAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `JetAIUpdate` (`p1w13-sonnet`, layer 10, batch A41): the aircraft that lives at an airfield (or a helipad). Its AI's machine has
//! thirteen more states ([`define_jet_states`]) for a jet that needs a runway (`JetAIStateMachine`): taxiing from the hangar and
//! to take off, waiting for a runway, the pause before taking off, taking off, landing, taxiing to its parking place and turning to
//! it, reloading, returning to land, and (its airfield gone) returning to where it was and circling there; ten for one that does
//! not (`HeliAIStateMachine`: a straight climb or descent instead of the runway). Its flags ([`Jet::flags`]) say whether it may
//! fly, whether it is taking off, landing or taxiing, and whether a command waits for that to end ([`Jet::most_recent_command`]).
//! Out of the ammunition that returns to base, it returns to reload (after a guard or hunt, it resumes it).
//!
//! The members are [`Jet`], held in [`AiSubclass::Jet`]; the parking place is the producer's ([`get_pp`]), reached through the
//! host's `pp_*` calls.
//!
//! # Upstream facts kept
//!
//! - `AIFollowPathState::onEnter` sets the final adjustment again, so the take-off and landing state's
//!   `setAdjustFinalDestination(false)` before it changes nothing; likewise `AIMoveOutOfTheWayState::onEnter` makes the taxi state
//!   adjust its destination after `setAdjustsDestination(false)`.
//! - `JetPauseBeforeTakeoffState::xfer` saves its own members only, not `AIFaceState`'s.
//! - The taxi to parking on a carrier appends the better parking assignment's position whether or not one was found (none: the
//!   position uninitialised, undefined upstream; refused here).
//! - The take-off state's update asks its parking place's `calcPPInfo` unchecked (none: undefined; refused here).
//! - The states use the current locomotor unchecked where upstream does (none: undefined; refused here).
//! - `m_lockonBlinky` is not set by the module data's constructor: the pool's zero (false).
//! - Another jet waiting to taxi to take off is asked by its AI's state id: in the differential test, the other objects' AIs never
//!   are in that state.

use crate::common::base_type::{Coord3D, PI};
use crate::common::game_common::{std_angle_diff, LOGICFRAMES_PER_SECOND, SECONDS_PER_LOGICFRAME_REAL};
use crate::common::game_type::{ObjectId, INVALID_ID, LAYER_GROUND};
use crate::common::kind_of::KINDOF_PRODUCED_AT_HELIPAD;
use crate::common::model_state::{MODELCONDITION_JETAFTERBURNER, MODELCONDITION_JETEXHAUST};
use crate::common::object_status_types::{
    OBJECT_STATUS_DECK_HEIGHT_OFFSET, OBJECT_STATUS_IS_ATTACKING, OBJECT_STATUS_REASSIGN_PARKING, OBJECT_STATUS_SOLD,
};
use crate::common::state_machine::{SmResult, StateExitType, StateId, StateMachine, StateMachineError, StateReturn, UtilityState};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_commands::{
    self as commands, AiCommandParms, AICMD_BUSY, AICMD_ENTER, AICMD_FOLLOW_EXITPRODUCTION_PATH, AICMD_FOLLOW_PATH,
    AICMD_GET_REPAIRED, AICMD_GUARD_AREA, AICMD_GUARD_OBJECT, AICMD_GUARD_POSITION, AICMD_GUARD_RETALIATE, AICMD_HUNT, AICMD_IDLE,
    AICMD_MOVE_TO_POSITION, AICMD_NO_COMMAND,
};
use crate::logic::ai::ai_move_states::{self as move_states, MoveToData};
use crate::logic::ai::ai_states::{ai, aism_clear, aism_set_state, with_ai, AiState, AiStateHost};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::{CommandSourceType, LocomotorSetType, AI_IDLE, CMD_FROM_AI, LOCOMOTORSET_NORMAL, LOCOMOTORSET_TAXIING};
use crate::logic::ai::hack_internet_ai_update::AiCommandParmsStorage;
use crate::logic::ai::path::Path;
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::modules::plant;
use crate::common::game_type::WeaponSlotType;
use crate::logic::weapon_types::WeaponStatus;

type Sm<H> = StateMachine<AiState<H>>;

/// The weapon slots (`WEAPONSLOT_COUNT`).
const SLOTS: [WeaponSlotType; 3] = [WeaponSlotType::Primary, WeaponSlotType::Secondary, WeaponSlotType::Tertiary];

/// `BIGNUM` (the file's, Locomotor.cpp's value).
const BIGNUM: f32 = 99999.0;

/// `TaxiType`.
pub const FROM_HANGAR: i32 = 0;
pub const FROM_PARKING: i32 = 1;
pub const TO_PARKING: i32 = 2;

/// `JetAIStateType` (after `JETAISTATETYPE_FIRST = 1000`).
pub const TAXI_FROM_HANGAR: StateId = 1001;
pub const TAKING_OFF_AWAIT_CLEARANCE: StateId = 1002;
pub const TAXI_TO_TAKEOFF: StateId = 1003;
pub const PAUSE_BEFORE_TAKEOFF: StateId = 1004;
pub const TAKING_OFF: StateId = 1005;
pub const LANDING_AWAIT_CLEARANCE: StateId = 1006;
pub const LANDING: StateId = 1007;
pub const TAXI_FROM_LANDING: StateId = 1008;
pub const ORIENT_FOR_PARKING_PLACE: StateId = 1009;
pub const RELOAD_AMMO: StateId = 1010;
pub const RETURNING_FOR_LANDING: StateId = 1011;
pub const RETURN_TO_DEAD_AIRFIELD: StateId = 1012;
pub const CIRCLING_DEAD_AIRFIELD: StateId = 1013;
/// `JETAISTATETYPE_FIRST`, `JETAISTATETYPE_LAST`.
const JETAISTATETYPE_FIRST: StateId = 1000;
const JETAISTATETYPE_LAST: StateId = 1014;

/// `FlagType`: the bits of [`Jet::flags`].
pub const HAS_PENDING_COMMAND: u32 = 0;
pub const ALLOW_AIR_LOCO: u32 = 1;
pub const HAS_PRODUCER_LOCATION: u32 = 2;
pub const TAKEOFF_IN_PROGRESS: u32 = 3;
pub const LANDING_IN_PROGRESS: u32 = 4;
pub const USE_SPECIAL_RETURN_LOCO: u32 = 5;
pub const ALLOW_CIRCLING: u32 = 6;
pub const ALLOW_INTERRUPT_AND_RESUME_OF_CUR_STATE_FOR_RELOAD: u32 = 7;
pub const TAXI_IN_PROGRESS: u32 = 8;

/// `JetOrHeliCirclingDeadAirfieldState`'s `HOW_OFTEN_TO_CHECK`: "only recheck for new airfields every second or so".
const HOW_OFTEN_TO_CHECK: i32 = LOGICFRAMES_PER_SECOND;

/// `ParkingPlaceBehaviorInterface::PPInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PpInfo {
    pub parking_space: Coord3D,
    pub parking_orientation: f32,
    pub runway_prep: Coord3D,
    pub runway_start: Coord3D,
    pub runway_end: Coord3D,
    pub runway_exit: Coord3D,
    pub runway_landing_start: Coord3D,
    pub runway_landing_end: Coord3D,
    pub runway_approach: Coord3D,
    pub hangar_internal: Coord3D,
    pub runway_takeoff_dist: f32,
    pub hangar_internal_orient: f32,
}

/// `JetAIUpdateModuleData`'s own fields (the field parse's names in the docs).
#[derive(Clone, Debug, PartialEq)]
pub struct JetData {
    /// `OutOfAmmoDamagePerSecond` (a percentage of the maximum health, per second).
    pub out_of_ammo_damage_per_second: f32,
    /// `NeedsRunway` (default true).
    pub needs_runway: bool,
    /// `KeepsParkingSpaceWhenAirborne` (default true).
    pub keeps_parking_space_when_airborne: bool,
    /// `TakeoffDistForMaxLift`.
    pub takeoff_dist_for_max_lift: f32,
    /// `MinHeight`.
    pub min_height: f32,
    /// `ParkingOffset`.
    pub parking_offset: f32,
    /// `SneakyOffsetWhenAttacking`.
    pub sneaky_offset_when_attacking: f32,
    /// `TakeoffPause` (frames).
    pub takeoff_pause: u32,
    /// `AttackLocomotorType` (default the normal set).
    pub attacking_loco: LocomotorSetType,
    /// `ReturnForAmmoLocomotorType` (default the normal set).
    pub returning_loco: LocomotorSetType,
    /// `AttackLocomotorPersistTime` (frames).
    pub attack_loco_persist_time: u32,
    /// `AttackersMissPersistTime` (frames).
    pub attackers_miss_persist_time: u32,
    /// `LockonTime` (frames).
    pub lockon_time: u32,
    /// `LockonCursor` names a template (`TheThingFactory->findTemplate`; default none).
    pub lockon_cursor: bool,
    /// `LockonInitialDist` (default 100).
    pub lockon_initial_dist: f32,
    /// `LockonFreq` (default 0.5).
    pub lockon_freq: f32,
    /// `LockonAngleSpin` (default 720).
    pub lockon_angle_spin: f32,
    /// `LockonBlinky` (the constructor leaves it to the pool's zero).
    pub lockon_blinky: bool,
    /// `ReturnToBaseIdleTime` (frames).
    pub return_to_base_idle_time: u32,
}

impl Default for JetData {
    /// `JetAIUpdateModuleData()`.
    fn default() -> Self {
        JetData {
            out_of_ammo_damage_per_second: 0.0,
            needs_runway: true,
            keeps_parking_space_when_airborne: true,
            takeoff_dist_for_max_lift: 0.0,
            min_height: 0.0,
            parking_offset: 0.0,
            sneaky_offset_when_attacking: 0.0,
            takeoff_pause: 0,
            attacking_loco: LOCOMOTORSET_NORMAL,
            returning_loco: LOCOMOTORSET_NORMAL,
            attack_loco_persist_time: 0,
            attackers_miss_persist_time: 0,
            lockon_time: 0,
            lockon_cursor: false,
            lockon_initial_dist: 100.0,
            lockon_freq: 0.5,
            lockon_angle_spin: 720.0,
            lockon_blinky: false,
            return_to_base_idle_time: 0,
        }
    }
}

/// `JetAIUpdate`'s members, with its module data.
#[derive(Clone, Debug, PartialEq)]
pub struct Jet {
    /// The module data's own fields.
    pub data: JetData,
    /// `m_producerLocation`: "if our producer dies, we have a place to circle aimlessly".
    pub producer_location: Coord3D,
    /// `m_mostRecentCommand`.
    pub most_recent_command: AiCommandParmsStorage,
    /// `m_afterburnerSound`'s playing handle (the sound itself the template's "Afterburner": unnamed here).
    pub afterburner_handle: u32,
    /// `m_attackLocoExpireFrame`.
    pub attack_loco_expire_frame: u32,
    /// `m_attackersMissExpireFrame`.
    pub attackers_miss_expire_frame: u32,
    /// `m_returnToBaseFrame`: "if nonzero, return to base at this frame when we are idle, even if not out of ammo".
    pub return_to_base_frame: u32,
    /// `m_targetedBy`: "object(s) currently targeting us".
    pub targeted_by: Vec<ObjectId>,
    /// `m_untargetableExpireFrame`.
    pub untargetable_expire_frame: u32,
    /// `m_lockonDrawable` (its id).
    pub lockon_drawable: Option<u32>,
    /// `m_flags` (the `FlagType` bits).
    pub flags: i32,
    /// `m_landingPosForHelipadStuff`.
    pub landing_pos_for_helipad_stuff: Coord3D,
    /// `m_enginesOn`.
    pub engines_on: bool,
}

impl Jet {
    /// `JetAIUpdate(thing, moduleData)`.
    pub fn new(data: JetData) -> Self {
        Jet {
            data,
            producer_location: Coord3D::default(),
            most_recent_command: AiCommandParmsStorage::default(),
            afterburner_handle: 0,
            attack_loco_expire_frame: 0,
            attackers_miss_expire_frame: 0,
            return_to_base_frame: 0,
            targeted_by: Vec::new(),
            untargetable_expire_frame: 0,
            lockon_drawable: None,
            flags: 0,
            landing_pos_for_helipad_stuff: Coord3D::default(),
            engines_on: true,
        }
    }

    /// `getFlag(f)`.
    pub fn get_flag(&self, f: u32) -> bool {
        (self.flags & (1 << f)) != 0
    }

    /// `setFlag(f, v)`.
    pub fn set_flag(&mut self, f: u32, v: bool) {
        if v {
            self.flags |= 1 << f;
        } else {
            self.flags &= !(1 << f);
        }
    }
}

/// The running AI's jet members.
pub fn jt<H: AiStateHost>(h: &mut H) -> &mut Jet {
    match &mut ai(h).subclass {
        AiSubclass::Jet(j) => j,
        _ => unreachable!("a JetAIUpdate"),
    }
}

fn flag<H: AiStateHost>(h: &mut H, f: u32) -> bool {
    jt(h).get_flag(f)
}

fn set_flag<H: AiStateHost>(h: &mut H, f: u32, v: bool) {
    jt(h).set_flag(f, v);
}

/// `getPP(id)`: an airfield (alive, `KINDOF_FS_AIRFIELD`, not sold) with a parking place: its id.
pub fn get_pp<H: AiStateHost>(h: &mut H, id: ObjectId) -> Option<ObjectId> {
    if !h.object_exists(id)
        || h.object_is_effectively_dead(id)
        || !h.object_is_kind_of(id, crate::common::kind_of::KINDOF_FS_AIRFIELD)
        || h.object_test_status(id, OBJECT_STATUS_SOLD)
    {
        return None;
    }
    h.parking_place_exists(id).then_some(id)
}

/// `getPP(jet->getProducerID())`.
fn producer_pp<H: AiStateHost>(h: &mut H) -> Option<ObjectId> {
    let p = producer(h);
    get_pp(h, p)
}

fn producer<H: AiStateHost>(h: &mut H) -> ObjectId {
    let me = h.object_id();
    h.object_producer_id(me)
}

fn set_producer<H: AiStateHost>(h: &mut H, airfield: ObjectId) {
    let me = h.object_id();
    h.object_set_producer(me, airfield);
}

fn parking_offset<H: AiStateHost>(h: &mut H) -> f32 {
    jt(h).data.parking_offset
}

/// `pp->reserveSpace(jet, parkingOffset, &ppinfo)`.
fn reserve_space_info<H: AiStateHost>(h: &mut H, airfield: ObjectId) -> Option<PpInfo> {
    let me = h.object_id();
    let offset = parking_offset(h);
    h.pp_reserve_space(airfield, me, offset, true)
}

/// `findSuitableAirfield(jet)`: the closest allied airfield (not under construction, not sold, alive, on the same map) with a space
/// it reserves for us (the partition manager's search: the host's).
fn find_suitable_airfield<H: AiStateHost>(h: &mut H) -> Option<ObjectId> {
    let me = h.object_id();
    // (PartitionFilterHasParkingPlace: the parking place reserves a space at no offset)
    let candidate = h.jet_airfield_candidate().filter(|&a| get_pp(h, a).is_some_and(|pp| h.pp_reserve_space(pp, me, 0.0, false).is_some()));
    h.log_find_suitable_airfield(me, candidate.unwrap_or(INVALID_ID));
    candidate
}

/// The current locomotor, which upstream uses unchecked (none: undefined).
fn cur_loco_checked<H: AiStateHost>(h: &mut H, what: &'static str) -> SmResult<()> {
    if ai(h).get_cur_locomotor().is_none() {
        return Err(StateMachineError::Undefined(what));
    }
    Ok(())
}

/// The current locomotor's precision flags (`setUsePreciseZPos`, `setUltraAccurate`, and `setAllowInvalidPosition` when given).
fn set_loco_precision<H: AiStateHost>(h: &mut H, precise: bool, ultra: bool, invalid: Option<bool>) {
    if let Some(l) = ai(h).get_cur_locomotor_mut() {
        l.set_use_precise_z_pos(precise);
        l.set_ultra_accurate(ultra);
        if let Some(i) = invalid {
            l.set_allow_invalid_position(i);
        }
    }
}

fn set_loco_max_lift<H: AiStateHost>(h: &mut H, lift: f32) {
    if let Some(l) = ai(h).get_cur_locomotor_mut() {
        l.set_max_lift(lift);
    }
}

fn choose_locomotor_set<H: AiStateHost>(h: &mut H, wst: LocomotorSetType) {
    with_ai(h, |a, h| a.choose_locomotor_set(h, wst));
}

fn ignore_obstacle_id<H: AiStateHost>(h: &mut H, id: ObjectId) {
    ai(h).ignore_obstacle_id(id);
}

fn set_locomotor_goal_none<H: AiStateHost>(h: &mut H) {
    with_ai(h, |a, h| a.set_locomotor_goal_none(h));
}

/// The jet's own path: from its position, marked optimised, then `nodes` (all on the ground), as its path (the old destroyed).
fn set_ground_path<H: AiStateHost>(h: &mut H, nodes: &[Coord3D]) {
    let mut path = Path::new();
    let pos = h.position();
    path.prepend_node(&pos, LAYER_GROUND);
    path.mark_optimized();
    for n in nodes {
        path.append_node(n, LAYER_GROUND);
    }
    // (TheAI->pathfinder()->setDebugPath: the debug display's)
    with_ai(h, |a, h| a.friend_set_path(h, Some(path)));
}

/// `isOutOfSpecialReloadAmmo()`: at least one weapon that returns to base to reload, and all such out of ammo.
pub fn is_out_of_special_reload_ammo<H: AiStateHost>(h: &mut H) -> bool {
    let mut specials = 0;
    let mut out = 0;
    for i in SLOTS {
        if !h.weapon_slot_present(i) || !h.weapon_returns_to_base_to_reload(i) {
            continue;
        }
        specials += 1;
        if h.weapon_status_now(i) == WeaponStatus::OutOfAmmo {
            out += 1;
        }
    }
    specials > 0 && out == specials && !plant(3760)
}

/// `intersectInfiniteLine2D(ax, ay, ao, cx, cy, co)`: the intersection of the two lines through the points at the angles (none:
/// parallel).
fn intersect_infinite_line_2d(ax: f32, ay: f32, ao: f32, cx: f32, cy: f32, co: f32) -> Option<(f32, f32)> {
    let bx = ax + ao.cos();
    let by = ay + ao.sin();
    let dx = cx + co.cos();
    let dy = cy + co.sin();
    let denom = (bx - ax) * (dy - cy) - (by - ay) * (dx - cx);
    if denom == 0.0 {
        return None; // "the lines are parallel."
    }
    let r = ((ay - cy) * (dx - cx) - (ax - cx) * (dy - cy)) / denom;
    Some((ax + r * (bx - ax), ay + r * (by - ay)))
}

fn calc_dist_sqr(a: &Coord3D, b: &Coord3D) -> f32 {
    let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
    dx * dx + dy * dy + dz * dz
}

/// `JetAIStateMachine(owner, name)`'s (`needs_runway`) or `HeliAIStateMachine`'s own states (after the AI's).
pub fn define_jet_states<H: AiStateHost>(sm: &mut Sm<H>, needs_runway: bool) -> SmResult<()> {
    let taxi = |mode| AiState::JetTaxi { mv: MoveToData::default(), mode };
    if needs_runway {
        let takeoff = |landing| AiState::JetTakeoffOrLanding {
            mv: MoveToData::default(),
            index: 0,
            adjust_final: true,
            adjust_final_override: false,
            retry_count: 10,
            max_lift: 0.0,
            max_speed: 0.0,
            landing,
            landing_sound_played: false,
        };
        sm.define_state(RETURNING_FOR_LANDING, AiState::JetReturnForLanding { mv: MoveToData::default() }, LANDING_AWAIT_CLEARANCE, RETURN_TO_DEAD_AIRFIELD, &[])?;
        sm.define_state(TAKING_OFF_AWAIT_CLEARANCE, AiState::JetAwaitingRunway { landing: false }, TAXI_TO_TAKEOFF, AI_IDLE, &[])?;
        sm.define_state(TAXI_TO_TAKEOFF, taxi(FROM_PARKING), PAUSE_BEFORE_TAKEOFF, AI_IDLE, &[])?;
        sm.define_state(
            PAUSE_BEFORE_TAKEOFF,
            AiState::JetPause { obj: false, can_turn_in_place: false, when: 0, when_transfer: 0, waited_for_taxi_id: INVALID_ID, reset_timer: false, afterburners: false },
            TAKING_OFF,
            AI_IDLE,
            &[],
        )?;
        sm.define_state(TAKING_OFF, takeoff(false), AI_IDLE, AI_IDLE, &[])?;
        sm.define_state(LANDING_AWAIT_CLEARANCE, AiState::JetAwaitingRunway { landing: true }, LANDING, AI_IDLE, &[])?;
        sm.define_state(LANDING, takeoff(true), TAXI_FROM_LANDING, AI_IDLE, &[])?;
        sm.define_state(TAXI_FROM_LANDING, taxi(TO_PARKING), ORIENT_FOR_PARKING_PLACE, AI_IDLE, &[])?;
        sm.define_state(TAXI_FROM_HANGAR, taxi(FROM_HANGAR), ORIENT_FOR_PARKING_PLACE, AI_IDLE, &[])?;
        sm.define_state(ORIENT_FOR_PARKING_PLACE, AiState::JetParkOrient, RELOAD_AMMO, AI_IDLE, &[])?;
        sm.define_state(RELOAD_AMMO, AiState::JetReload { reload_time: 0, reload_done_frame: 0 }, AI_IDLE, AI_IDLE, &[])?;
        sm.define_state(RETURN_TO_DEAD_AIRFIELD, AiState::JetReturnToDeadAirfield { mv: MoveToData::default() }, CIRCLING_DEAD_AIRFIELD, RETURN_TO_DEAD_AIRFIELD, &[])?;
        sm.define_state(CIRCLING_DEAD_AIRFIELD, AiState::JetCircling { check_airfield: 0 }, AI_IDLE, AI_IDLE, &[])?;
    } else {
        let heli = |landing| AiState::HeliTakeoffOrLanding {
            path: [Coord3D::default(); 2],
            index: 0,
            parking_loc: Coord3D::default(),
            parking_orientation: 0.0,
            landing,
        };
        sm.define_state(RETURNING_FOR_LANDING, AiState::JetReturnForLanding { mv: MoveToData::default() }, LANDING_AWAIT_CLEARANCE, RETURN_TO_DEAD_AIRFIELD, &[])?;
        sm.define_state(TAKING_OFF_AWAIT_CLEARANCE, AiState::Utility(UtilityState::Success), TAKING_OFF, AI_IDLE, &[])?;
        sm.define_state(TAKING_OFF, heli(false), AI_IDLE, AI_IDLE, &[])?;
        sm.define_state(LANDING_AWAIT_CLEARANCE, AiState::Utility(UtilityState::Success), ORIENT_FOR_PARKING_PLACE, AI_IDLE, &[])?;
        sm.define_state(ORIENT_FOR_PARKING_PLACE, AiState::JetParkOrient, LANDING, AI_IDLE, &[])?;
        sm.define_state(LANDING, heli(true), RELOAD_AMMO, AI_IDLE, &[])?;
        sm.define_state(RELOAD_AMMO, AiState::JetReload { reload_time: 0, reload_done_frame: 0 }, AI_IDLE, AI_IDLE, &[])?;
        sm.define_state(RETURN_TO_DEAD_AIRFIELD, AiState::JetReturnToDeadAirfield { mv: MoveToData::default() }, CIRCLING_DEAD_AIRFIELD, RETURN_TO_DEAD_AIRFIELD, &[])?;
        sm.define_state(CIRCLING_DEAD_AIRFIELD, AiState::JetCircling { check_airfield: 0 }, AI_IDLE, AI_IDLE, &[])?;
        sm.define_state(TAXI_FROM_HANGAR, taxi(FROM_HANGAR), AI_IDLE, AI_IDLE, &[])?;
    }
    Ok(())
}

// ---- JetAwaitingRunwayState

/// `JetAwaitingRunwayState::onEnter()`: taking off or landing, circling allowed.
pub fn awaiting_runway_on_enter<H: AiStateHost>(h: &mut H, landing: bool) -> StateReturn {
    set_flag(h, TAKEOFF_IN_PROGRESS, !landing);
    set_flag(h, LANDING_IN_PROGRESS, landing);
    set_flag(h, ALLOW_CIRCLING, true);
    StateReturn::CONTINUE
}

/// `JetAwaitingRunwayState::update()`: a runway reserved (a space first): success; none to wait at: success; no space: failure;
/// taking off a carrier, moving forward in the queue when a better place is free; waiting meanwhile.
pub fn awaiting_runway_update<H: AiStateHost>(h: &mut H, landing: bool) -> SmResult<StateReturn> {
    if h.is_effectively_dead() {
        return Ok(StateReturn::FAILURE);
    }
    let Some(pp) = producer_pp(h) else {
        return Ok(StateReturn::SUCCESS); // "no producer? just skip this step."
    };
    let me = h.object_id();
    // "gotta reserve a space in order to reserve a runway"
    let offset = parking_offset(h);
    if h.pp_reserve_space(pp, me, offset, false).is_none() {
        return Ok(StateReturn::FAILURE);
    }
    if h.pp_reserve_runway(pp, me, landing) && !plant(3761) {
        return Ok(StateReturn::SUCCESS);
    } else if h.status_test(OBJECT_STATUS_DECK_HEIGHT_OFFSET) && !landing {
        // "we need to be at the front of the carrier queue."
        if let Some(best) = h.pp_calc_best_parking_assignment(pp, me) {
            set_flag(h, TAXI_IN_PROGRESS, true);
            set_flag(h, ALLOW_AIR_LOCO, false);
            choose_locomotor_set(h, LOCOMOTORSET_TAXIING);
            with_ai(h, |a, h| a.destroy_path(h));
            set_ground_path(h, &[best]);
            cur_loco_checked(h, "a carrier's queue move without a locomotor")?;
            set_loco_precision(h, true, true, Some(true));
            let p = producer(h);
            ignore_obstacle_id(h, p);
        }
    }
    // "can't get a runway? gotta wait."
    set_locomotor_goal_none(h);
    Ok(StateReturn::CONTINUE)
}

/// `JetAwaitingRunwayState::onExit()`.
pub fn awaiting_runway_on_exit<H: AiStateHost>(h: &mut H) {
    set_flag(h, TAKEOFF_IN_PROGRESS, false);
    set_flag(h, LANDING_IN_PROGRESS, false);
    set_flag(h, ALLOW_CIRCLING, false);
}

// ---- JetOrHeliCirclingDeadAirfieldState

/// `JetOrHeliCirclingDeadAirfieldState::onEnter()`: not out of ammunition with no producer (placed on the map): failure; else
/// staying, the low-fuel voice.
pub fn circling_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> StateReturn {
    if !is_out_of_special_reload_ammo(h) && producer(h) == INVALID_ID {
        return StateReturn::FAILURE;
    }
    set_locomotor_goal_none(h); // "just stay where we are."
    if let Some(AiState::JetCircling { check_airfield }) = sm.state_mut(id) {
        *check_airfield = HOW_OFTEN_TO_CHECK;
    }
    // "Play the "low fuel" voice whenever the craft is circling above the airfield."
    let me = h.object_id();
    h.play_per_unit_sound_for(b"VoiceLowFuel", me);
    StateReturn::CONTINUE
}

/// `JetOrHeliCirclingDeadAirfieldState::update()`: staying, the out-of-ammo damage; every second, a suitable airfield (our
/// producer now): success.
pub fn circling_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> StateReturn {
    set_locomotor_goal_none(h);
    let mut damage_rate = jt(h).data.out_of_ammo_damage_per_second;
    if damage_rate > 0.0 {
        damage_rate *= SECONDS_PER_LOGICFRAME_REAL;
        damage_rate *= h.body_max_health();
        let mut damage_info = DamageInfo::default();
        damage_info.input.damage_type = DAMAGE_UNRESISTABLE;
        damage_info.input.death_type = DEATH_NORMAL;
        damage_info.input.source_id = INVALID_ID;
        damage_info.input.amount = damage_rate;
        h.attempt_damage(&mut damage_info);
    }
    let check = match sm.state_mut(id) {
        Some(AiState::JetCircling { check_airfield }) => {
            *check_airfield -= 1;
            *check_airfield <= 0
        }
        _ => unreachable!("the circling state"),
    };
    if check {
        if let Some(AiState::JetCircling { check_airfield }) = sm.state_mut(id) {
            *check_airfield = HOW_OFTEN_TO_CHECK;
        }
        if let Some(airfield) = find_suitable_airfield(h) {
            set_producer(h, airfield);
            return StateReturn::SUCCESS;
        }
    }
    StateReturn::CONTINUE
}

// ---- JetOrHeliReturningToDeadAirfieldState, JetOrHeliReturnForLandingState (AIInternalMoveToStates)

/// `JetOrHeliReturningToDeadAirfieldState::onEnter()`: to the producer's location, adjusted.
pub fn return_to_dead_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let goal = jt(h).producer_location;
    let mv = move_states::mv(sm, id);
    mv.adjust_destinations = true;
    mv.goal_position = goal;
    move_states::internal_on_enter(sm, h, id)
}

/// `JetOrHeliReturnForLandingState::onEnter()`: the producer's parking place (its producer gone: forgotten, a suitable airfield's
/// instead, or failure); to the helipad's landing position, else (a space reserved, or failure) the runway's approach or the
/// parking space, exactly.
pub fn return_for_landing_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let mut pp = producer_pp(h);
    if pp.is_none() {
        // "nuke the producer id, since it's dead"
        set_producer(h, INVALID_ID);
        let airfield = find_suitable_airfield(h);
        pp = match airfield {
            Some(a) => get_pp(h, a),
            None => None,
        };
        match (airfield, pp) {
            (Some(a), Some(_)) => set_producer(h, a),
            _ => return Ok(StateReturn::FAILURE),
        }
    }
    let pp = pp.expect("a parking place");
    let goal = if h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) {
        jt(h).landing_pos_for_helipad_stuff
    } else {
        let Some(info) = reserve_space_info(h, pp) else { return Ok(StateReturn::FAILURE) };
        if jt(h).data.needs_runway {
            info.runway_approach
        } else {
            info.parking_space
        }
    };
    let mv = move_states::mv(sm, id);
    mv.goal_position = goal;
    mv.adjust_destinations = false; // "precision is necessary"
    move_states::internal_on_enter(sm, h, id)
}

// ---- JetOrHeliTaxiState (an AIMoveOutOfTheWayState)

/// `JetOrHeliTaxiState::onEnter()`: taxiing (from the hangar, from parking to the runway, from the runway to parking) on the
/// ground: the path through the parking place's points (an intermediate one where the parking place faces askew), precise; then
/// the move out of the way's (to the path's end).
pub fn taxi_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, mode: i32) -> SmResult<StateReturn> {
    ai(h).set_can_path_through_units(true);
    set_flag(h, TAKEOFF_IN_PROGRESS, mode != TO_PARKING);
    set_flag(h, LANDING_IN_PROGRESS, mode == TO_PARKING);
    set_flag(h, TAXI_IN_PROGRESS, true);
    if mode == TO_PARKING && h.has_countermeasures() {
        h.reload_countermeasures(); // "Instantly reload flares."
    }
    set_flag(h, ALLOW_AIR_LOCO, false);
    choose_locomotor_set(h, LOCOMOTORSET_TAXIING);
    let Some(pp) = producer_pp(h) else {
        return Ok(StateReturn::SUCCESS); // "no airfield? just skip this step."
    };
    let Some(info) = reserve_space_info(h, pp) else {
        return Ok(StateReturn::FAILURE); // "full?"
    };
    let mut intermed: Option<Coord3D> = None;
    // (`atan2` on the floats: the double one, rounded)
    let orient = ((info.runway_prep.y - info.parking_space.y) as f64).atan2((info.runway_prep.x - info.parking_space.x) as f64) as f32;
    if std_angle_diff(orient, info.parking_orientation).abs() > PI / 128.0 {
        let z = (info.parking_space.z + info.runway_prep.z) * 0.5;
        intermed = intersect_infinite_line_2d(
            info.parking_space.x,
            info.parking_space.y,
            info.parking_orientation,
            info.runway_prep.x,
            info.runway_prep.y,
            info.parking_orientation + PI / 2.0,
        )
        .map(|(x, y)| Coord3D { x, y, z });
    }
    with_ai(h, |a, h| a.destroy_path(h));
    let me = h.object_id();
    let deck = h.status_test(OBJECT_STATUS_DECK_HEIGHT_OFFSET);
    let mut nodes: Vec<Coord3D> = Vec::new();
    if mode == TO_PARKING {
        if deck {
            // "We're on an aircraft carrier."
            if let Some(locs) = h.pp_taxi_locations(pp, me) {
                nodes.extend(locs);
            }
            // "We just landed... see if we can get a better space forward"
            let Some(pos) = h.pp_calc_best_parking_assignment(pp, me) else {
                return Err(StateMachineError::Undefined("a carrier's taxi to parking without a better place (its position uninitialised)"));
            };
            nodes.push(pos);
        } else {
            // "We're on a normal airfield"
            nodes.push(info.runway_prep);
            nodes.extend(intermed);
            nodes.push(info.parking_space);
        }
    } else if mode == FROM_PARKING {
        if deck {
            if info.runway_start != info.runway_prep {
                nodes.push(info.runway_start);
            }
        } else {
            nodes.extend(intermed);
            nodes.push(info.runway_prep);
            nodes.push(info.runway_start);
        }
    } else if deck {
        // "Aircraft carrier"
        if h.status_test(OBJECT_STATUS_REASSIGN_PARKING) {
            // "we are being reassigned a parking space."
            h.set_status(OBJECT_STATUS_REASSIGN_PARKING, false);
            nodes.push(info.runway_prep);
        } else {
            let Some(locs) = h.pp_creation_locations(pp, me) else {
                return Ok(StateReturn::FAILURE); // "No creation locations specified for runway"
            };
            // "Skip the first node because it's the creation location."
            nodes.extend(locs.into_iter().skip(1));
            nodes.push(info.runway_prep);
        }
    } else {
        // "Airfield"
        nodes.push(info.parking_space);
    }
    // (the path made as one, then set, as upstream)
    let mut path = Path::new();
    let pos = h.position();
    path.prepend_node(&pos, LAYER_GROUND);
    path.mark_optimized();
    for n in &nodes {
        path.append_node(n, LAYER_GROUND);
    }
    {
        let mv = move_states::mv(sm, id);
        mv.waiting_for_path = false;
        mv.adjust_destinations = false; // "precision is necessary"
    }
    with_ai(h, |a, h| a.friend_set_path(h, Some(path)));
    cur_loco_checked(h, "a taxi without a locomotor")?;
    set_loco_precision(h, true, true, Some(true));
    let p = producer(h);
    ignore_obstacle_id(h, p);
    move_states::on_enter(sm, h, id)
}

/// `JetOrHeliTaxiState::update()`: taxiing to parking or from the hangar, a better parking place appended to the path; then the
/// move out of the way's.
pub fn taxi_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, mode: i32) -> SmResult<StateReturn> {
    if h.is_effectively_dead() {
        return Ok(StateReturn::FAILURE);
    }
    if mode == TO_PARKING || mode == FROM_HANGAR {
        // "it is possible for your airfield to get destroyed while taxiing."
        if let Some(pp) = producer_pp(h) {
            let me = h.object_id();
            if let Some(best) = h.pp_calc_best_parking_assignment(pp, me) {
                if let Some(path) = ai(h).path.as_mut() {
                    path.append_node(&best, LAYER_GROUND);
                }
            }
        }
    }
    move_states::update(sm, h, id)
}

/// `JetOrHeliTaxiState::onExit()`: the locomotor no longer precise (used unchecked), the flags cleared; then the move's.
pub fn taxi_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, status: StateExitType) -> SmResult<()> {
    cur_loco_checked(h, "a taxi's end without a locomotor")?;
    set_loco_precision(h, false, false, Some(false));
    set_flag(h, TAKEOFF_IN_PROGRESS, false);
    set_flag(h, LANDING_IN_PROGRESS, false);
    set_flag(h, TAXI_IN_PROGRESS, false);
    ai(h).set_can_path_through_units(false);
    move_states::on_exit(sm, h, id, status);
    Ok(())
}

// ---- JetTakeoffOrLandingState (an AIFollowPathState)

fn takeoff_landing<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> bool {
    match sm.state(id) {
        Some(AiState::JetTakeoffOrLanding { landing, .. }) => *landing,
        _ => unreachable!("the take-off or landing state"),
    }
}

/// `JetTakeoffOrLandingState::onEnter()`: flying allowed, the normal set; its lift and speed kept (landing: the minimum speed;
/// taking off: no lift yet), precise; the runway's path (landing: the approach, then the carrier's landing strip or the runway
/// backwards; taking off: its end at the approach's height, then its exit); then the follow path's.
pub fn takeoff_or_landing_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let landing = takeoff_landing(sm, id);
    if h.is_effectively_dead() {
        return Ok(StateReturn::FAILURE);
    }
    set_flag(h, TAKEOFF_IN_PROGRESS, !landing);
    set_flag(h, LANDING_IN_PROGRESS, landing);
    set_flag(h, ALLOW_AIR_LOCO, true);
    choose_locomotor_set(h, LOCOMOTORSET_NORMAL);
    cur_loco_checked(h, "a take-off or landing without a locomotor")?;
    set_loco_max_lift(h, BIGNUM);
    let bdt = h.damage_state();
    let (max_lift, max_speed) = with_ai(h, |a, h| {
        let l = a.get_cur_locomotor().expect("the locomotor");
        (l.get_max_lift(h, bdt), l.get_max_speed_for_condition(h, bdt))
    });
    if let Some(AiState::JetTakeoffOrLanding { max_lift: m, max_speed: s, landing_sound_played, .. }) = sm.state_mut(id) {
        *m = max_lift;
        *s = max_speed;
        *landing_sound_played = false;
    }
    if let Some(l) = ai(h).get_cur_locomotor_mut() {
        if landing {
            let min = l.get_min_speed();
            l.set_max_speed(min);
        } else {
            l.set_max_lift(0.0);
        }
        l.set_use_precise_z_pos(true);
        l.set_ultra_accurate(true);
    }
    let p = producer(h);
    ignore_obstacle_id(h, p);
    let Some(pp) = get_pp(h, p) else {
        return Ok(StateReturn::SUCCESS); // "no airfield? just skip this step"
    };
    let Some(mut info) = reserve_space_info(h, pp) else {
        return Ok(StateReturn::FAILURE); // "it's full."
    };
    // "only check this for landing; we might have already given up the reservation to the guy behind us for takeoff"
    let me = h.object_id();
    if landing && !h.pp_reserve_runway(pp, me, landing) {
        return Ok(StateReturn::FAILURE);
    }
    let mut path = Vec::new();
    if landing {
        path.push(info.runway_approach);
        if h.status_test(OBJECT_STATUS_DECK_HEIGHT_OFFSET) {
            // "Assigned to an aircraft carrier which has separate landing strips."
            path.push(info.runway_landing_start);
            path.push(info.runway_landing_end);
        } else {
            // "land the same way we took off but in reverse."
            path.push(info.runway_end);
            path.push(info.runway_start);
        }
    } else {
        info.runway_end.z = info.runway_approach.z;
        path.push(info.runway_end);
        path.push(info.runway_exit);
    }
    move_states::mv(sm, id).adjust_destinations = false; // "precision is necessary"
    if let Some(AiState::JetTakeoffOrLanding { adjust_final, .. }) = sm.state_mut(id) {
        *adjust_final = false; // "especially at the endpoint!"
    }
    ai(h).machine.goal_path = path;
    move_states::on_enter(sm, h, id)
}

/// `JetTakeoffOrLandingState::update()`: landing, full lift (the wheels' screech once on the ground); taking off, the runway passed
/// to the next in line and the lift growing with the distance run; then the follow path's.
pub fn takeoff_or_landing_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if h.is_effectively_dead() {
        return Ok(StateReturn::FAILURE);
    }
    let landing = takeoff_landing(sm, id);
    if landing {
        cur_loco_checked(h, "a landing without a locomotor")?;
        set_loco_max_lift(h, BIGNUM);
        let played = matches!(sm.state(id), Some(AiState::JetTakeoffOrLanding { landing_sound_played: true, .. }));
        if !played {
            let pp = producer_pp(h);
            let pos = h.position();
            let z_slop = 0.25;
            let layer = h.highest_layer_for_destination(&pos);
            let mut ground_z = h.layer_height(pos.x, pos.y, layer);
            if let Some(pp) = pp {
                ground_z += h.pp_landing_deck_height_offset(pp);
            }
            if pos.z - z_slop <= ground_z {
                if let Some(AiState::JetTakeoffOrLanding { landing_sound_played, .. }) = sm.state_mut(id) {
                    *landing_sound_played = true;
                }
                h.play_wheel_screech(&pos);
            }
        }
    } else {
        let me = h.object_id();
        let pp = producer_pp(h);
        if let Some(pp) = pp {
            h.pp_transfer_runway_reservation(pp, me);
        }
        // "As it approaches the end of the runway, the plane will gain more lift"
        let Some(pp) = pp else {
            return Err(StateMachineError::Undefined("a take-off's lift without a parking place (calcPPInfo on none)"));
        };
        let info = h.pp_calc_pp_info(pp, me);
        let pos = h.position();
        let (dx, dy, dz) = (info.runway_end.x - pos.x, info.runway_end.y - pos.y, info.runway_end.z - pos.z);
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let mut ratio: f32 = 1.0 - (dist / info.runway_takeoff_dist);
        ratio *= ratio; // "dampen it...."
        ratio = ratio.clamp(0.0, 1.0);
        let max_lift = match sm.state(id) {
            Some(AiState::JetTakeoffOrLanding { max_lift, .. }) => *max_lift,
            _ => unreachable!("the take-off or landing state"),
        };
        cur_loco_checked(h, "a take-off without a locomotor")?;
        set_loco_max_lift(h, max_lift * ratio);
    }
    move_states::update(sm, h, id)
}

/// `JetTakeoffOrLandingState::onExit()`: the follow path's first; the flags and afterburners off, the locomotor (if any) no longer
/// precise and its lift restored (not when dead); taking off, its space given up (unless kept airborne); the runway released.
pub fn takeoff_or_landing_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId, status: StateExitType) {
    let landing = takeoff_landing(sm, id);
    move_states::on_exit(sm, h, id, status);
    set_flag(h, TAKEOFF_IN_PROGRESS, false);
    set_flag(h, LANDING_IN_PROGRESS, false);
    enable_afterburners(h, false);
    let dead = h.is_effectively_dead();
    if let Some(l) = ai(h).get_cur_locomotor_mut() {
        l.set_use_precise_z_pos(false);
        l.set_ultra_accurate(false);
        // "don't restore lift if dead -- this may fight with JetSlowDeathBehavior!"
        if !dead {
            l.set_max_lift(BIGNUM);
        }
    }
    ignore_obstacle_id(h, INVALID_ID);
    let pp = producer_pp(h);
    let me = h.object_id();
    if !landing {
        if let Some(pp) = pp {
            if !jt(h).data.keeps_parking_space_when_airborne {
                h.pp_release_space(pp, me);
            }
        }
    }
    if let Some(pp) = pp {
        h.pp_release_runway(pp, me);
    }
}

// ---- HeliTakeoffOrLandingState

/// `HeliTakeoffOrLandingState::onEnter()`: flying allowed, precise; the two points: the parking spot (a helipad's landing position
/// or our own, else the reserved space) and above it at the approach's height; landing, down; taking off, up.
pub fn heli_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let landing = matches!(sm.state(id), Some(AiState::HeliTakeoffOrLanding { landing: true, .. }));
    set_flag(h, TAKEOFF_IN_PROGRESS, !landing);
    set_flag(h, LANDING_IN_PROGRESS, landing);
    set_flag(h, ALLOW_AIR_LOCO, true);
    choose_locomotor_set(h, LOCOMOTORSET_NORMAL);
    cur_loco_checked(h, "a helicopter's take-off or landing without a locomotor")?;
    set_loco_precision(h, true, true, None);
    let p = producer(h);
    ignore_obstacle_id(h, p);
    let Some(pp) = get_pp(h, p) else {
        return Ok(StateReturn::SUCCESS); // "no airfield? just skip this step"
    };
    let (parking_loc, parking_orientation, approach) = if h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) {
        let orientation = h.orientation();
        let loc = if landing { jt(h).landing_pos_for_helipad_stuff } else { h.position() };
        let mut approach = loc;
        approach.z += h.pp_approach_height(pp) + h.pp_landing_deck_height_offset(pp);
        (loc, orientation, approach)
    } else {
        let Some(info) = reserve_space_info(h, pp) else { return Ok(StateReturn::FAILURE) };
        let mut approach = info.parking_space;
        approach.z += info.runway_approach.z - info.runway_end.z;
        (info.parking_space, info.parking_orientation, approach)
    };
    if let Some(AiState::HeliTakeoffOrLanding { path, index, parking_loc: l, parking_orientation: o, .. }) = sm.state_mut(id) {
        *l = parking_loc;
        *o = parking_orientation;
        *path = if landing { [approach, parking_loc] } else { [parking_loc, approach] };
        *index = 0;
    }
    Ok(StateReturn::CONTINUE)
}

/// `HeliTakeoffOrLandingState::update()`: to the current point (adjusted, a goal of the pathfinder, unless landing at an airfield);
/// within 3 of it, the next; past the second: success.
pub fn heli_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> StateReturn {
    if h.is_effectively_dead() {
        return StateReturn::FAILURE;
    }
    let (landing, index, mut point) = match sm.state(id) {
        Some(AiState::HeliTakeoffOrLanding { landing, index, path, .. }) => (*landing, *index, path[*index as usize]),
        _ => unreachable!("the helicopter's take-off or landing state"),
    };
    if h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) || !landing {
        h.pf_adjust_destination(&mut point);
        h.pf_update_goal(&point, LAYER_GROUND);
        if let Some(AiState::HeliTakeoffOrLanding { path, .. }) = sm.state_mut(id) {
            path[index as usize] = point;
        }
    }
    ai(h).set_locomotor_goal_position_explicit(&point);
    const THRESH: f32 = 3.0;
    let mut index = index;
    if calc_dist_sqr(&h.position(), &point) <= THRESH * THRESH && !plant(3762) {
        index += 1;
    }
    if let Some(AiState::HeliTakeoffOrLanding { index: i, .. }) = sm.state_mut(id) {
        *i = index;
    }
    if index >= 2 {
        return StateReturn::SUCCESS;
    }
    StateReturn::CONTINUE
}

/// `HeliTakeoffOrLandingState::onExit()`: the flags off, the locomotor (if any) no longer precise, its lift restored (not when
/// dead); landed, taxiing; taken off, its space given up (unless kept airborne).
pub fn heli_on_exit<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) {
    let landing = matches!(sm.state(id), Some(AiState::HeliTakeoffOrLanding { landing: true, .. }));
    set_flag(h, TAKEOFF_IN_PROGRESS, false);
    set_flag(h, LANDING_IN_PROGRESS, false);
    let dead = h.is_effectively_dead();
    if let Some(l) = ai(h).get_cur_locomotor_mut() {
        l.set_use_precise_z_pos(false);
        l.set_ultra_accurate(false);
        if !dead {
            l.set_max_lift(BIGNUM);
        }
    }
    ignore_obstacle_id(h, INVALID_ID);
    let pp = producer_pp(h);
    if landing {
        set_flag(h, ALLOW_AIR_LOCO, false);
        choose_locomotor_set(h, LOCOMOTORSET_TAXIING);
    } else if let Some(pp) = pp {
        if !jt(h).data.keeps_parking_space_when_airborne {
            let me = h.object_id();
            h.pp_release_space(pp, me);
        }
    }
}

/// `HeliTakeoffOrLandingState::xfer` (version 1): the two points, the index, the parking spot and its orientation.
pub fn heli_xfer(path: &mut [Coord3D; 2], index: &mut i32, parking_loc: &mut Coord3D, parking_orientation: &mut f32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_coord3d(&mut path[0])?;
    xfer.xfer_coord3d(&mut path[1])?;
    xfer.xfer_int(index)?;
    xfer.xfer_coord3d(parking_loc)?;
    xfer.xfer_real(parking_orientation)?;
    Ok(())
}

// ---- JetOrHeliParkOrientState

/// `JetOrHeliParkOrientState::onEnter()`: a helipad's: success at once; else landing.
pub fn park_orient_on_enter<H: AiStateHost>(h: &mut H) -> StateReturn {
    if h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) {
        return StateReturn::SUCCESS;
    }
    set_flag(h, TAKEOFF_IN_PROGRESS, false);
    set_flag(h, LANDING_IN_PROGRESS, true);
    let p = producer(h);
    ignore_obstacle_id(h, p);
    StateReturn::CONTINUE
}

/// `JetOrHeliParkOrientState::update()`: facing the parking place's orientation: success; else stopped and set at its spot (a
/// carrier's runway preparation spot), turning to it (no parking place or space: failure).
pub fn park_orient_update<H: AiStateHost>(h: &mut H) -> SmResult<StateReturn> {
    if h.is_effectively_dead() {
        return Ok(StateReturn::FAILURE);
    }
    let Some(pp) = producer_pp(h) else { return Ok(StateReturn::FAILURE) };
    let Some(info) = reserve_space_info(h, pp) else { return Ok(StateReturn::FAILURE) };
    const THRESH: f32 = 0.001;
    if std_angle_diff(h.orientation(), info.parking_orientation).abs() <= THRESH {
        return Ok(StateReturn::SUCCESS);
    }
    // "magically position it correctly."
    if !h.has_physics() {
        return Err(StateMachineError::Undefined("a jet set at its parking place without physics"));
    }
    h.scrub_velocity_2d(0.0);
    let mut hoverloc = info.parking_space;
    if h.status_test(OBJECT_STATUS_DECK_HEIGHT_OFFSET) {
        hoverloc = info.runway_prep;
    }
    hoverloc.z = h.position().z;
    h.set_position(&hoverloc);
    ai(h).set_locomotor_goal_orientation(info.parking_orientation);
    Ok(StateReturn::CONTINUE)
}

/// `JetOrHeliParkOrientState::onExit()`.
pub fn park_orient_on_exit<H: AiStateHost>(h: &mut H) {
    set_flag(h, TAKEOFF_IN_PROGRESS, false);
    set_flag(h, LANDING_IN_PROGRESS, false);
    ignore_obstacle_id(h, INVALID_ID);
}

// ---- JetPauseBeforeTakeoffState (an AIFaceState facing the position)

/// `findWaiter()`: another jet with a take-off runway reservation still taxiing to take off (the first such kept as waited for).
fn find_waiter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> bool {
    let Some(pp) = producer_pp(h) else { return false };
    let me = h.object_id();
    let count = h.pp_runway_count(pp);
    for i in 0..count {
        let other = h.pp_runway_reservation(pp, i);
        if !h.object_exists(other) || other == me {
            continue;
        }
        if !h.object_has_ai(other) {
            continue;
        }
        if h.other_ai_current_state_id(other) == TAXI_TO_TAKEOFF {
            if let Some(AiState::JetPause { waited_for_taxi_id, .. }) = sm.state_mut(id) {
                if *waited_for_taxi_id == INVALID_ID {
                    *waited_for_taxi_id = other;
                }
            }
            return true;
        }
    }
    false
}

/// `JetPauseBeforeTakeoffState::onEnter()`: taking off; its members reset; facing the runway's end (no parking place or space:
/// success).
pub fn pause_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    set_flag(h, TAKEOFF_IN_PROGRESS, true);
    set_flag(h, LANDING_IN_PROGRESS, false);
    if let Some(AiState::JetPause { when, when_transfer, waited_for_taxi_id, reset_timer, afterburners, .. }) = sm.state_mut(id) {
        *when = 0;
        *when_transfer = 0;
        *waited_for_taxi_id = INVALID_ID;
        *reset_timer = false;
        *afterburners = false;
    }
    let Some(pp) = producer_pp(h) else {
        return Ok(StateReturn::SUCCESS); // "no airfield? just skip this step."
    };
    let Some(info) = reserve_space_info(h, pp) else {
        return Ok(StateReturn::SUCCESS); // "full?"
    };
    sm.set_goal_position(Some(&info.runway_end));
    crate::logic::ai::ai_face_exit_states::face_on_enter(sm, h, id)
}

/// `JetPauseBeforeTakeoffState::update()`: the face's always; another still taxiing: waiting; then the pause counted down (from
/// when the others are ready), the afterburners on, the runway passed on after a frame or two; the pause over: the face's answer.
pub fn pause_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if h.is_effectively_dead() {
        return Ok(StateReturn::FAILURE);
    }
    // "always call this."
    let super_status = crate::logic::ai::ai_face_exit_states::face_update(sm, h, id)?;
    if find_waiter(sm, h, id) {
        return Ok(StateReturn::CONTINUE);
    }
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    let me = h.object_id();
    let pause = jt(h).data.takeoff_pause;
    let (when, when_transfer, start_afterburners) = match sm.state_mut(id) {
        Some(AiState::JetPause { when, when_transfer, waited_for_taxi_id, reset_timer, afterburners, .. }) => {
            if !*reset_timer {
                // "we had to wait, but now everyone else is ready, so restart our countdown."
                *when = now.wrapping_add(pause);
                if *waited_for_taxi_id == INVALID_ID {
                    *waited_for_taxi_id = me; // "just so we don't pick up anyone else"
                    *when_transfer = now + 1;
                } else {
                    *when_transfer = now + 2; // "2 seems odd, but is correct"
                }
                *reset_timer = true;
            }
            let start = !*afterburners;
            *afterburners = true;
            (*when, *when_transfer, start)
        }
        _ => unreachable!("the pause before take-off"),
    };
    if start_afterburners {
        enable_afterburners(h, true);
    }
    // "once we start the final wait, release the runways for guys behind us, so they can start taxiing"
    if let Some(pp) = producer_pp(h) {
        if now >= when_transfer {
            h.pp_transfer_runway_reservation(pp, me);
        }
    }
    if now >= when && !plant(3763) {
        return Ok(super_status);
    }
    Ok(StateReturn::CONTINUE)
}

/// `JetPauseBeforeTakeoffState::onExit()` (the face's does nothing).
pub fn pause_on_exit<H: AiStateHost>(h: &mut H) {
    set_flag(h, TAKEOFF_IN_PROGRESS, false);
    set_flag(h, LANDING_IN_PROGRESS, false);
}

/// `JetPauseBeforeTakeoffState::xfer` (version 1): its own members only.
pub fn pause_xfer(when: &mut u32, when_transfer: &mut u32, afterburners: &mut bool, reset_timer: &mut bool, waited: &mut ObjectId, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_unsigned_int(when)?;
    xfer.xfer_unsigned_int(when_transfer)?;
    xfer.xfer_bool(afterburners)?;
    xfer.xfer_bool(reset_timer)?;
    xfer.xfer_object_id(waited)?;
    Ok(())
}

// ---- JetOrHeliReloadAmmoState

/// `JetOrHeliReloadAmmoState::onEnter()`: the longest reload of the weapons (each its clip's reload time by the share empty), at
/// least a frame.
pub fn reload_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> StateReturn {
    set_flag(h, TAKEOFF_IN_PROGRESS, false);
    set_flag(h, LANDING_IN_PROGRESS, false);
    set_flag(h, USE_SPECIAL_RETURN_LOCO, false);
    let mut reload_time: u32 = 0;
    for i in SLOTS {
        if !h.weapon_slot_present(i) {
            continue;
        }
        let remaining = h.weapon_remaining_ammo(i);
        let clip_size = h.weapon_clip_size(i);
        let mut rt = h.weapon_clip_reload_time(i);
        if clip_size > 0 {
            // "bias by amount empty."
            let needed = clip_size - remaining;
            rt = (rt * needed) / clip_size;
        }
        // (an Int compared with the UnsignedInt: as unsigned)
        if rt as u32 > reload_time {
            reload_time = rt as u32;
        }
    }
    if reload_time < 1 {
        reload_time = 1;
    }
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    if let Some(AiState::JetReload { reload_time: r, reload_done_frame }) = sm.state_mut(id) {
        *r = reload_time;
        *reload_done_frame = reload_time.wrapping_add(now);
    }
    StateReturn::CONTINUE
}

/// `JetOrHeliReloadAmmoState::update()`: each weapon's clip filled by the time passed (full when done); all full: success.
pub fn reload_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> StateReturn {
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    let (reload_time, done) = match sm.state(id) {
        Some(AiState::JetReload { reload_time, reload_done_frame }) => (*reload_time, *reload_done_frame),
        _ => unreachable!("the reload state"),
    };
    let mut all_done = true;
    for i in SLOTS {
        if !h.weapon_slot_present(i) {
            continue;
        }
        if now >= done {
            h.weapon_set_clip_percent_full(i, 1.0, false);
        } else {
            let percent = reload_time.wrapping_sub(done.wrapping_sub(now)) as f32 / reload_time as f32;
            h.weapon_set_clip_percent_full(i, percent, false);
        }
        if h.weapon_remaining_ammo(i) != h.weapon_clip_size(i) {
            all_done = false;
        }
    }
    if all_done && !plant(3764) {
        return StateReturn::SUCCESS;
    }
    StateReturn::CONTINUE
}

/// `JetOrHeliReloadAmmoState::onExit()`.
pub fn reload_on_exit<H: AiStateHost>(h: &mut H) {
    set_flag(h, TAKEOFF_IN_PROGRESS, false);
    set_flag(h, LANDING_IN_PROGRESS, false);
}

/// `JetOrHeliReloadAmmoState::xfer` (version 1): the reload time and its end.
pub fn reload_xfer(reload_time: &mut u32, reload_done_frame: &mut u32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_unsigned_int(reload_time)?;
    xfer.xfer_unsigned_int(reload_done_frame)?;
    Ok(())
}

// ---- JetAIUpdate's members

/// `isIdle()` (the jet's): never with a command waiting ("scripting relies on us never claiming to be "idle""), else the base's.
pub fn jet_is_idle(j: &Jet, base_idle: bool) -> bool {
    if j.get_flag(HAS_PENDING_COMMAND) && !plant(3765) {
        return false;
    }
    base_idle
}

/// `isReloading()`.
pub fn is_reloading(state: StateId) -> bool {
    state == RELOAD_AMMO
}

/// `isTaxiingToParking()`: in any state between parking and the air.
pub fn is_taxiing_to_parking(state: StateId) -> bool {
    matches!(
        state,
        TAXI_FROM_HANGAR | TAXI_FROM_LANDING | ORIENT_FOR_PARKING_PLACE | RELOAD_AMMO | TAKING_OFF_AWAIT_CLEARANCE | TAXI_TO_TAKEOFF | PAUSE_BEFORE_TAKEOFF | TAKING_OFF
    )
}

/// `onObjectCreated()`'s own part (after the base's): grounded, taxiing.
pub fn jet_on_object_created<H: AiStateHost>(h: &mut H) {
    set_flag(h, ALLOW_AIR_LOCO, false);
    choose_locomotor_set(h, LOCOMOTORSET_TAXIING);
}

/// `onDelete()`'s own part (after the base's, which does nothing): our space at the producer given up.
pub fn jet_on_delete<H: AiStateHost>(h: &mut H) {
    if let Some(pp) = producer_pp(h) {
        let me = h.object_id();
        h.pp_release_space(pp, me);
    }
}

/// `getProducerLocation()`: once, the producer's position (none: ours); with no space reserved there (spawned by a script, placed
/// on the map), flying at once, else taxiing.
pub fn get_producer_location<H: AiStateHost>(h: &mut H) {
    if flag(h, HAS_PRODUCER_LOCATION) {
        return;
    }
    let p = producer(h);
    let loc = if h.object_exists(p) { h.object_position(p) } else { h.position() };
    jt(h).producer_location = loc;
    let pp = get_pp(h, p);
    let me = h.object_id();
    if pp.is_none_or(|pp| !h.pp_has_reserved_space(pp, me)) {
        set_flag(h, ALLOW_AIR_LOCO, true);
        choose_locomotor_set(h, LOCOMOTORSET_NORMAL);
    } else {
        set_flag(h, ALLOW_AIR_LOCO, false);
        choose_locomotor_set(h, LOCOMOTORSET_TAXIING);
    }
    set_flag(h, HAS_PRODUCER_LOCATION, true);
}

/// The machine cleared, the AI's command source, then the state (`getStateMachine()->clear(); setLastCommandSource(src);
/// setState(state)`).
fn restart_in<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, state: StateId, clear: bool) -> SmResult<()> {
    if clear {
        aism_clear(sm, h)?;
    }
    commands::set_last_command_source(h, CMD_FROM_AI);
    aism_set_state(sm, h, state).map(|_| ())
}

/// `update()`'s own part, before the base's (the AI and its machine lent): the producer's location; idle (or reloading), at the
/// airfield healed (a helipad's: taking off again), out of special ammunition: back to reload; a waiting command given; idle too
/// long: back to base; not idle, a guard or hunt interrupted to reload; the drawable lifted to the minimum height; the exhaust; the
/// attack locomotor and the attackers' miss timers; the lock-on; the special locomotors; the engines' sound.
pub fn jet_pre_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    get_producer_location(h);
    let me = h.object_id();
    let pp = producer_pp(h);
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    // "srj sez: not 100% sure on this. calling RELOAD_AMMO "idle" allows us to get healed while reloading"
    let base_idle = ai(h).base_is_idle(&*sm);
    if base_idle || sm.get_current_state_id() == RELOAD_AMMO {
        if let Some(pp) = pp {
            let healed = h.body_health() == h.body_max_health();
            if !flag(h, ALLOW_AIR_LOCO) && !flag(h, HAS_PENDING_COMMAND) && h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) && healed && !plant(3766) {
                // "we're completely healed, so take off again"
                h.parking_place_set_healee(pp, false);
                set_flag(h, ALLOW_AIR_LOCO, true);
                restart_in(sm, h, TAKING_OFF_AWAIT_CLEARANCE, true)?;
            } else {
                let add = !flag(h, ALLOW_AIR_LOCO);
                h.parking_place_set_healee(pp, add);
            }
        }
        // "note that we might still have weapons with ammo, but still be forced to return to reload."
        if is_out_of_special_reload_ammo(h) && flag(h, ALLOW_AIR_LOCO) {
            jt(h).return_to_base_frame = 0;
            // "this is really a "just-in-case" to ensure the targeter list doesn't spin out of control (srj)"
            prune_dead_targeters(h);
            set_flag(h, USE_SPECIAL_RETURN_LOCO, true);
            restart_in(sm, h, RETURNING_FOR_LANDING, false)?;
        } else if flag(h, HAS_PENDING_COMMAND) && sm.get_current_state_id() != RELOAD_AMMO {
            // "if we are reloading ammo, wait will we are done before processing the pending command."
            jt(h).return_to_base_frame = 0;
            // "values don't matter, will be wiped by next line"
            let mut parms = AiCommandParms::new(AICMD_MOVE_TO_POSITION, CMD_FROM_AI);
            let storage = jt(h).most_recent_command.clone();
            storage.reconstitute(h, &mut parms);
            set_flag(h, HAS_PENDING_COMMAND, false);
            commands::ai_do_command(sm, h, &parms)?;
        } else if jt(h).return_to_base_frame != 0 && now >= jt(h).return_to_base_frame && flag(h, ALLOW_AIR_LOCO) {
            jt(h).return_to_base_frame = 0;
            set_flag(h, USE_SPECIAL_RETURN_LOCO, false);
            restart_in(sm, h, RETURNING_FOR_LANDING, false)?;
        } else if jt(h).return_to_base_frame == 0 && jt(h).data.return_to_base_idle_time > 0 && flag(h, ALLOW_AIR_LOCO) {
            let idle_time = jt(h).data.return_to_base_idle_time;
            jt(h).return_to_base_frame = now.wrapping_add(idle_time);
        }
    } else {
        if let Some(pp) = pp {
            h.parking_place_set_healee(pp, false);
        }
        jt(h).return_to_base_frame = 0;
        if flag(h, ALLOW_INTERRUPT_AND_RESUME_OF_CUR_STATE_FOR_RELOAD) && is_out_of_special_reload_ammo(h) && flag(h, ALLOW_AIR_LOCO) {
            set_flag(h, USE_SPECIAL_RETURN_LOCO, true);
            set_flag(h, HAS_PENDING_COMMAND, true);
            set_flag(h, ALLOW_INTERRUPT_AND_RESUME_OF_CUR_STATE_FOR_RELOAD, false);
            restart_in(sm, h, RETURNING_FOR_LANDING, false)?;
        }
    }

    let mut min_height = jt(h).data.min_height;
    if let Some(pp) = pp {
        min_height += h.pp_landing_deck_height_offset(pp);
    }
    if h.has_drawable() {
        let id = sm.get_current_state_id();
        let need_to_check_min_height = (JETAISTATETYPE_FIRST..=JETAISTATETYPE_LAST).contains(&id) || !h.is_above_terrain() || !flag(h, ALLOW_AIR_LOCO);
        if need_to_check_min_height || h.status_test(OBJECT_STATUS_DECK_HEIGHT_OFFSET) {
            let ht = if h.is_above_terrain() { h.height_above_terrain() } else { 0.0 };
            if ht < min_height {
                h.drawable_set_instance_matrix(Some(min_height - ht));
            } else {
                h.drawable_set_instance_matrix(None);
            }
        } else {
            h.drawable_set_instance_matrix(None);
        }
    }

    if !h.has_physics() {
        return Err(StateMachineError::Undefined("a jet's update without physics"));
    }
    if h.velocity_magnitude() > 0.0 && flag(h, ALLOW_AIR_LOCO) {
        h.set_model_condition_state(MODELCONDITION_JETEXHAUST);
    } else {
        h.clear_model_condition_state(MODELCONDITION_JETEXHAUST);
    }

    if h.status_test(OBJECT_STATUS_IS_ATTACKING) {
        let (loco_persist, miss_persist) = (jt(h).data.attack_loco_persist_time, jt(h).data.attackers_miss_persist_time);
        jt(h).attack_loco_expire_frame = now.wrapping_add(loco_persist);
        jt(h).attackers_miss_expire_frame = now.wrapping_add(miss_persist);
    } else {
        let j = jt(h);
        if j.attack_loco_expire_frame != 0 && now >= j.attack_loco_expire_frame {
            j.attack_loco_expire_frame = 0;
        }
        if j.attackers_miss_expire_frame != 0 && now >= j.attackers_miss_expire_frame {
            j.attackers_miss_expire_frame = 0;
        }
    }
    {
        let j = jt(h);
        if j.untargetable_expire_frame != 0 && now >= j.untargetable_expire_frame {
            j.untargetable_expire_frame = 0;
        }
    }

    position_lockon(h);

    if jt(h).attack_loco_expire_frame != 0 {
        let wst = jt(h).data.attacking_loco;
        choose_locomotor_set(h, wst);
    } else if flag(h, USE_SPECIAL_RETURN_LOCO) {
        let wst = jt(h).data.returning_loco;
        choose_locomotor_set(h, wst);
    }

    if !h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) && h.has_drawable() {
        let moving = flag(h, TAKEOFF_IN_PROGRESS)
            || flag(h, LANDING_IN_PROGRESS)
            || h.is_significantly_above_terrain()
            || with_ai(h, |a, h| a.is_moving(&*sm, h))
            || ai(h).is_waiting_for_path();
        if moving {
            if !jt(h).engines_on {
                // "We just started moving, therefore turn on the engines!"
                h.drawable_enable_ambient_sound(true);
                jt(h).engines_on = true;
            }
        } else if jt(h).engines_on {
            // "We're no longer moving, so turn off the engines!"
            h.drawable_enable_ambient_sound(false);
            jt(h).engines_on = false;
        }
    }
    let _ = me;
    Ok(())
}

/// `chooseLocomotorSet(wst)`'s override: grounded, the taxiing set; an attack's persisting, the attack set; returning for
/// ammunition, the return set.
pub fn locomotor_set_for(j: &Jet, wst: LocomotorSetType) -> LocomotorSetType {
    if !j.get_flag(ALLOW_AIR_LOCO) {
        LOCOMOTORSET_TAXIING
    } else if j.attack_loco_expire_frame != 0 {
        j.data.attacking_loco
    } else if j.get_flag(USE_SPECIAL_RETURN_LOCO) {
        j.data.returning_loco
    } else {
        wst
    }
}

/// `setLocomotorGoalNone()`'s override: taking off or landing in the air (not circling), straight on, 1000 ahead; `None` for the
/// base's.
pub fn goal_none_override(j: &Jet, pos: &Coord3D, dir: &Coord3D) -> Option<Coord3D> {
    if (j.get_flag(TAKEOFF_IN_PROGRESS) || j.get_flag(LANDING_IN_PROGRESS)) && j.get_flag(ALLOW_AIR_LOCO) && !j.get_flag(ALLOW_CIRCLING) && !plant(3767) {
        return Some(Coord3D { x: pos.x + dir.x * 1000.0, y: pos.y + dir.y * 1000.0, z: pos.z });
    }
    None
}

/// `getSneakyTargetingOffset(offset)`: while the attackers miss, true, the offset ahead (2-D) by the sneaky offset.
pub fn get_sneaky_targeting_offset<H: AiStateHost>(h: &mut H) -> Option<Coord3D> {
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    let expire = jt(h).attackers_miss_expire_frame;
    if expire != 0 && now < expire {
        let off = jt(h).data.sneaky_offset_when_attacking;
        let dir = h.unit_direction_vector_2d();
        return Some(Coord3D { x: dir.x * off, y: dir.y * off, z: 0.0 });
    }
    None
}

/// `pruneDeadTargeters()`: the targeters no longer there forgotten.
fn prune_dead_targeters<H: AiStateHost>(h: &mut H) {
    let targeters = std::mem::take(&mut jt(h).targeted_by);
    let kept: Vec<ObjectId> = targeters.into_iter().filter(|&t| h.object_exists(t)).collect();
    jt(h).targeted_by = kept;
}

/// `positionLockon()`: the lock-on cursor (if any; untargetable no longer: destroyed) spun in from its initial distance to our
/// bounding circle over the lock-on time, facing us; on each phase's falling edge the tick (and, blinking, shown; else hidden).
fn position_lockon<H: AiStateHost>(h: &mut H) {
    let Some(draw) = jt(h).lockon_drawable else { return };
    if jt(h).untargetable_expire_frame == 0 {
        h.destroy_drawable(draw);
        jt(h).lockon_drawable = None;
        return;
    }
    let d = jt(h).data.clone();
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    let remaining = jt(h).untargetable_expire_frame.wrapping_sub(now);
    let elapsed = d.lockon_time.wrapping_sub(remaining);
    let own = h.position();
    let mut pos = own;
    let frac = remaining as f32 / d.lockon_time as f32;
    let final_dist = h.bounding_circle_radius();
    let dist = final_dist + (d.lockon_initial_dist - final_dist) * frac;
    let angle = d.lockon_angle_spin * frac;
    pos.x += angle.cos() * dist;
    pos.y += angle.sin() * dist;
    // "pos.z is untouched"
    h.rope_call(draw, crate::logic::ai::chinook_ai_update::RopeCall::SetPosition(pos));
    let dx = own.x - pos.x;
    let dy = own.y - pos.y;
    if dx != 0.0 || dy != 0.0 {
        h.drawable_set_orientation(draw, (dy as f64).atan2(dx as f64) as f32);
    }
    // "the Gaussian sum, to avoid keeping a running total"
    let elapsed_time_sum_prev = 0.5 * (elapsed.wrapping_sub(1)) as f32 * elapsed as f32;
    let elapsed_time_sum_curr = elapsed_time_sum_prev + elapsed as f32;
    let factor = d.lockon_freq / d.lockon_time as f32;
    let last_phase = ((factor * elapsed_time_sum_prev) as i32 & 1) != 0;
    let this_phase = ((factor * elapsed_time_sum_curr) as i32 & 1) != 0;
    if last_phase && !this_phase {
        let me = h.object_id();
        h.play_lockon_tick(me);
        if d.lockon_blinky {
            h.lockon_set_hidden(false);
        }
    } else if d.lockon_blinky {
        h.lockon_set_hidden(true);
    }
}

/// `buildLockonDrawableIfNecessary()`: untargetable, the cursor (its template found) made once; then placed.
fn build_lockon_drawable_if_necessary<H: AiStateHost>(h: &mut H) {
    if jt(h).untargetable_expire_frame == 0 {
        return;
    }
    if jt(h).data.lockon_cursor && jt(h).lockon_drawable.is_none() {
        jt(h).lockon_drawable = h.new_rope_drawable();
    }
    position_lockon(h);
}

/// `addTargeter(id, add)`: with a lock-on time, the targeter added (the first making us untargetable for the lock-on time, the
/// cursor shown) or removed (the last ending it).
pub fn add_targeter<H: AiStateHost>(h: &mut H, id: ObjectId, add: bool) {
    let lockon_time = jt(h).data.lockon_time;
    if lockon_time == 0 {
        return;
    }
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    let j = jt(h);
    let found = j.targeted_by.iter().position(|&t| t == id);
    if add {
        if found.is_none() {
            j.targeted_by.push(id);
            if j.untargetable_expire_frame == 0 && j.targeted_by.len() == 1 {
                j.untargetable_expire_frame = now.wrapping_add(lockon_time);
                build_lockon_drawable_if_necessary(h);
            }
        }
    } else if let Some(i) = found {
        j.targeted_by.remove(i);
        if j.targeted_by.is_empty() && !plant(3768) {
            j.untargetable_expire_frame = 0;
        }
    }
}

/// `isTemporarilyPreventingAimSuccess()`.
pub fn is_temporarily_preventing_aim_success<H: AiStateHost>(h: &mut H) -> bool {
    let now = crate::logic::locomotor::LocomotorHost::frame(h);
    let expire = jt(h).untargetable_expire_frame;
    expire != 0 && now < expire
}

/// `isAllowedToMoveAwayFromUnit()`: not parked (or landing, taking off); else the base's (true).
pub fn is_allowed_to_move_away_from_unit(j: &Jet) -> bool {
    if !j.get_flag(ALLOW_AIR_LOCO) || j.get_flag(TAKEOFF_IN_PROGRESS) || j.get_flag(LANDING_IN_PROGRESS) {
        return false; // "parked (or landing) units don't get to do this."
    }
    true
}

/// `getTreatAsAircraftForLocoDistToGoal()`'s override: never while taxiing; `None` for the base's.
pub fn treat_as_aircraft_override(j: &Jet) -> Option<bool> {
    j.get_flag(TAXI_IN_PROGRESS).then_some(false)
}

/// `notifyVictimIsDead()`: a jet that needs a runway returns to base now.
pub fn notify_victim_is_dead<H: AiStateHost>(h: &mut H) {
    if jt(h).data.needs_runway {
        let now = crate::logic::locomotor::LocomotorHost::frame(h);
        jt(h).return_to_base_frame = now;
    }
}

/// `ai->notifyVictimIsDead()` from the attack states (virtual upstream: the base's does nothing, a jet's returns to base).
pub fn notify_victim_is_dead_hook<H: AiStateHost>(h: &mut H) {
    if matches!(ai(h).subclass, AiSubclass::Jet(_)) {
        notify_victim_is_dead(h);
    }
}

/// `doLandingCommand(airfield, src)`: a helipad's landing spot found around it; with its parking place (a space reserved, or a
/// helipad's), our producer now (our space at the old one given up), back to land.
fn do_landing_command<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, airfield: ObjectId, src: CommandSourceType) -> SmResult<()> {
    let helipad = h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD);
    if helipad {
        let mut pos = h.object_position(airfield);
        let max_radius = h.object_bounding_circle_radius(airfield) * 10.0;
        if let Some(tmp) = h.find_position_around_no_source(&pos, 0.0, max_radius) {
            pos = tmp;
        }
        jt(h).landing_pos_for_helipad_stuff = pos;
    }
    // (the airfield's behavior modules: its one parking place, the scene's)
    if !h.parking_place_exists(airfield) {
        return Ok(());
    }
    let me = h.object_id();
    let offset = parking_offset(h);
    if helipad || h.pp_reserve_space(airfield, me, offset, false).is_some() {
        // "if we had a space at another airfield, release it"
        if let Some(old) = producer_pp(h) {
            if old != airfield {
                h.pp_release_space(old, me);
            }
        }
        set_producer(h, airfield);
        set_flag(h, USE_SPECIAL_RETURN_LOCO, false);
        set_flag(h, ALLOW_INTERRUPT_AND_RESUME_OF_CUR_STATE_FOR_RELOAD, false);
        commands::set_last_command_source(h, src);
        aism_set_state(sm, h, RETURNING_FOR_LANDING)?;
    }
    Ok(())
}

/// `isParkedAt(obj)`: grounded, not a helipad's, at that airfield (our producer's parking place).
fn is_parked_at<H: AiStateHost>(h: &mut H, obj: ObjectId) -> bool {
    if !flag(h, ALLOW_AIR_LOCO) && !h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) && obj != INVALID_ID {
        if let Some(airfield) = producer_pp(h) {
            if airfield == obj {
                return true;
            }
        }
    }
    false
}

/// The jet's command members the base's switch reaches (`privateFollowPath` leaving production, `privateEnter`,
/// `privateGetRepaired`): `None` when the command is not one of them.
pub fn jet_private_command<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, parms: &AiCommandParms) -> Option<SmResult<()>> {
    let src = parms.cmd_source;
    match parms.cmd {
        AICMD_FOLLOW_EXITPRODUCTION_PATH => {
            // `privateFollowPath(path, ignoreObject, src, TRUE)`: from the hangar (a helipad's: straight to take off)
            let r = (|| {
                aism_clear(sm, h)?;
                if h.object_exists(parms.obj) {
                    ai(h).ignore_obstacle(parms.obj);
                }
                commands::set_last_command_source(h, src);
                let state = if h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) { TAKING_OFF_AWAIT_CLEARANCE } else { TAXI_FROM_HANGAR };
                aism_set_state(sm, h, state).map(|_| ())
            })();
            Some(r)
        }
        AICMD_FOLLOW_PATH => None,
        AICMD_ENTER | AICMD_GET_REPAIRED => {
            // "we are already landing. just ignore it."
            if flag(h, LANDING_IN_PROGRESS) {
                return Some(Ok(()));
            }
            let target = if h.object_exists(parms.obj) { parms.obj } else { INVALID_ID };
            let allowed = if parms.cmd == AICMD_ENTER { h.action_can_enter_object(target, src) } else { h.action_can_get_repaired_at(target, src) };
            if !allowed {
                return Some(Ok(()));
            }
            if target == INVALID_ID {
                // (upstream's action manager refuses none; a host that answers yes uses it unchecked)
                return Some(Err(StateMachineError::Undefined("a jet's landing at no object")));
            }
            Some(do_landing_command(sm, h, target, src))
        }
        _ => None,
    }
}

/// `aiDoCommand(parms)` (the jet's): the producer's location; refused as the base refuses; kept; taking off or landing, waiting
/// for it to end; idle while reloading, waiting; idle in the air (not a helipad's), back to land; grounded, any command but idle,
/// busy and leaving production taking off first (and one at our own airfield ignored); guards and hunts resumable after reloading;
/// then the base's.
pub fn jet_ai_do_command<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, parms: &AiCommandParms) -> SmResult<()> {
    // "call this from aiDoCommand as well as update, because this can be called before update ever is"
    get_producer_location(h);
    if !commands::is_allowed_to_respond_to_ai_commands(sm, h, parms) {
        return Ok(());
    }
    // "note that we always store this, even if nothing will be "pending"."
    let mut storage = std::mem::take(&mut jt(h).most_recent_command);
    storage.store(h, parms);
    jt(h).most_recent_command = storage;
    if flag(h, TAKEOFF_IN_PROGRESS) || flag(h, LANDING_IN_PROGRESS) {
        // "have to wait for takeoff or landing to complete, just store the sucker"
        set_flag(h, HAS_PENDING_COMMAND, true);
        return Ok(());
    } else if parms.cmd == AICMD_IDLE && sm.get_current_state_id() == RELOAD_AMMO {
        // "uber-special-case... if we are told to idle, but are reloading ammo, ignore it for now"
        set_flag(h, HAS_PENDING_COMMAND, true);
        return Ok(());
    } else if parms.cmd == AICMD_IDLE && { let me = h.object_id(); h.object_is_airborne_target(me) } && !h.is_kind_of(KINDOF_PRODUCED_AT_HELIPAD) {
        restart_in(sm, h, RETURNING_FOR_LANDING, true)?;
        return Ok(());
    } else if !flag(h, ALLOW_AIR_LOCO) {
        let take_off = match parms.cmd {
            AICMD_IDLE | AICMD_BUSY | AICMD_FOLLOW_EXITPRODUCTION_PATH => false,
            AICMD_ENTER | AICMD_GET_REPAIRED => {
                // "if we're already parked at the airfield in question, just ignore."
                let obj = if h.object_exists(parms.obj) { parms.obj } else { INVALID_ID };
                if is_parked_at(h, obj) {
                    return Ok(());
                }
                true
            }
            _ => true,
        };
        if take_off {
            // "nuke any existing pending cmd"
            let mut storage = std::mem::take(&mut jt(h).most_recent_command);
            storage.store(h, parms);
            jt(h).most_recent_command = storage;
            set_flag(h, HAS_PENDING_COMMAND, true);
            restart_in(sm, h, TAKING_OFF_AWAIT_CLEARANCE, true)?;
            return Ok(());
        }
    }
    let resumable = matches!(parms.cmd, AICMD_GUARD_POSITION | AICMD_GUARD_OBJECT | AICMD_GUARD_AREA | AICMD_HUNT | AICMD_GUARD_RETALIATE);
    set_flag(h, ALLOW_INTERRUPT_AND_RESUME_OF_CUR_STATE_FOR_RELOAD, resumable);
    set_flag(h, HAS_PENDING_COMMAND, false);
    commands::base_ai_do_command(sm, h, parms)
}

/// `friend_enableAfterburners(v)`: the afterburner condition, and its sound started (not playing) or stopped (playing).
pub fn enable_afterburners<H: AiStateHost>(h: &mut H, v: bool) {
    let me = h.object_id();
    let handle = jt(h).afterburner_handle;
    if v {
        h.set_model_condition_state(MODELCONDITION_JETAFTERBURNER);
        if !h.audio_is_currently_playing(handle) {
            let new = h.play_per_unit_sound_for(b"Afterburner", me);
            jt(h).afterburner_handle = new;
        }
    } else {
        h.clear_model_condition_state(MODELCONDITION_JETAFTERBURNER);
        if h.audio_is_currently_playing(handle) {
            h.remove_audio_event(handle);
        }
    }
}

/// `friend_getPendingCommandType()`.
pub fn pending_command_type(j: &Jet) -> crate::logic::ai::ai_commands::AiCommandType {
    if j.get_flag(HAS_PENDING_COMMAND) {
        return j.most_recent_command.cmd;
    }
    AICMD_NO_COMMAND
}

/// `JetAIUpdate::xfer`'s own members (version 2, after the base's): the producer's location, the stored command, the frames, the
/// targeters, the lock-on cursor (its template's name; one made on load), the flags, the engines.
pub fn jet_xfer_members<H: AiStateHost>(j: &mut Jet, h: &mut H, xfer: &mut dyn Xfer) -> XferResult<()> {
    xfer.xfer_coord3d(&mut j.producer_location)?;
    j.most_recent_command.do_xfer(h, xfer)?;
    xfer.xfer_unsigned_int(&mut j.attack_loco_expire_frame)?;
    xfer.xfer_unsigned_int(&mut j.attackers_miss_expire_frame)?;
    xfer.xfer_unsigned_int(&mut j.return_to_base_frame)?;
    xfer.xfer_stl_object_id_list(&mut j.targeted_by)?;
    xfer.xfer_unsigned_int(&mut j.untargetable_expire_frame)?;
    // (the cursor's template: "lockon", the only one)
    let mut draw_name: Vec<u8> = if j.lockon_drawable.is_some() { b"lockon".to_vec() } else { Vec::new() };
    xfer.xfer_ascii_string(&mut draw_name)?;
    if !draw_name.is_empty() && j.lockon_drawable.is_none() && draw_name.as_slice() == b"lockon" {
        j.lockon_drawable = h.new_rope_drawable();
    }
    xfer.xfer_int(&mut j.flags)?;
    xfer.xfer_bool(&mut j.engines_on)?;
    Ok(())
}
