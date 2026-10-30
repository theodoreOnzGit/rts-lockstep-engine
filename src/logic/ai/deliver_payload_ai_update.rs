// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/DeliverPayloadAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DeliverPayloadAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `DeliverPayloadAIUpdate` (`p1w13-sonnet`, layer 10, batch A37): the cargo plane's or the bomber's delivery, a second machine
//! (`DeliverPayloadStateMachine`, [`DeliverState`]) beside the AI's: approach the target, deliver (doors open, the contained items
//! dropped or fired one per drop delay, the visible payload created per bone), consider a new approach (fly on far enough to turn,
//! up to the maximum attempts), recover from going off the map, head off the map, clean up (destroyed). The dive and strafe run in
//! the AI's update whatever the state.
//!
//! The members are [`DeliverPayload`], held in [`AiSubclass::DeliverPayload`]; the machine (none until [`deliver_payload`] makes
//! it) is `AiUpdateModule::deliver_machine`, lent to the host ([`AiStateHost::deliver_machine_stack`]) while the AI runs, so that
//! `privateMoveToAndEvacuateAndExit`'s [`deliver_payload_via_module_data`] can remake it. While the machine runs the AI and the AI's
//! machine are lent ([`AiStateHost::ai_stack`], [`AiStateHost::ai_machine_stack`]).
//!
//! # Upstream facts kept
//!
//! - `deliverPayload` deletes the old machine (its destructor exits the current state, `EXIT_RESET`) before it makes the new one
//!   and enters its default state (the approach, which gives the move).
//! - `ConsiderNewApproachState::onExit` sets the locomotor's allow-invalid-position to TRUE ("go back to normal"), as its enter
//!   does; `HeadOffMapState`'s facing direction is not saved; the approach, head-off-map and clean-up states save their version
//!   only.
//! - `xfer` (version 5) saves the data but its strafe FX; the weapon template by name (found again on a load, an empty name keeping
//!   the data's); the decal template whole (`xferRadiusDecalTemplate`); the decal itself not at all (a load clears it); the
//!   machine's snapshot whenever there is a machine, whatever the saved flag said.
//! - `isAllowedToRespondToAiCommands` refuses everything once heading off the map; `getAiFreeToExit` is FREE only while
//!   delivering and alive.
//! - Undefined upstream, refused here as [`StateMachineError::Undefined`]: a locomotor call without a current locomotor, a physics
//!   call without physics, the first item of an empty contain list, a visible payload of an unknown template or with no
//!   controlling player.
//!
//! [`AiUpdateModule::update`]: crate::logic::ai::ai_states::AiUpdateModule::update

use std::convert::Infallible;
use std::marker::PhantomData;

use petir::Real;

use crate::common::base_type::{real_to_int_ceil, Coord3D};
use crate::common::game_common::VeterancyLevel;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::model_state::{MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING};
use crate::common::state_machine::{
    SmResult, StateBehavior, StateConditionInfo, StateExitType, StateId, StateMachine, StateMachineError, StateReturn, INVALID_STATE_ID,
};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferVersion};
use crate::logic::ai::ai_commands as commands;
use crate::logic::ai::ai_states::{ai, AiState, AiStateHost};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::{AI_IDLE, AI_MOVE_TO, CMD_FROM_AI};
use crate::logic::ai::assault_transport_ai_update::MemberCommand;
use crate::logic::locomotor::Locomotor;
use crate::logic::modules::deliver_payload_data::DeliverPayloadData;
use crate::logic::modules::plant;
use crate::logic::radius_decal::{RadiusDecal, RadiusDecalTemplate};
use crate::logic::weapon_set_state::LOCKED_TEMPORARILY;

type Sm<H> = StateMachine<AiState<H>>;

/// `APPROACH`: "Flying towards target".
pub const APPROACH: StateId = 0;
/// `DELIVERING`: "Delivering the payload to the target".
pub const DELIVERING: StateId = 1;
/// `CONSIDER_NEW_APPROACH`: "Deciding if I should reapproach to deliver more payload or go home".
pub const CONSIDER_NEW_APPROACH: StateId = 2;
/// `RECOVER_FROM_OFF_MAP`: "oops, went off the map, special recovery needed".
pub const RECOVER_FROM_OFF_MAP: StateId = 3;
/// `HEAD_OFF_MAP`: "We're all done here, take off into the sunset".
pub const HEAD_OFF_MAP: StateId = 4;
/// `CLEAN_UP`: "Made it to the sunset. Delete peacefully, don't kill".
pub const CLEAN_UP: StateId = 5;

/// `DIVESTATE_PREDIVE` ("Stored in save file as int, don't renumber!").
pub const DIVESTATE_PREDIVE: i32 = 0;
/// `DIVESTATE_DIVING`.
pub const DIVESTATE_DIVING: i32 = 1;
/// `DIVESTATE_POSTDIVE`.
pub const DIVESTATE_POSTDIVE: i32 = 2;

/// `DeliverPayloadAIUpdateModuleData`'s own fields (the delays in frames).
#[derive(Clone, Debug, PartialEq)]
pub struct DeliverPayloadModuleData {
    /// `DoorDelay`.
    pub door_delay: u32,
    /// `DeliveryDistance`: "How far away from target I can unload, plus how far after target I need to turn around at".
    pub max_distance_to_target: f32,
    /// `MaxAttempts`: "How many times I can re-approach".
    pub max_number_attempts: i32,
    /// `DropDelay`.
    pub drop_delay: u32,
    /// `DropOffset`.
    pub drop_offset: Coord3D,
    /// `DropVariance`.
    pub drop_variance: Coord3D,
    /// `PutInContainer`.
    pub put_in_container_name: Vec<u8>,
    /// `DeliveryDecal`.
    pub delivery_decal_template: RadiusDecalTemplate,
    /// `DeliveryDecalRadius`.
    pub delivery_decal_radius: f32,
}

impl Default for DeliverPayloadModuleData {
    /// `DeliverPayloadAIUpdateModuleData()`.
    fn default() -> Self {
        DeliverPayloadModuleData {
            door_delay: 0,
            max_distance_to_target: 0.0,
            max_number_attempts: 0,
            drop_delay: 0,
            drop_offset: Coord3D::default(),
            drop_variance: Coord3D::default(),
            put_in_container_name: Vec::new(),
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
        }
    }
}

/// `DeliverPayloadAIUpdate`'s members (its machine is the module's), with its module data.
#[derive(Clone, Debug, PartialEq)]
pub struct DeliverPayload {
    /// The module data's own fields.
    pub module_data: DeliverPayloadModuleData,
    /// `m_targetPos`: "Where I plan to deliver my little friends, if obj is null".
    pub target_pos: Coord3D,
    /// `m_moveToPos`: "Where I am moving to."
    pub move_to_pos: Coord3D,
    /// `m_data`.
    pub data: DeliverPayloadData,
    /// `m_visibleItemsDelivered`.
    pub visible_items_delivered: i32,
    /// `m_deliveryDecal`.
    pub delivery_decal: RadiusDecal,
    /// `m_previousDistanceSqr`.
    pub previous_distance_sqr: f32,
    /// `m_freeToExit`.
    pub free_to_exit: bool,
    /// `m_acceptingCommands`.
    pub accepting_commands: bool,
    /// `m_diveState` (a `DiveState`; as loaded, any value).
    pub dive_state: i32,
}

impl DeliverPayload {
    /// `DeliverPayloadAIUpdate(thing, moduleData)`'s members.
    pub fn new(module_data: DeliverPayloadModuleData) -> Self {
        DeliverPayload {
            module_data,
            target_pos: Coord3D::default(),
            move_to_pos: Coord3D::default(),
            data: DeliverPayloadData::default(),
            visible_items_delivered: 0,
            delivery_decal: RadiusDecal::default(),
            previous_distance_sqr: 0.0,
            free_to_exit: false,
            accepting_commands: true,
            dive_state: DIVESTATE_PREDIVE,
        }
    }

    /// `getAiFreeToExit(exiter)`: not while dead; FREE only while delivering.
    pub fn ai_free_to_exit(&self, effectively_dead: bool) -> bool {
        !effectively_dead && self.free_to_exit
    }
}

/// `DeliverPayloadStateMachine`'s states, with their members.
pub enum DeliverState<H: AiStateHost> {
    /// `ApproachState`.
    Approach,
    /// `DeliveringState`: `m_dropDelayLeft`, `m_didOpen`.
    Delivering { drop_delay_left: u32, did_open: bool },
    /// `ConsiderNewApproachState`: `m_numberEntriesToState`.
    ConsiderNewApproach { number_entries_to_state: i32 },
    /// `RecoverFromOffMapState`: `m_reEntryFrame`.
    RecoverFromOffMap { re_entry_frame: u32 },
    /// `HeadOffMapState`: `facingDirectionUponDelivery`.
    HeadOffMap { facing_direction_upon_delivery: Coord3D },
    /// `CleanUpState`.
    CleanUp,
    /// (names the host's type; never made)
    _Host(Infallible, PhantomData<fn() -> H>),
}

/// `DeliverPayloadStateMachine`'s one transition condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliverCondition {
    /// `isOffMap`.
    IsOffMap,
}

/// The running AI's `DeliverPayloadAIUpdate` members.
fn dp<H: AiStateHost>(h: &mut H) -> &mut DeliverPayload {
    match &mut ai(h).subclass {
        AiSubclass::DeliverPayload(d) => d,
        _ => unreachable!("a DeliverPayloadAIUpdate"),
    }
}

/// `sqr(x)` (`Real`).
fn sqr(x: f32) -> f32 {
    x * x
}

/// The current locomotor (`getCurLocomotor()`), undefined upstream when there is none.
fn cur_loco<H: AiStateHost>(h: &mut H) -> SmResult<&mut Locomotor> {
    let a = ai(h);
    match a.cur_locomotor.and_then(|c| a.locomotor_set.get_mut(c)) {
        Some(l) => Ok(l),
        None => Err(StateMachineError::Undefined("a delivery's locomotor call without a locomotor")),
    }
}

/// `getPhysics()` of the owner, undefined upstream when there is none.
fn need_physics<H: AiStateHost>(h: &mut H) -> SmResult<()> {
    if !h.has_physics() {
        return Err(StateMachineError::Undefined("a delivery's physics call without physics"));
    }
    Ok(())
}

/// Runs `f` with the AI's machine `sm` lent to the host (the deliver machine's states reach it there).
fn lend_ai_machine<H: AiStateHost, R>(sm: &mut Sm<H>, h: &mut H, f: impl FnOnce(&mut H) -> R) -> R {
    let m = std::mem::take(sm);
    h.ai_machine_stack().push(m);
    let r = f(h);
    *sm = h.ai_machine_stack().pop().expect("the lent machine");
    r
}

/// `ai->aiMoveToPosition(pos, CMD_FROM_AI)` (the AI's machine lent).
fn ai_move_to<H: AiStateHost>(h: &mut H, pos: &Coord3D) -> SmResult<()> {
    crate::logic::ai::supply_truck_ai_update::with_ai_machine(h, |sm, h| commands::ai_move_to_position(sm, h, pos, CMD_FROM_AI))
}

/// `ai->getAIStateType()`.
fn ai_state_type<H: AiStateHost>(h: &mut H) -> StateId {
    crate::logic::ai::supply_truck_ai_update::with_ai_machine(h, |sm, _| sm.get_current_state_id())
}

/// `"%s%02d"` of a base name and a number (`AsciiString::format`).
pub fn numbered_name(base: &[u8], n: i32) -> Vec<u8> {
    let mut v = base.to_vec();
    v.extend_from_slice(format!("{n:02}").as_bytes());
    v
}

/// `DeliverPayloadStateMachine(owner)`: its six states (the approach first: the default state).
pub fn make_deliver_machine<H: AiStateHost>() -> SmResult<StateMachine<DeliverState<H>>> {
    let mut m = StateMachine::new();
    m.define_state(APPROACH, DeliverState::Approach, DELIVERING, CONSIDER_NEW_APPROACH, &[])?;
    m.define_state(DELIVERING, DeliverState::Delivering { drop_delay_left: 0, did_open: false }, HEAD_OFF_MAP, CONSIDER_NEW_APPROACH, &[])?;
    m.define_state(
        CONSIDER_NEW_APPROACH,
        DeliverState::ConsiderNewApproach { number_entries_to_state: 0 },
        APPROACH,
        HEAD_OFF_MAP,
        &[StateConditionInfo { test: DeliverCondition::IsOffMap, to_state_id: RECOVER_FROM_OFF_MAP }],
    )?;
    m.define_state(RECOVER_FROM_OFF_MAP, DeliverState::RecoverFromOffMap { re_entry_frame: 0 }, APPROACH, APPROACH, &[])?;
    m.define_state(HEAD_OFF_MAP, DeliverState::HeadOffMap { facing_direction_upon_delivery: Coord3D::default() }, CLEAN_UP, CLEAN_UP, &[])?;
    m.define_state(CLEAN_UP, DeliverState::CleanUp, INVALID_STATE_ID, INVALID_STATE_ID, &[])?;
    Ok(m)
}

/// `isOffMap()`: outside the extent including the border (on its edge counts as outside).
pub fn is_off_map<H: AiStateHost>(h: &mut H) -> bool {
    let (lo, hi) = h.terrain_extent_including_border();
    let p = h.position();
    !((lo.x < p.x) && (p.x < hi.x) && (lo.y < p.y) && (p.y < hi.y)) || plant(3680)
}

/// `isCloseEnoughToTarget()`: within the allowed distance (plus the pre-open distance while inbound); the distance remembered.
pub fn is_close_enough_to_target<H: AiStateHost>(h: &mut H) -> bool {
    let target = dp(h).target_pos;
    let allowed = dp(h).data.dist_to_target;
    let mut allowed_distance_sqr = sqr(allowed);
    let current_distance_sqr = h.distance_squared_to_position_2d(&target);
    let d = dp(h);
    let in_bound = d.previous_distance_sqr > current_distance_sqr;
    d.previous_distance_sqr = current_distance_sqr;
    if in_bound && !plant(3681) {
        allowed_distance_sqr = sqr(allowed + d.data.pre_open_distance);
    }
    allowed_distance_sqr > current_distance_sqr
}

/// `calcMinTurnRadius(&timeToTravelThatDist)`: the maximum speed over the maximum turn rate (none: 999999), and the time to travel it.
pub fn calc_min_turn_radius<H: AiStateHost>(h: &mut H) -> SmResult<(f32, f32)> {
    let bdt = h.damage_state();
    cur_loco(h)?;
    let a = h.ai_stack().pop().expect("a running AI");
    let l = a.locomotor_set.get(a.cur_locomotor.expect("the locomotor")).expect("the locomotor");
    let max_speed = l.get_max_speed_for_condition(h, bdt);
    let max_turn_rate = l.get_max_turn_rate(h, bdt);
    h.ai_stack().push(a);
    let min_turn_radius = if max_turn_rate > 0.0 && !plant(3682) { max_speed / max_turn_rate } else { 999999.0 };
    Ok((min_turn_radius, min_turn_radius / max_speed))
}

/// `getPutInContainerTemplateViaModuleData()`: the named template, if found (none for no name).
pub fn put_in_container_template<H: AiStateHost>(h: &mut H, d: &DeliverPayload) -> Option<u32> {
    let name = &d.module_data.put_in_container_name;
    if name.is_empty() {
        return None;
    }
    h.thing_template_by_name(name)
}

/// `deliverPayload(moveToPos, targetPos, data)` ("THIS GETS CALLED VIA OBJECT CREATION LISTS ONLY!!!"): the old machine deleted,
/// the members set, the decal made, the visible payload's subobjects shown, then the new machine made and its default state entered.
pub fn deliver_payload<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, move_to_pos: &Coord3D, target_pos: &Coord3D, data: &DeliverPayloadData) -> SmResult<()> {
    let old = h.deliver_machine_stack().last_mut().expect("the deliver machine, lent").take();
    if let Some(old) = old {
        lend_ai_machine(sm, h, |h| old.destroy(h))?;
    }
    {
        let d = dp(h);
        d.move_to_pos = *move_to_pos;
        d.target_pos = *target_pos;
        d.data = data.clone();
    }
    // m_deliveryDecal.clear(); createRadiusDecal(*targetPos, radius, getControllingPlayer(), m_deliveryDecal)
    kill_delivery_decal(h);
    let has_player = h.controlling_player_type().is_some();
    let (template, radius) = (dp(h).data.delivery_decal_template.clone(), dp(h).data.delivery_decal_radius);
    h.delivery_decal_create(&template, target_pos, radius, has_player);
    dp(h).delivery_decal.empty = !(has_player && !template.name.is_empty() && radius > 0.0);
    let d = dp(h);
    // "We can't dive, so setting it to post-dive will prevent dive logic from being processed."
    d.dive_state = if d.data.dive_start_distance <= 0.0 && !plant(3683) { DIVESTATE_POSTDIVE } else { DIVESTATE_PREDIVE };
    d.visible_items_delivered = 0;
    // "If we have visible bones, then show each of those subobjects."
    if h.has_drawable() {
        let (n, sub) = (dp(h).data.visible_num_bones, dp(h).data.visible_sub_object_name.clone());
        for i in 1..=n {
            if !sub.is_empty() {
                h.drawable_show_sub_object(&numbered_name(&sub, i), true);
            }
        }
    }
    // "must make the state machine AFTER initing the other stuff, since it may inquire of its values..."
    let mut m = make_deliver_machine()?;
    let r = lend_ai_machine(sm, h, |h| m.init_default_state(h));
    *h.deliver_machine_stack().last_mut().expect("the deliver machine, lent") = Some(m);
    r.map(|_| ())
}

/// `deliverPayloadViaModuleData(moveToPos)` ("THIS GETS CALLED FOR SCRIPTED REINFORCEMENTS ONLY!!!"): a delivery of the module
/// data's offset, variance, distance, attempts, drop delay and decal, to the move position.
pub fn deliver_payload_via_module_data<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, move_to_pos: &Coord3D) -> SmResult<()> {
    let md = dp(h).module_data.clone();
    let data = DeliverPayloadData {
        drop_offset: md.drop_offset,
        drop_variance: md.drop_variance,
        dist_to_target: md.max_distance_to_target,
        max_attempts: if plant(3695) { DeliverPayloadData::default().max_attempts } else { md.max_number_attempts },
        drop_delay: md.drop_delay,
        delivery_decal_template: md.delivery_decal_template,
        delivery_decal_radius: md.delivery_decal_radius,
        ..Default::default()
    };
    deliver_payload(sm, h, move_to_pos, move_to_pos, &data)
}

/// `killDeliveryDecal()` (`m_deliveryDecal.clear()`).
pub fn kill_delivery_decal<H: AiStateHost>(h: &mut H) {
    dp(h).delivery_decal.empty = true;
    h.delivery_decal_clear();
}

/// The update's dive: within the start distance (2-D) it dives (the locomotor's precise z, the "StartDive" sound); within the end
/// distance (3-D) it stops; while diving, a strafing weapon fires at a point that runs along the approach (the slot locked, the
/// strafe FX there) while not climbing fast.
pub fn deliver_dive<H: AiStateHost>(h: &mut H) -> SmResult<()> {
    let d = dp(h);
    if d.dive_state == DIVESTATE_POSTDIVE {
        return Ok(());
    }
    let target = d.target_pos;
    if d.dive_state == DIVESTATE_PREDIVE {
        // "Check to see if we are close enough to start diving."
        let start_dive_distance_squared = sqr(d.data.dive_start_distance);
        let current_distance_squared = h.distance_squared_to_position_2d(&target);
        if current_distance_squared <= start_dive_distance_squared {
            dp(h).dive_state = DIVESTATE_DIVING;
            cur_loco(h)?.set_use_precise_z_pos(true);
            let pos = h.position();
            if !plant(3684) {
                h.play_per_unit_sound_at(b"StartDive", &pos);
            }
        }
        return Ok(());
    }
    // "Check to see when we shall end diving"
    let end_dive_distance_squared = sqr(d.data.dive_end_distance);
    let current_distance_squared = h.distance_squared_3d(&target);
    if current_distance_squared <= end_dive_distance_squared {
        dp(h).dive_state = DIVESTATE_POSTDIVE;
        cur_loco(h)?.set_use_precise_z_pos(false);
    }
    let slot = dp(h).data.strafing_weapon_slot;
    if slot != -1 && !plant(3685) {
        // "SHOOT!!"
        need_physics(h)?;
        if h.velocity().z < 5.0 {
            // "Calc strafe ratio"
            let start_dive_distance = dp(h).data.dive_start_distance;
            let end_dive_distance = end_dive_distance_squared.sqrt();
            let current_distance = current_distance_squared.sqrt();
            let dive_ratio = (start_dive_distance - current_distance) / (start_dive_distance - end_dive_distance);
            let mut velocity = h.velocity();
            velocity.z = 0.0;
            velocity.normalize();
            velocity.scale(dive_ratio * 100.0);
            let mut backwards = velocity;
            backwards.scale(0.33);
            let mut strafe_point = target;
            strafe_point.sub(&backwards);
            strafe_point.add(&velocity);
            strafe_point.z = h.ground_height(strafe_point.x, strafe_point.y);
            // "lock it just till the weapon is empty or the attack is "done""
            h.set_weapon_lock(slot, LOCKED_TEMPORARILY);
            h.fire_current_weapon_at_position(&strafe_point);
            let fx = dp(h).data.strafe_fx.clone();
            h.fx_do_pos(&fx, &strafe_point);
        }
    }
    Ok(())
}

/// The approach's update: delivering when close enough; idle (told so from outside) is a failure; another state than the move
/// gets the move again.
fn approach_update<H: AiStateHost>(h: &mut H) -> SmResult<StateReturn> {
    if ai(h).is_ai_in_dead_state() {
        return Ok(StateReturn::FAILURE);
    }
    if is_close_enough_to_target(h) {
        return Ok(StateReturn::SUCCESS);
    }
    let t = ai_state_type(h);
    if t != AI_MOVE_TO {
        if t == AI_IDLE {
            // "Because something outside us has told us to IDLE, so geez, ignore it and try our approach again"
            return Ok(StateReturn::FAILURE);
        }
        // (a DEBUG_CRASH: "hmm, bailed from moveto state early... should this be possible?")
        let pos = dp(h).move_to_pos;
        if !plant(3686) {
            ai_move_to(h, &pos)?;
        }
    }
    Ok(StateReturn::CONTINUE)
}

/// The delivering state's update ("Kick a dude out every so often"): after the door (then drop) delay, free to exit; away from the
/// target a failure; out of contained and visible payload a success; else the first item dropped (or fired), and the visible
/// payload's next ones created.
fn delivering_update<H: AiStateHost>(sm: &mut StateMachine<DeliverState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if let Some(DeliverState::Delivering { drop_delay_left, .. }) = sm.state_mut(id) {
        if *drop_delay_left > 0 {
            *drop_delay_left -= 1;
            return Ok(StateReturn::CONTINUE);
        }
    }
    dp(h).free_to_exit = true;
    let drop_delay = dp(h).data.drop_delay;
    if let Some(DeliverState::Delivering { drop_delay_left, did_open }) = sm.state_mut(id) {
        *did_open = true;
        if !plant(3687) {
            *drop_delay_left = drop_delay;
        }
    }
    if !is_close_enough_to_target(h) {
        return Ok(StateReturn::FAILURE);
    }
    let me = h.object_id();
    let items = if h.has_contain() { Some(h.contain_items(me)) } else { None };
    let d = dp(h);
    if items.as_ref().is_none_or(|v| v.is_empty()) && d.visible_items_delivered == d.data.visible_num_bones {
        // "We are out of payload to drop AND our visible payload is empty."
        return Ok(StateReturn::SUCCESS);
    }
    // "Handle contained payload delivery."
    if let Some(items) = items {
        let Some(&item) = items.first() else {
            return Err(StateMachineError::Undefined("a delivery's first item of an empty contain list"));
        };
        deliver_item(h, item)?;
    }
    // "Handle visible payload delivery."
    if dp(h).visible_items_delivered < dp(h).data.visible_num_bones {
        let mut attempt_drops = dp(h).data.visible_items_dropped_per_interval;
        let draw = h.has_drawable();
        let mut update_sub_objects = false;
        while attempt_drops != 0 && dp(h).visible_items_delivered < dp(h).data.visible_num_bones {
            // "Drop an item!"
            if draw {
                let n = dp(h).visible_items_delivered + 1;
                let sub = dp(h).data.visible_sub_object_name.clone();
                if !sub.is_empty() {
                    // "First hide the subobject as it becomes the delivered payload."
                    h.drawable_show_sub_object(&numbered_name(&sub, n), false);
                    update_sub_objects = true;
                }
                if !dp(h).data.visible_payload_template_name.is_empty() {
                    // "Second, create the subobject for real"
                    create_visible_payload(h, n)?;
                }
            }
            // "Update the counters"
            attempt_drops -= 1;
            dp(h).visible_items_delivered += 1;
        }
        if update_sub_objects {
            h.drawable_update_sub_objects();
        }
    }
    Ok(StateReturn::CONTINUE)
}

/// One contained item delivered: fired at the target (and destroyed) for a weapon delivery; else told to exit, put at its position
/// (with the variance) plus the drop offset, sent at the target (parachuting directly) or the move position, its minefield and
/// smart bomb aimed, and pushed with the owner's velocity.
fn deliver_item<H: AiStateHost>(h: &mut H, item: ObjectId) -> SmResult<()> {
    let d = dp(h).clone();
    if d.data.fire_weapon && !plant(3688) {
        let mut pos = d.target_pos;
        pos.x += d.data.drop_offset.x;
        pos.y += d.data.drop_offset.y;
        pos.z += d.data.drop_offset.z;
        h.fire_current_weapon_at_position(&pos);
        h.object_destroy(item);
        return Ok(());
    }
    let item_ai = h.object_has_ai(item);
    if item_ai {
        h.member_ai_command(item, MemberCommand::Exit { transport: INVALID_ID, src: CMD_FROM_AI });
    }
    let mut pos = h.object_position(item);
    let var = d.data.drop_variance;
    if var.x > 0.0 {
        pos.x += h.logic_random_value_real(-var.x, var.x);
    }
    if var.y > 0.0 {
        pos.y += h.logic_random_value_real(-var.y, var.y);
    }
    if var.z > 0.0 {
        pos.z += h.logic_random_value_real(-var.z, var.z);
    }
    pos.x += d.data.drop_offset.x;
    pos.y += d.data.drop_offset.y;
    pos.z += d.data.drop_offset.z;
    h.object_set_position(item, &pos);
    if d.data.is_parachute_directly && h.object_has_contain(item) {
        // "go for the direct hit"
        h.object_contain_set_override_destination(item, &d.target_pos);
    } else if item_ai {
        // "This will most likely be ignored, as the item is still in the transport, and therefore Held."
        h.member_ai_command(item, MemberCommand::MoveToPosition { pos: d.move_to_pos, src: CMD_FROM_AI });
    }
    h.object_aim_update_module(item, ItemModule::GenerateMinefield, &d.move_to_pos);
    h.object_aim_update_module(item, ItemModule::SmartBombTargetHoming, &d.move_to_pos);
    if d.data.inherit_transport_velocity {
        need_physics(h)?;
        let velocity = h.velocity();
        if !h.object_has_physics(item) {
            return Err(StateMachineError::Undefined("a delivered item's physics call without physics"));
        }
        h.object_apply_force(item, &velocity);
    }
    Ok(())
}

/// The modules of a delivered item its delivery aims (`findUpdateModule`), each if it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemModule {
    /// `GenerateMinefieldBehavior::setMinefieldTarget`.
    GenerateMinefield,
    /// `SmartBombTargetHomingUpdate::SetTargetPosition`.
    SmartBombTargetHoming,
}

/// The visible payload's `n`th item made: its producer, at its drop bone (else the owner's position), the owner's orientation,
/// the owner's velocity inherited (a motive force, and set back by it); a projectile fired at the target with the visible payload
/// weapon (none: nothing), else pitched and moved to the move position.
fn create_visible_payload<H: AiStateHost>(h: &mut H, n: i32) -> SmResult<()> {
    let d = dp(h).clone();
    if h.thing_template_by_name(&d.data.visible_payload_template_name).is_none() {
        return Err(StateMachineError::Undefined("a delivery's visible payload of an unknown template"));
    }
    if h.controlling_player_type().is_none() {
        return Err(StateMachineError::Undefined("a delivery's visible payload with no controlling player"));
    }
    let payload = h.payload_new(&d.data.visible_payload_template_name);
    h.payload_set_producer(payload);
    let mut placed = false;
    if !d.data.visible_drop_bone_name.is_empty() {
        if let Some(bone) = h.drawable_pristine_bone_position(&d.data.visible_drop_bone_name, n) {
            let world = h.drawable_convert_bone_pos_to_world_pos(&bone);
            h.payload_set_position(payload, &world);
            placed = true;
        }
    }
    if !placed {
        let pos = h.position();
        h.payload_set_position(payload, &pos);
    }
    let angle = h.orientation();
    h.payload_set_orientation(payload, angle);
    // "If we want the payload to inherit the transport's velocity, do so now."
    if d.data.inherit_transport_velocity {
        need_physics(h)?;
        let mut starting_force = h.velocity();
        starting_force.scale(h.payload_mass(payload));
        h.payload_apply_motive_force(payload, &starting_force);
        let mut back_position = h.velocity();
        back_position.scale(-1.0);
        back_position.add(&h.payload_position(payload));
        h.payload_set_position(payload, &back_position);
    }
    // "Are we firing a missile?"
    let mut projectile_fired = false;
    if h.payload_is_projectile(payload) {
        // "Missile!" (no weapon template: a DEBUG_ASSERTCRASH, and nothing)
        if let Some(weapon) = d.data.visible_payload_weapon_template {
            let v = h.veterancy_level().unwrap_or(VeterancyLevel::Regular);
            let exhaust = h.weapon_template_projectile_exhaust(weapon, v);
            h.payload_projectile_fire(payload, &d.target_pos, weapon, exhaust);
            projectile_fired = true;
        }
    }
    // "If not, are we dropping a bomb?"
    if !projectile_fired {
        if d.data.exit_pitch_rate != 0.0 {
            let pitch_rate = h.payload_center_of_mass_offset(payload) * d.data.exit_pitch_rate;
            h.payload_set_pitch_rate(payload, pitch_rate);
        }
        if h.payload_has_ai(payload) {
            h.payload_ai_move_to_position(payload, &d.move_to_pos);
        }
    }
    Ok(())
}

/// The considering state's `onEnter` ("Increment local counter o' futility"): over the attempts a failure; else away, far enough to
/// turn round (2.2 turn radii straight on), off-map positions allowed.
fn consider_on_enter<H: AiStateHost>(sm: &mut StateMachine<DeliverState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let mut count = 0;
    if let Some(DeliverState::ConsiderNewApproach { number_entries_to_state }) = sm.state_mut(id) {
        *number_entries_to_state += 1;
        count = *number_entries_to_state;
    }
    if count > dp(h).data.max_attempts && !plant(3689) {
        // "Too many approaches! Time to give up."
        return Ok(StateReturn::FAILURE);
    }
    let (min_turn_radius, _) = calc_min_turn_radius(h)?;
    // "we must be at least 2*radius dist away from our target. (we add a little fudge ...)"
    const DIST_FUDGE: f32 = 2.2;
    let min_re_approach_dist = min_turn_radius * DIST_FUDGE;
    let dir = h.unit_direction_vector_2d();
    let pos = h.position();
    let re_approach_point = Coord3D { x: pos.x + dir.x * min_re_approach_dist, y: pos.y + dir.y * min_re_approach_dist, z: 0.0 };
    ai_move_to(h, &re_approach_point)?;
    // "we allow these guys to go to invalid (ie, off-map) positions"
    cur_loco(h)?.set_allow_invalid_position(true);
    Ok(StateReturn::CONTINUE)
}

/// The recovering state's `onEnter`: hold in place, a re-entry frame a turn's travel time away, momentum killed, out of the
/// partition and hidden.
fn recover_on_enter<H: AiStateHost>(sm: &mut StateMachine<DeliverState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    // "have him hold in place, if possible."
    let pos = h.position();
    ai_move_to(h, &pos)?;
    // "a little cheesy... make a delay based on turn-radius time to simulate offscreen maneuvering."
    let (_, time_to_travel_that_dist) = calc_min_turn_radius(h)?;
    let frame = crate::logic::locomotor::LocomotorHost::frame(h).wrapping_add_signed(real_to_int_ceil(time_to_travel_that_dist));
    if let Some(DeliverState::RecoverFromOffMap { re_entry_frame }) = sm.state_mut(id) {
        *re_entry_frame = frame;
    }
    // "kill its momentum"
    need_physics(h)?;
    h.physics_reset_dynamic();
    h.partition_unregister();
    h.set_drawable_hidden(true);
    Ok(StateReturn::CONTINUE)
}

/// The recovering state's update: at the re-entry frame back in the partition and shown, at the extent's closest edge point
/// (keeping its height if above the terrain), facing the move position, momentum killed.
fn recover_update<H: AiStateHost>(sm: &mut StateMachine<DeliverState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    let re_entry_frame = match sm.state(id) {
        Some(DeliverState::RecoverFromOffMap { re_entry_frame }) => *re_entry_frame,
        _ => 0,
    };
    if crate::logic::locomotor::LocomotorHost::frame(h) < re_entry_frame && !plant(3690) {
        return Ok(StateReturn::CONTINUE);
    }
    h.partition_register();
    h.set_drawable_hidden(false);
    let pos = h.position();
    let mut enter_coord = h.terrain_find_closest_edge_point(&pos);
    if h.is_above_terrain() {
        enter_coord.z = h.position().z;
    }
    h.set_position(&enter_coord);
    let m = dp(h).move_to_pos;
    // double: `atan2` of two `Real` differences, the C function
    let enter_angle = Real::atan2(f64::from(m.y - enter_coord.y), f64::from(m.x - enter_coord.x)) as f32;
    h.set_orientation(enter_angle);
    need_physics(h)?;
    h.physics_reset_dynamic();
    Ok(StateReturn::SUCCESS)
}

/// The heading-off state's `onEnter` ("Give move order out of town"): the decal killed; a self-destructing deliverer destroyed;
/// else straight on 1.2 map diagonals, off-map positions allowed, ultra-accurate, no more commands; the facing noted.
fn head_off_on_enter<H: AiStateHost>(sm: &mut StateMachine<DeliverState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    kill_delivery_decal(h);
    if dp(h).data.self_destruct_object && !plant(3691) {
        // "this makes delivery unit pop away like a bubble..."
        h.destroy_object();
        return Ok(StateReturn::CONTINUE);
    }
    // "just keep moving straight ahead till we exit the map."
    let mut exit_coord = h.position();
    let dir = h.unit_direction_vector_2d();
    let (lo, hi) = h.terrain_extent();
    const FUDGE: f32 = 1.2;
    // double: `FUDGE * sqrt(...)`, the C function's double times the `Real`
    let huge_dist = (f64::from(FUDGE) * Real::sqrt(f64::from(sqr(hi.x - lo.x) + sqr(hi.y - lo.y)))) as f32;
    exit_coord.x += dir.x * huge_dist;
    exit_coord.y += dir.y * huge_dist;
    let l = cur_loco(h)?;
    l.set_allow_invalid_position(true);
    // "set ultra-accurate just so AI won't try to adjust our dest"
    l.set_ultra_accurate(true);
    ai_move_to(h, &exit_coord)?;
    // "once we get into head-off-map state, we're done... don't respond to anything else."
    dp(h).accepting_commands = false;
    let facing = h.unit_direction_vector_3d();
    if let Some(DeliverState::HeadOffMap { facing_direction_upon_delivery }) = sm.state_mut(id) {
        *facing_direction_upon_delivery = facing;
    }
    Ok(StateReturn::CONTINUE)
}

/// The heading-off state's update: off the map a success; turned away from the facing it set out with (while turning), killed
/// ("rather than face eternally spinning on the point of a mineret").
fn head_off_update<H: AiStateHost>(sm: &mut StateMachine<DeliverState<H>>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if is_off_map(h) {
        return Ok(StateReturn::SUCCESS);
    }
    need_physics(h)?;
    if h.physics_turning() != 0 {
        let f = match sm.state(id) {
            Some(DeliverState::HeadOffMap { facing_direction_upon_delivery }) => *facing_direction_upon_delivery,
            _ => Coord3D::default(),
        };
        let c = h.unit_direction_vector_3d();
        let dot = f.x * c.x + f.y * c.y + f.z * c.z;
        if dot < 0.3 && !plant(3692) {
            h.kill_self();
        }
    }
    Ok(StateReturn::CONTINUE)
}

impl<H: AiStateHost> StateBehavior for DeliverState<H> {
    type Ctx = H;
    type Condition = DeliverCondition;

    fn on_enter(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(DeliverState::Approach) => {
                // "Give the move command"
                if ai(h).is_ai_in_dead_state() {
                    return Ok(StateReturn::FAILURE);
                }
                let pos = dp(h).move_to_pos;
                ai_move_to(h, &pos)?;
            }
            Some(DeliverState::Delivering { .. }) => {
                // "Open the pod bay doors, Hal" ("for now, this assumes at most one door")
                // (`Object::clearAndSetModelConditionState`: the drawable's, if any)
                if h.has_drawable() {
                    h.clear_and_set_model_condition_flags(&[MODELCONDITION_DOOR_1_CLOSING], MODELCONDITION_DOOR_1_OPENING);
                }
                let door_delay = dp(h).module_data.door_delay;
                if let Some(DeliverState::Delivering { drop_delay_left, did_open }) = sm.state_mut(id) {
                    *drop_delay_left = door_delay;
                    *did_open = false;
                }
            }
            Some(DeliverState::ConsiderNewApproach { .. }) => return consider_on_enter(sm, h, id),
            Some(DeliverState::RecoverFromOffMap { .. }) => return recover_on_enter(sm, h, id),
            Some(DeliverState::HeadOffMap { .. }) => return head_off_on_enter(sm, h, id),
            Some(DeliverState::CleanUp) => {
                // "Delete my successful butt" (the contain's count asserted: a DEBUG_ASSERTCRASH)
                h.destroy_object();
            }
            Some(DeliverState::_Host(never, _)) => match *never {},
            None => {}
        }
        Ok(StateReturn::CONTINUE)
    }

    fn on_exit(sm: &mut StateMachine<Self>, h: &mut H, id: StateId, _status: StateExitType) -> SmResult<()> {
        match sm.state(id) {
            Some(DeliverState::Delivering { .. }) => {
                // "Close the doors"
                dp(h).free_to_exit = false;
                // (`Object::clearAndSetModelConditionState`: the drawable's, if any)
                if h.has_drawable() {
                    h.clear_and_set_model_condition_flags(&[MODELCONDITION_DOOR_1_OPENING], MODELCONDITION_DOOR_1_CLOSING);
                }
            }
            Some(DeliverState::ConsiderNewApproach { .. }) => {
                // "go back to normal." (TRUE, as upstream)
                cur_loco(h)?.set_allow_invalid_position(true);
            }
            _ => {}
        }
        Ok(())
    }

    fn update(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(DeliverState::Approach) => approach_update(h),
            Some(DeliverState::Delivering { .. }) => delivering_update(sm, h, id),
            // "stay in this state 'till we reach our goal."
            Some(DeliverState::ConsiderNewApproach { .. }) => {
                Ok(if ai_state_type(h) != AI_MOVE_TO { StateReturn::SUCCESS } else { StateReturn::CONTINUE })
            }
            Some(DeliverState::RecoverFromOffMap { .. }) => recover_update(sm, h, id),
            Some(DeliverState::HeadOffMap { .. }) => head_off_update(sm, h, id),
            _ => Ok(StateReturn::CONTINUE),
        }
    }

    fn test(_sm: &mut StateMachine<Self>, h: &mut H, _id: StateId, condition: DeliverCondition) -> SmResult<bool> {
        Ok(match condition {
            DeliverCondition::IsOffMap => is_off_map(h),
        })
    }

    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// Each its version (1), then the delivering state's delay and opened flag, the considering state's count, the recovering
    /// state's frame.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        match self {
            DeliverState::Delivering { drop_delay_left, did_open } => {
                xfer.xfer_unsigned_int(drop_delay_left)?;
                xfer.xfer_bool(did_open)?;
            }
            DeliverState::ConsiderNewApproach { number_entries_to_state } => xfer.xfer_int(number_entries_to_state)?,
            DeliverState::RecoverFromOffMap { re_entry_frame } => xfer.xfer_unsigned_int(re_entry_frame)?,
            _ => {}
        }
        Ok(())
    }

    fn load_post_process(&mut self) {}
}

/// `DeliverPayloadStateMachine::xfer`: its version (1), then the machine's.
pub fn deliver_machine_xfer<H: AiStateHost>(m: &mut StateMachine<DeliverState<H>>, xfer: &mut dyn Xfer) -> SmResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    m.xfer_machine(xfer)
}

/// `RadiusDecalTemplate::xferRadiusDecalTemplate(xfer)`: its version (1), the texture, the style (4 bytes), the opacities, the
/// throb time, the colour, the owner-only flag.
pub fn xfer_radius_decal_template(t: &mut RadiusDecalTemplate, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_ascii_string(&mut t.name)?;
    let mut b = t.shadow_type.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    t.shadow_type = u32::from_le_bytes(b);
    xfer.xfer_real(&mut t.min_opacity)?;
    xfer.xfer_real(&mut t.max_opacity)?;
    xfer.xfer_unsigned_int(&mut t.opacity_throb_time)?;
    xfer.xfer_int(&mut t.color)?;
    xfer.xfer_bool(&mut t.only_visible_to_owning_player)
}

/// `DeliverPayloadAIUpdate::xfer`'s members after the base's (`version` its version): the positions, the visible count, the dive
/// state, the data (a copy, put back whole), then (the machine's snapshot is the caller's) the decal (a load clears it).
pub fn deliver_xfer_members<H: AiStateHost>(d: &mut DeliverPayload, h: &mut H, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
    xfer.xfer_coord3d(&mut d.target_pos)?;
    xfer.xfer_coord3d(&mut d.move_to_pos)?;
    xfer.xfer_int(&mut d.visible_items_delivered)?;
    let mut b = d.dive_state.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    d.dive_state = i32::from_le_bytes(b);
    let mut data = d.data.clone();
    xfer.xfer_ascii_string(&mut data.visible_drop_bone_name)?;
    xfer.xfer_ascii_string(&mut data.visible_sub_object_name)?;
    xfer.xfer_ascii_string(&mut data.visible_payload_template_name)?;
    xfer.xfer_real(&mut data.dist_to_target)?;
    if version >= 5 {
        xfer.xfer_real(&mut data.pre_open_distance)?;
    }
    xfer.xfer_int(&mut data.max_attempts)?;
    xfer.xfer_coord3d(&mut data.drop_offset)?;
    xfer.xfer_coord3d(&mut data.drop_variance)?;
    xfer.xfer_unsigned_int(&mut data.drop_delay)?;
    xfer.xfer_bool(&mut data.fire_weapon)?;
    xfer.xfer_bool(&mut data.self_destruct_object)?;
    xfer.xfer_int(&mut data.visible_num_bones)?;
    xfer.xfer_real(&mut data.dive_start_distance)?;
    xfer.xfer_real(&mut data.dive_end_distance)?;
    let mut b = data.strafing_weapon_slot.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    data.strafing_weapon_slot = i32::from_le_bytes(b);
    xfer.xfer_int(&mut data.visible_items_dropped_per_interval)?;
    xfer.xfer_bool(&mut data.inherit_transport_velocity)?;
    xfer.xfer_bool(&mut data.is_parachute_directly)?;
    xfer.xfer_real(&mut data.exit_pitch_rate)?;
    // (`const FXList *m_strafeFX;` not saved)
    xfer.xfer_real(&mut data.strafe_length)?;
    let mut weapon_template_name = match data.visible_payload_weapon_template {
        Some(w) => h.weapon_template_name(w),
        None => Vec::new(),
    };
    xfer.xfer_ascii_string(&mut weapon_template_name)?;
    if xfer.get_xfer_mode() == XferMode::Load && !weapon_template_name.is_empty() {
        data.visible_payload_weapon_template = h.weapon_template_by_name(&weapon_template_name);
    }
    xfer_radius_decal_template(&mut data.delivery_decal_template, xfer)?;
    xfer.xfer_real(&mut data.delivery_decal_radius)?;
    d.data = data;
    Ok(())
}

/// `m_deliveryDecal.xferRadiusDecal(xfer)` ("@todo implement me": nothing saved; a load clears it), then the version-dependent
/// flags and the remembered distance.
pub fn deliver_xfer_tail<H: AiStateHost>(d: &mut DeliverPayload, h: &mut H, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
    if xfer.get_xfer_mode() == XferMode::Load {
        d.delivery_decal.empty = true;
        h.delivery_decal_clear();
    }
    if version >= 2 {
        xfer.xfer_bool(&mut d.free_to_exit)?;
    }
    if version >= 3 {
        xfer.xfer_bool(&mut d.accepting_commands)?;
    }
    if version >= 4 {
        xfer.xfer_real(&mut d.previous_distance_sqr)?;
    }
    Ok(())
}
