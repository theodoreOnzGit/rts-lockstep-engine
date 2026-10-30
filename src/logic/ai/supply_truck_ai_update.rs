// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/SupplyTruckAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SupplyTruckAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `SupplyTruckAIUpdate` (`p1w13-sonnet`, layer 10, batch A35): the supply truck's autopilot, a second machine
//! (`SupplyTruckStateMachine`, [`SupplyState`]) beside the AI's: idle, busy (the player's), wanting (dock with the best supply
//! warehouse, or with boxes the best supply center), regrouping (back to base when there is none), docking.
//!
//! The truck's members are [`SupplyTruck`], held in [`AiSubclass::SupplyTruck`]; its machine is
//! `AiUpdateModule::supply_machine` (it names the host's type). The machine's states reach the AI and the AI's machine, lent to
//! the host while it runs ([`AiStateHost::ai_stack`], [`AiStateHost::ai_machine_stack`]). [`AiUpdateModule::update`] runs it
//! before the base's update; `ai_do_command`'s idle and dock take its overrides; the docking states ask
//! [`action_delay_for_dock`]; [`gain_one_box`] and [`lose_one_box`] are the docks' calls.
//!
//! # Upstream facts kept
//!
//! - The machine is made, and its default state (busy, the first defined) entered, by the AI's constructor: the object has no AI
//!   yet, so the busy state's `onEnter` does not clear the forced-busy flag, and the transitions checked on entering it all
//!   answer false.
//! - `xfer` saves the machine, the preferred dock, the boxes and the forced-wanting flag, not the forced-busy flag; the idle and
//!   busy states save nothing (not even a version).
//! - `gainOneBox` with the last stock plays the depleted voice unless the best warehouse is within a quarter of the scan
//!   distance; the scan distance is doubled for a computer player.
//! - `isAvailableForSupplying` is always true; the wanting state's check of it never fails.
//!
//! [`AiUpdateModule::update`]: crate::logic::ai::ai_states::AiUpdateModule::update

use std::convert::Infallible;
use std::marker::PhantomData;

use crate::common::base_type::Coord3D;
use crate::common::game_common::PlayerType;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{KindOfType, KINDOF_CASH_GENERATOR, KINDOF_COMMANDCENTER, KINDOF_STRUCTURE};
use crate::common::state_machine::{
    SmResult, StateBehavior, StateConditionInfo, StateExitType, StateId, StateMachine, StateReturn,
};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_commands::{self as commands, AiCommandParms, AICMD_DOCK};
use crate::logic::ai::ai_states::{ai, AiState, AiStateHost};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::{AI_DOCK, CMD_FROM_AI, CMD_FROM_PLAYER};
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `ST_IDLE`: "Not doing anything. Should I autopilot?"
pub const ST_IDLE: StateId = 0;
/// `ST_BUSY`: "Direct player involvement (move) has taken me off autopilot".
pub const ST_BUSY: StateId = 1;
/// `ST_WANTING`: "Search for warehouse or center and dock with it".
pub const ST_WANTING: StateId = 2;
/// `ST_REGROUPING`: "Wanting failed, so hang out at base until something changes."
pub const ST_REGROUPING: StateId = 3;
/// `ST_DOCKING`: "Docking substates are running, wait for them to finish".
pub const ST_DOCKING: StateId = 4;
/// `REGROUP_SUCCESS_DISTANCE_SQUARED`.
const REGROUP_SUCCESS_DISTANCE_SQUARED: f32 = 225.0;

/// A dock's supply module (`findUpdateModule` of `SupplyWarehouseDockUpdate`, then `SupplyCenterDockUpdate`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplyDockKind {
    /// Neither.
    None,
    /// A `SupplyWarehouseDockUpdate`.
    Warehouse,
    /// A `SupplyCenterDockUpdate`.
    Center,
}

/// `SupplyTruckAIUpdateModuleData`'s own fields (the delays in frames).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SupplyTruckData {
    /// `MaxBoxes`.
    pub max_boxes: i32,
    /// `SupplyCenterActionDelay`.
    pub center_delay: u32,
    /// `SupplyWarehouseActionDelay`.
    pub warehouse_delay: u32,
    /// `SupplyWarehouseScanDistance` (default 100).
    pub warehouse_scan_distance: f32,
    /// `SuppliesDepletedVoice` is named (an empty one is not played).
    pub has_depleted_voice: bool,
}

/// `SupplyTruckAIUpdate`'s members (its machine is the module's), with its module data.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SupplyTruck {
    /// The module data's own fields.
    pub data: SupplyTruckData,
    /// `m_preferredDock`: "Instead of searching, try this one first".
    pub preferred_dock: ObjectId,
    /// `m_numberBoxes`.
    pub number_boxes: i32,
    /// `m_forcePending`.
    pub force_pending: bool,
    /// `m_forcedBusyPending`.
    pub forced_busy_pending: bool,
}

impl SupplyTruck {
    /// `SupplyTruckAIUpdate(thing, moduleData)`'s members.
    pub fn new(data: SupplyTruckData) -> Self {
        SupplyTruck { data, preferred_dock: INVALID_ID, number_boxes: 0, force_pending: false, forced_busy_pending: false }
    }
}

/// `SupplyTruckStateMachine`'s states.
pub enum SupplyState<H: AiStateHost> {
    /// `SupplyTruckBusyState`.
    Busy,
    /// `SupplyTruckIdleState`.
    Idle,
    /// `SupplyTruckWantsToPickUpOrDeliverBoxesState`.
    Wanting,
    /// `RegroupingState`.
    Regrouping,
    /// `DockingState`.
    Docking,
    /// (names the host's type; never made)
    _Host(Infallible, PhantomData<fn() -> H>),
}

/// `SupplyTruckStateMachine`'s transition conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplyCondition {
    /// `ownerDocking`: the AI's state is the dock state.
    OwnerDocking,
    /// `ownerIdle`.
    OwnerIdle,
    /// `ownerAvailableForSupplying`: available, and idle.
    OwnerAvailableForSupplying,
    /// `ownerNotDockingOrIdle`.
    OwnerNotDockingOrIdle,
    /// `isForcedIntoWantingState`.
    IsForcedIntoWantingState,
    /// `isForcedIntoBusyState`.
    IsForcedIntoBusyState,
    /// `ownerPlayerCommanded`: the last command was the player's.
    OwnerPlayerCommanded,
}

/// The running AI's `SupplyTruckAIUpdate` members.
fn st<H: AiStateHost>(h: &mut H) -> &mut SupplyTruck {
    match &mut ai(h).subclass {
        AiSubclass::SupplyTruck(s) => s,
        _ => unreachable!("a SupplyTruckAIUpdate"),
    }
}

/// Runs `f` on the AI's machine (lent to the host while the supply machine runs).
pub(crate) fn with_ai_machine<H: AiStateHost, R>(h: &mut H, f: impl FnOnce(&mut Sm<H>, &mut H) -> R) -> R {
    let mut sm = h.ai_machine_stack().pop().expect("the AI's machine, lent");
    let r = f(&mut sm, h);
    h.ai_machine_stack().push(sm);
    r
}

/// `ai->isIdle()` (the AI's override, as upstream's virtual).
fn ai_is_idle<H: AiStateHost>(h: &mut H) -> bool {
    with_ai_machine(h, |sm, h| ai(h).is_idle(&*sm))
}

/// `ai->getAIStateType()`.
fn ai_state_type<H: AiStateHost>(h: &mut H) -> StateId {
    with_ai_machine(h, |sm, _| sm.get_current_state_id())
}

/// `SupplyTruckStateMachine(owner)`: its five states and their conditions (busy first: the default state).
pub fn make_supply_machine<H: AiStateHost>() -> SmResult<StateMachine<SupplyState<H>>> {
    use SupplyCondition::*;
    let c = |test, to_state_id| StateConditionInfo { test, to_state_id };
    let mut m = StateMachine::new();
    m.define_state(ST_BUSY, SupplyState::Busy, ST_BUSY, ST_BUSY, &[c(OwnerIdle, ST_IDLE), c(OwnerDocking, ST_DOCKING)])?;
    m.define_state(
        ST_IDLE,
        SupplyState::Idle,
        ST_BUSY,
        ST_BUSY,
        &[
            c(IsForcedIntoBusyState, ST_BUSY),
            c(IsForcedIntoWantingState, ST_WANTING),
            c(OwnerDocking, ST_DOCKING),
            c(OwnerNotDockingOrIdle, ST_BUSY),
        ],
    )?;
    m.define_state(ST_WANTING, SupplyState::Wanting, ST_BUSY, ST_REGROUPING, &[c(OwnerDocking, ST_DOCKING), c(OwnerNotDockingOrIdle, ST_BUSY)])?;
    m.define_state(ST_REGROUPING, SupplyState::Regrouping, ST_WANTING, ST_BUSY, &[c(OwnerPlayerCommanded, ST_BUSY)])?;
    m.define_state(
        ST_DOCKING,
        SupplyState::Docking,
        ST_BUSY,
        ST_BUSY,
        &[c(IsForcedIntoBusyState, ST_BUSY), c(OwnerAvailableForSupplying, ST_WANTING), c(OwnerNotDockingOrIdle, ST_BUSY)],
    )?;
    Ok(m)
}

/// The wanting state's update: dock with the best supply center (with boxes) or warehouse (without).
fn wanting_update<H: AiStateHost>(h: &mut H) -> SmResult<StateReturn> {
    let num_boxes = st(h).number_boxes;
    let best = if num_boxes > 0 { h.supply_find_best_center() } else { h.supply_find_best_warehouse() };
    if best != INVALID_ID && !plant(3660) {
        with_ai_machine(h, |sm, h| {
            let mut parms = AiCommandParms::new(AICMD_DOCK, CMD_FROM_AI);
            parms.obj = best;
            commands::ai_do_command(sm, h, &parms)
        })?;
        return Ok(StateReturn::SUCCESS);
    }
    // "we aren't going to wait right here, we will go back to base and wait for"
    Ok(StateReturn::FAILURE)
}

/// The regrouping state's `onEnter`: "Will tell me to aiMove back to base" (a cash generator, else a command center, else any
/// structure of the player's, the closest; near enough already, nothing).
fn regrouping_on_enter<H: AiStateHost>(h: &mut H) -> SmResult<StateReturn> {
    if h.controlling_player_type().is_none() {
        return Ok(StateReturn::FAILURE);
    }
    ai(h).ignore_obstacle(INVALID_ID);
    let mut destination_object = INVALID_ID;
    for kind in [KINDOF_CASH_GENERATOR, KINDOF_COMMANDCENTER, KINDOF_STRUCTURE] as [KindOfType; 3] {
        destination_object = h.player_find_closest_by_kind_of(kind);
        if destination_object != INVALID_ID {
            break;
        }
    }
    if destination_object == INVALID_ID {
        return Ok(StateReturn::FAILURE);
    }
    if h.distance_squared_to_object_2d(destination_object) < REGROUP_SUCCESS_DISTANCE_SQUARED && !plant(3661) {
        // "Don't say Success so we don't spin the machine. After one update we'll go back."
        return Ok(StateReturn::CONTINUE);
    }
    let pos = h.object_position(destination_object);
    let Some(destination) = h.find_position_around_no_source(&pos, 0.0, 100.0) else {
        return Ok(StateReturn::FAILURE);
    };
    with_ai_machine(h, |sm, h| commands::ai_move_to_position(sm, h, &destination, CMD_FROM_AI))?;
    // "Remember to say continue when you change ai command inside a state"
    Ok(StateReturn::CONTINUE)
}

impl<H: AiStateHost> StateBehavior for SupplyState<H> {
    type Ctx = H;
    type Condition = SupplyCondition;

    fn on_enter(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(SupplyState::Busy) => {
                // (the object's AI: none yet while the AI's constructor enters the default state)
                if !h.ai_stack().is_empty() {
                    st(h).forced_busy_pending = false;
                }
            }
            // (the supply truck's `getWorkerAIInterface()` is none)
            Some(SupplyState::Idle) => {}
            Some(SupplyState::Wanting) | Some(SupplyState::Docking) => st(h).force_pending = false,
            Some(SupplyState::Regrouping) => return regrouping_on_enter(h),
            Some(SupplyState::_Host(never, _)) => match *never {},
            None => {}
        }
        Ok(StateReturn::CONTINUE)
    }

    fn on_exit(_sm: &mut StateMachine<Self>, _h: &mut H, _id: StateId, _status: StateExitType) -> SmResult<()> {
        Ok(())
    }

    fn update(sm: &mut StateMachine<Self>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(SupplyState::Wanting) => wanting_update(h),
            // "Once we have regrouped, chill with the regrouping. We will succeed to Wanting, which can handle Busy."
            Some(SupplyState::Regrouping) => Ok(if ai_is_idle(h) { StateReturn::SUCCESS } else { StateReturn::CONTINUE }),
            _ => Ok(StateReturn::CONTINUE),
        }
    }

    fn test(_sm: &mut StateMachine<Self>, h: &mut H, _id: StateId, condition: SupplyCondition) -> SmResult<bool> {
        // (each asks `owner->getAIUpdateInterface()` first: none while the AI's constructor enters the default state)
        if h.ai_stack().is_empty() {
            return Ok(false);
        }
        Ok(match condition {
            SupplyCondition::IsForcedIntoWantingState => st(h).force_pending,
            SupplyCondition::IsForcedIntoBusyState => st(h).forced_busy_pending && !plant(3662),
            SupplyCondition::OwnerDocking => ai_state_type(h) == AI_DOCK,
            SupplyCondition::OwnerPlayerCommanded => ai(h).last_command_source == CMD_FROM_PLAYER,
            SupplyCondition::OwnerIdle => ai_is_idle(h),
            SupplyCondition::OwnerAvailableForSupplying => ai_is_idle(h),
            SupplyCondition::OwnerNotDockingOrIdle => !ai_is_idle(h) && ai_state_type(h) != AI_DOCK,
        })
    }

    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// The busy and idle states save nothing; the others their version (1).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        if matches!(self, SupplyState::Wanting | SupplyState::Regrouping | SupplyState::Docking) {
            let current_version: XferVersion = 1;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
        }
        Ok(())
    }

    fn load_post_process(&mut self) {}
}

/// `SupplyTruckStateMachine::xfer`: its version (1), then the machine's.
pub fn supply_machine_xfer<H: AiStateHost>(m: &mut StateMachine<SupplyState<H>>, xfer: &mut dyn Xfer) -> SmResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    m.xfer_machine(xfer)
}

/// `getActionDelayForDock(dock)`: the warehouse's or the center's delay, else none.
pub fn action_delay_for_dock<H: AiStateHost>(h: &mut H, s: &SupplyTruck, dock: ObjectId) -> u32 {
    match h.dock_supply_kind(dock) {
        SupplyDockKind::Warehouse => s.data.warehouse_delay,
        SupplyDockKind::Center => s.data.center_delay,
        SupplyDockKind::None => 0,
    }
}

/// `getWarehouseScanDistance()`: doubled for a computer player (the player unchecked upstream).
pub fn warehouse_scan_distance<H: AiStateHost>(h: &mut H, s: &SupplyTruck) -> f32 {
    if h.controlling_player_type() == Some(PlayerType::Computer) {
        return 2.0 * s.data.warehouse_scan_distance;
    }
    s.data.warehouse_scan_distance
}

/// `loseOneBox()`: one box fewer (none: false), the drawable told.
pub fn lose_one_box<H: AiStateHost>(h: &mut H, s: &mut SupplyTruck) -> bool {
    if s.number_boxes == 0 {
        return false;
    }
    s.number_boxes -= 1;
    h.drawable_update_supply_status(s.data.max_boxes, s.number_boxes);
    true
}

/// `gainOneBox(remainingStock)`: one box more (full: false); the warehouse's last stock taken, the depleted voice unless the best
/// warehouse is near; the drawable told.
pub fn gain_one_box<H: AiStateHost>(h: &mut H, s: &mut SupplyTruck, remaining_stock: i32) -> bool {
    if s.number_boxes >= s.data.max_boxes && !plant(3663) {
        return false;
    }
    s.number_boxes += 1;
    if remaining_stock == 0 {
        let best_warehouse = h.supply_find_best_warehouse();
        let play_depleted = if best_warehouse != INVALID_ID {
            let me = h.position();
            let wh = h.object_position(best_warehouse);
            let delta = Coord3D { x: me.x - wh.x, y: me.y - wh.y, z: me.z - wh.z };
            delta.length() > warehouse_scan_distance(h, s) / 4.0
        } else {
            true
        };
        if play_depleted && s.data.has_depleted_voice {
            h.play_supplies_depleted_voice();
        }
    }
    h.drawable_update_supply_status(s.data.max_boxes, s.number_boxes);
    true
}
