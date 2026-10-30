// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/WorkerAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/WorkerAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `WorkerAIUpdate` (`p1w13-sonnet`, layer 10, batch A39): the GLA worker, a dozer and a supply truck in one. Its own machine
//! (`WorkerStateMachine`, [`WorkerState`]) says which it is: as a dozer it runs the dozer's primary machine
//! ([`DozerPrimaryState`]), as a supply truck the supply truck's machine ([`SupplyState`]); a dock (or a forced supply run) makes it
//! a supply truck, a busy supply brain a dozer again, and so does a dozer's task or construction, or its supply brain going idle.
//!
//! The members are [`Worker`]: the dozer's ([`Dozer`], which [`dz`] reaches) and the supply truck's ([`SupplyTruck`]), held in
//! [`AiSubclass::Worker`]; the dozer's and the supply truck's code is theirs (`dozer_ai_update`, `supply_truck_ai_update`), with the
//! worker's overrides where its virtual members differ. Its three machines are `AiUpdateModule::{dozer_machine, supply_machine,
//! worker_machine}`, lent to the host while the AI runs ([`AiStateHost::dozer_machine_stack`],
//! [`AiStateHost::supply_machine_stack`], [`AiStateHost::worker_machine_stack`]); a running machine's slot is empty.
//!
//! # Upstream facts kept
//!
//! - `ActAsDozerState::onExit()` and `ActAsSupplyTruckState::onExit()` take no exit status: they override nothing (`State`'s is
//!   `onExit(StateExitType)`), so they never run, and leaving a role never resets its brain (`resetDozerBrain` and
//!   `resetSupplyTruckBrain` are called by nothing else).
//! - The constructor makes the three machines (the worker's first, entered last: its transitions then ask an object with no AI,
//!   and answer false); `m_preferredDock`, which it does not set, is `INVALID_ID`: the memory pool zeroes every object it makes.
//! - `update` runs the base's update first, then (unless dead) the worker's machine, then as a dozer a repair it may no longer do
//!   cancelled and the dozer's machine; as a supply truck the supply truck's machine and mine clearing allowed. Never asleep.
//! - The overrides that differ from the dozer's: no computer modifier of the bored range; a repair asks the action manager before
//!   the current repair; a new task forgets the preferred dock, ends its dock points where it starts them, and leaves the supply
//!   truck role; a cancelled task moves to where the worker is (the dozer idles); a completed or cancelled repair of a bridge
//!   tower removes the scaffolding; no move-away exception; a command that leaves it clearing mines drops its boxes; a computer
//!   player's construction checks only a clear path and no overlap, leaves the supply truck role, and sets the new object's
//!   under-construction status before placing it.

use std::convert::Infallible;
use std::marker::PhantomData;

use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::object_status_types::OBJECT_STATUS_IS_ATTACKING;
use crate::common::state_machine::{
    SmResult, StateBehavior, StateConditionInfo, StateExitType, StateId, StateMachine, StateReturn, INVALID_STATE_ID,
};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_commands as commands;
use crate::logic::ai::ai_states::{ai, AiStateHost};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::{AI_DOCK, CMD_FROM_AI};
use crate::logic::ai::dozer_ai_update::{self as dozer, dz, Dozer, DozerData, DOZER_TASK_INVALID, DOZER_TASK_REPAIR};
use crate::logic::ai::supply_truck_ai_update::{with_ai_machine, SupplyTruck, SupplyTruckData, ST_BUSY, ST_DOCKING, ST_WANTING};
use crate::logic::modules::plant;
use crate::logic::weapon::WEAPON_ANTI_MINE;
use crate::logic::weapon_types::WEAPONSET_MINE_CLEARING_DETAIL;

/// `AS_DOZER`: "When not actively Gathering, or when actively building, I am a dozer".
pub const AS_DOZER: StateId = 0;
/// `AS_SUPPLY_TRUCK`: "When told explicitly by player or other object, I become a supply truck".
pub const AS_SUPPLY_TRUCK: StateId = 1;

/// `WorkerAIUpdateModuleData`'s own fields: the dozer's, the supply truck's, and the upgraded supply boost.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorkerData {
    /// `RepairHealthPercentPerSecond`, `BoredTime`, `BoredRange`.
    pub dozer: DozerData,
    /// `MaxBoxes`, `SupplyCenterActionDelay`, `SupplyWarehouseActionDelay`, `SupplyWarehouseScanDistance`,
    /// `SuppliesDepletedVoice`.
    pub supply: SupplyTruckData,
    /// `UpgradedSupplyBoost`.
    pub upgraded_supply_boost: i32,
}

/// `WorkerAIUpdate`'s members (its machines are the module's), with its module data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Worker {
    /// The dozer's members and data (`DozerAIInterface`).
    pub dozer: Dozer,
    /// The supply truck's members and data (`SupplyTruckAIInterface`).
    pub supply: SupplyTruck,
    /// `UpgradedSupplyBoost`.
    pub upgraded_supply_boost: i32,
}

impl Worker {
    /// `WorkerAIUpdate(thing, moduleData)`'s members.
    pub fn new(data: WorkerData) -> Self {
        Worker { dozer: Dozer::new(data.dozer), supply: SupplyTruck::new(data.supply), upgraded_supply_boost: data.upgraded_supply_boost }
    }
}

/// `WorkerStateMachine`'s states (neither does anything: see the module docs).
pub enum WorkerState<H: AiStateHost> {
    /// `ActAsDozerState`.
    AsDozer,
    /// `ActAsSupplyTruckState`.
    AsSupplyTruck,
    /// (names the host's type; never made)
    _Host(Infallible, PhantomData<fn() -> H>),
}

/// `WorkerStateMachine`'s transition conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerCondition {
    /// `supplyTruckSubMachineWantsToEnter`: forced into wanting, or docking.
    SupplyTruckSubMachineWantsToEnter,
    /// `supplyTruckSubMachineReadyToLeave`: not wanting to enter, and the supply brain active and busy.
    SupplyTruckSubMachineReadyToLeave,
}

/// Whether the running AI is a `WorkerAIUpdate`.
pub fn is_worker<H: AiStateHost>(h: &mut H) -> bool {
    matches!(ai(h).subclass, AiSubclass::Worker(_))
}

/// The running AI's worker members.
pub fn wk<H: AiStateHost>(h: &mut H) -> &mut Worker {
    match &mut ai(h).subclass {
        AiSubclass::Worker(w) => w,
        _ => unreachable!("a WorkerAIUpdate"),
    }
}

/// Runs `f` on the worker's machine (lent to the host, not running).
fn with_worker_machine<H: AiStateHost, R>(h: &mut H, f: impl FnOnce(&mut StateMachine<WorkerState<H>>, &mut H) -> R) -> R {
    let mut m = h.worker_machine_stack().last_mut().expect("the worker machine's slot").take().expect("the worker machine (not running)");
    let r = f(&mut m, h);
    *h.worker_machine_stack().last_mut().expect("the worker machine's slot") = Some(m);
    r
}

/// `m_workerMachine->getCurrentStateID()`.
pub fn worker_state<H: AiStateHost>(h: &mut H) -> StateId {
    with_worker_machine(h, |m, _| m.get_current_state_id())
}

/// `m_supplyTruckStateMachine->getCurrentStateID()` (the supply machine lent, or re-lent by its running state).
fn supply_state<H: AiStateHost>(h: &mut H) -> StateId {
    h.supply_machine_stack().last().expect("the supply machine's slot").as_ref().expect("the supply machine (lent)").get_current_state_id()
}

/// `WorkerStateMachine(owner)`: as a dozer (first: the default state), as a supply truck, each with its condition.
pub fn make_worker_machine<H: AiStateHost>() -> SmResult<StateMachine<WorkerState<H>>> {
    use WorkerCondition::*;
    let c = |test, to_state_id| StateConditionInfo { test, to_state_id };
    let mut m = StateMachine::new();
    m.define_state(AS_DOZER, WorkerState::AsDozer, INVALID_STATE_ID, INVALID_STATE_ID, &[c(SupplyTruckSubMachineWantsToEnter, AS_SUPPLY_TRUCK)])?;
    m.define_state(AS_SUPPLY_TRUCK, WorkerState::AsSupplyTruck, INVALID_STATE_ID, INVALID_STATE_ID, &[c(SupplyTruckSubMachineReadyToLeave, AS_DOZER)])?;
    Ok(m)
}

/// `supplyTruckSubMachineWantsToEnter`: "If I detect a Supply force message, or if I have been put straight in dock".
fn wants_to_enter<H: AiStateHost>(h: &mut H) -> bool {
    let master_state = with_ai_machine(h, |sm, _| sm.get_current_state_id());
    wk(h).supply.force_pending || master_state == AI_DOCK
}

impl<H: AiStateHost> StateBehavior for WorkerState<H> {
    type Ctx = H;
    type Condition = WorkerCondition;

    fn on_enter(_sm: &mut StateMachine<Self>, _h: &mut H, _id: StateId) -> SmResult<StateReturn> {
        Ok(StateReturn::CONTINUE)
    }

    /// (the states' `onExit()` overrides nothing: `State`'s, which does nothing)
    fn on_exit(_sm: &mut StateMachine<Self>, _h: &mut H, _id: StateId, _status: StateExitType) -> SmResult<()> {
        Ok(())
    }

    fn update(_sm: &mut StateMachine<Self>, _h: &mut H, _id: StateId) -> SmResult<StateReturn> {
        Ok(StateReturn::CONTINUE)
    }

    fn test(sm: &mut StateMachine<Self>, h: &mut H, _id: StateId, condition: WorkerCondition) -> SmResult<bool> {
        // (each asks the owner's AI first: none while the AI's constructor enters the default state)
        if h.ai_stack().is_empty() {
            return Ok(false);
        }
        Ok(match condition {
            WorkerCondition::SupplyTruckSubMachineWantsToEnter => wants_to_enter(h),
            // "Active and Busy means it isn't doing anything Supply related." (`isSupplyTruckBrainActiveAndBusy`: this machine is
            // the worker's)
            WorkerCondition::SupplyTruckSubMachineReadyToLeave => {
                !wants_to_enter(h) && sm.get_current_state_id() == AS_SUPPLY_TRUCK && supply_state(h) == ST_BUSY && !plant(3730)
            }
        })
    }

    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// The states' snapshot interface is stubbed: nothing.
    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    fn load_post_process(&mut self) {}
}

/// `WorkerStateMachine::xfer`: its version (1), then the machine's.
pub fn worker_machine_xfer<H: AiStateHost>(m: &mut StateMachine<WorkerState<H>>, xfer: &mut dyn Xfer) -> SmResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    m.xfer_machine(xfer)
}

/// `exitingSupplyTruckState()` (and `newTask`'s and `construct`'s same lines): as a supply truck, the AI told to idle ("Explicitly
/// slap it upside the head.") and the worker made a dozer.
pub fn exiting_supply_truck_state<H: AiStateHost>(h: &mut H) -> SmResult<()> {
    if worker_state(h) == AS_SUPPLY_TRUCK && !plant(3731) {
        with_ai_machine(h, |sm, h| commands::ai_idle(sm, h, CMD_FROM_AI))?;
        // "Both states are being polite, so I must force the switch."
        with_worker_machine(h, |m, h| m.set_state(h, AS_DOZER).map(|_| ()))?;
    }
    Ok(())
}

/// `isSupplyTruckBrainActiveAndBusy()`.
pub fn is_supply_truck_brain_active_and_busy<H: AiStateHost>(h: &mut H) -> bool {
    worker_state(h) == AS_SUPPLY_TRUCK && supply_state(h) == ST_BUSY
}

/// `resetSupplyTruckBrain()` (`resupply` false: `resetDozerBrain()`): the brain's machine reset (called by nothing upstream but
/// the states' dead `onExit`s).
pub fn reset_brain<H: AiStateHost>(h: &mut H, supply: bool) -> SmResult<()> {
    if !supply {
        return dozer::reset_dozer_machine(h);
    }
    let mut m = h.supply_machine_stack().last_mut().expect("the supply machine's slot").take().expect("the supply machine (not running)");
    let r = m.reset_to_default_state(h);
    *h.supply_machine_stack().last_mut().expect("the supply machine's slot") = Some(m);
    r.map(|_| ())
}

/// `isCurrentlyFerryingSupplies()`: the supply brain wanting or docking.
pub fn is_currently_ferrying_supplies<H: AiStateHost>(h: &mut H) -> bool {
    matches!(supply_state(h), ST_WANTING | ST_DOCKING)
}

/// `getUpgradedSupplyBoost()`: the boost once the player has the worker shoes.
pub fn upgraded_supply_boost<H: AiStateHost>(h: &mut H) -> i32 {
    if h.player_has_upgrade_complete("Upgrade_GLAWorkerShoes") && !plant(3732) {
        return wk(h).upgraded_supply_boost;
    }
    0
}

/// `privateDock(dock, cmdSource)`'s own part (after the base's): "If this is a command from a player, I will remember this as my
/// favorite dock".
pub fn remember_dock<H: AiStateHost>(h: &mut H, dock: ObjectId, from_player: bool) {
    if from_player && dock != INVALID_ID && !plant(3733) {
        wk(h).supply.preferred_dock = dock;
    }
}

/// `newTask`'s first line: "If we are dozing, we don't want any supply truck stuff going on. jba."
pub fn forget_preferred_dock<H: AiStateHost>(h: &mut H) {
    wk(h).supply.preferred_dock = INVALID_ID;
}

/// `aiDoCommand`'s last lines: "if clearing mines, we drop any boxes we were carrying" (the drawable told).
pub fn drop_boxes_if_clearing_mines<H: AiStateHost>(h: &mut H) {
    let clearing = h.status_test(OBJECT_STATUS_IS_ATTACKING) && h.current_weapon_anti_mask().is_some_and(|m| m & WEAPON_ANTI_MINE as i32 != 0);
    if clearing && wk(h).supply.number_boxes > 0 && !plant(3734) {
        let w = wk(h);
        w.supply.number_boxes = 0;
        let max = w.supply.data.max_boxes;
        h.drawable_update_supply_status(max, 0);
    }
}

/// `update()`'s own part, after the base's (the AI, its machine and the three machines lent): dead, nothing; the worker's machine;
/// as a dozer, a current repair it may no longer do cancelled, then the dozer's machine (a reset asked for while it runs done after
/// it, as the dozer's); as a supply truck, the supply truck's machine, then mine clearing allowed ("If we are harvesting, we can be
/// diverted to clear mines. jba.").
pub fn worker_update<H: AiStateHost>(h: &mut H) -> SmResult<()> {
    if h.is_effectively_dead() {
        return Ok(());
    }
    let mut wm = h.worker_machine_stack().last_mut().expect("the worker machine's slot").take().expect("the worker machine");
    let r = wm.update_state_machine(h);
    *h.worker_machine_stack().last_mut().expect("the worker machine's slot") = Some(wm);
    r?;
    if worker_state(h) == AS_DOZER {
        let current_task = dz(h).current_task;
        if current_task != DOZER_TASK_INVALID {
            let target = dozer::get_task_target(h, current_task);
            let target = if h.object_exists(target) { target } else { INVALID_ID };
            let src = ai(h).last_command_source;
            if current_task == DOZER_TASK_REPAIR && !h.action_can_repair_object(target, src) && !plant(3735) {
                dozer::cancel_task(h, current_task)?;
            }
        }
        let mut m = h.dozer_machine_stack().last_mut().expect("the dozer machine's slot").take().expect("the dozer machine");
        let r = m.update_state_machine(h);
        let r = r.and_then(|_| {
            if std::mem::take(&mut dz(h).machine_reset_pending) {
                m.reset_to_default_state(h).map(|_| ())
            } else {
                Ok(())
            }
        });
        *h.dozer_machine_stack().last_mut().expect("the dozer machine's slot") = Some(m);
        r
    } else {
        let mut m = h.supply_machine_stack().last_mut().expect("the supply machine's slot").take().expect("the supply machine");
        let r = m.update_state_machine(h);
        *h.supply_machine_stack().last_mut().expect("the supply machine's slot") = Some(m);
        r?;
        h.set_weapon_set_flag(WEAPONSET_MINE_CLEARING_DETAIL, true);
        Ok(())
    }
}
