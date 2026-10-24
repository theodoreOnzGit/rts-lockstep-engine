// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/StateMachine.h
//   GeneralsMD/Code/GameEngine/Source/Common/StateMachine.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Michael S. Booth, January 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StateMachine`, the generic finite state machine that the AI and some
//! modules derive from (Phase 1, layer 5).
//!
//! # The shape, for the code that builds on it (layer 10, `AIStateMachine`)
//!
//! Upstream: `class State` is abstract (`onEnter`, `onExit`, `update`, the
//! four `is*` queries, `crc`, `xfer`, `loadPostProcess`), each AI state
//! derives from it, a `StateMachine` owns one instance per state id in a
//! `std::map<StateID, State*>`, and a transition condition is a function
//! pointer plus a `void*`. **Here there is no trait object.**
//!
//! - **A state is a value of an enum the caller defines** (`S`, the
//!   implementor of [`StateBehavior`]): one variant per upstream state class,
//!   holding that class's data members. The machine stores `S` values in a
//!   `BTreeMap<StateId, _>` (the ordered map `std::map` is; the order is the
//!   order `xfer` walks) and dispatches by `match` inside the caller's
//!   [`StateBehavior`] functions. [`UtilityState`] is the four states the
//!   upstream header defines (`SuccessState`, `FailureState`, `ContinueState`,
//!   `SleepState`): put it in one variant of `S` and delegate to it.
//! - **A condition is a value of an enum too** (`S::Condition`): upstream's
//!   `(StateTransFuncPtr test, void *userData)` pair becomes one variant
//!   carrying its `userData` as fields; [`StateBehavior::test`] `match`es on
//!   it.
//! - **The callbacks are associated functions, not methods on the state:**
//!   they receive the machine and the state's id, `fn update(sm, cx, id)`, and
//!   reach their own data with [`StateMachine::state`] / [`StateMachine::state_mut`]
//!   (`match sm.state_mut(id) { Some(MyState::Move { timer, .. }) => ... }`)
//!   at the point of use. This is what makes the machine re-entrant as
//!   upstream is: `update()` may call `getMachine()->setState(...)`, which
//!   runs `onExit`/`onEnter` of other states (or of the same one) while the
//!   outer call is still running, and the machine then notices that the
//!   current state changed under it. A `&mut` to the state held across such a
//!   call would be aliased; an id is not.
//! - **The owner `Object` and the world are the context `S::Ctx`**, passed to
//!   every call (`cx`). Whatever upstream reads through `getMachineOwner()` or
//!   `TheGameLogic` the state reads from `cx`. The machine itself needs only
//!   the two things [`StateContext`] names. The goal object is stored as an
//!   [`ObjectId`] (as upstream) and looked up through the context.
//! - **Errors.** What upstream `throw`s (`ERROR_BAD_ARG`, `SC_INVALID_DATA`, an
//!   `XferStatus`) is a [`StateMachineError`], and every call that can reach
//!   a callback or an unknown state id returns [`SmResult`]. Nothing is
//!   caught: after an error the machine is exactly where upstream's stack
//!   unwinding leaves it.
//!
//! ```
//! use rts_lockstep_engine::common::state_machine::*;
//! use rts_lockstep_engine::common::xfer::{Xfer, XferResult};
//! use rts_lockstep_engine::common::game_type::ObjectId;
//!
//! struct World { frame: u32 }
//! impl StateContext for World {
//!     fn frame(&self) -> u32 { self.frame }
//!     fn find_object(&self, _: ObjectId) -> Option<ObjectRef> { None }
//! }
//!
//! enum Guard { Wait(UtilityState), Patrol { laps: u32 } }
//! #[derive(Clone, Copy)]
//! enum Cond { LapsDone }
//!
//! impl StateBehavior for Guard {
//!     type Ctx = World;
//!     type Condition = Cond;
//!     fn on_enter(sm: &mut StateMachine<Self>, _: &mut World, id: StateId) -> SmResult<StateReturn> {
//!         Ok(match sm.state(id) { Some(Guard::Wait(u)) => u.on_enter(), _ => StateReturn::CONTINUE })
//!     }
//!     fn update(sm: &mut StateMachine<Self>, _: &mut World, id: StateId) -> SmResult<StateReturn> {
//!         Ok(match sm.state_mut(id) {
//!             Some(Guard::Wait(u)) => u.update(),
//!             Some(Guard::Patrol { laps }) => { *laps += 1; StateReturn::CONTINUE }
//!             None => StateReturn::FAILURE,
//!         })
//!     }
//!     fn test(sm: &mut StateMachine<Self>, _: &mut World, id: StateId, c: Cond) -> SmResult<bool> {
//!         Ok(match (c, sm.state(id)) { (Cond::LapsDone, Some(Guard::Patrol { laps })) => *laps >= 3, _ => false })
//!     }
//!     fn crc(&mut self, _: &mut dyn Xfer) -> XferResult<()> { Ok(()) }
//!     fn xfer(&mut self, _: &mut dyn Xfer) -> XferResult<()> { Ok(()) }
//!     fn load_post_process(&mut self) {}
//! }
//!
//! let mut sm = StateMachine::<Guard>::new();
//! sm.define_state(1, Guard::Patrol { laps: 0 }, EXIT_MACHINE_WITH_SUCCESS, INVALID_STATE_ID,
//!     &[StateConditionInfo { test: Cond::LapsDone, to_state_id: 2 }]).unwrap();
//! sm.define_state(2, Guard::Wait(UtilityState::Sleep), INVALID_STATE_ID, INVALID_STATE_ID, &[]).unwrap();
//! let mut world = World { frame: 10 };
//! assert_eq!(sm.init_default_state(&mut world), Ok(StateReturn::CONTINUE));
//! sm.update_state_machine(&mut world).unwrap();
//! sm.update_state_machine(&mut world).unwrap();
//! // the third update completes the laps; the condition moves to the sleeper
//! assert_eq!(sm.update_state_machine(&mut world), Ok(StateReturn::SLEEP_FOREVER));
//! assert_eq!(sm.get_current_state_id(), 2);
//! ```
//!
//! # Cut points: every `Object` / `GameLogic` call in the two files
//!
//! | upstream | here |
//! |---|---|
//! | `TheGameLogic->getFrame()` (`updateStateMachine`, `internalSetState`) | [`StateContext::frame`] |
//! | `TheGameLogic->findObjectByID(id)` (`getGoalObject`, `isGoalObjectDestroyed`) | [`StateContext::find_object`] |
//! | `Object::getID()`, `Object::getPosition()` (`setGoalObject(const Object*)`) | the argument is an [`ObjectRef`] (`None` for `NULL`) |
//! | `State::getMachineOwner()`, `StateMachine::getOwner()`, the constructor's `Object *owner` | no field: the owner is in `S::Ctx`; the caller's states read it from `cx` |
//! | `m_owner->getTemplate()->getName()` (debug logging) | not translated (see below) |
//! | `TheGlobalData->m_stateMachineDebug`, `TheObjectIDToDebug` | not translated (see below) |
//! | `State::getMachineGoalObject()` | [`StateMachine::goal_object`] with the context |
//!
//! # Not translated
//!
//! - The `STATE_MACHINE_DEBUG` code (`_DEBUG` or `_INTERNAL` builds only): the
//!   state and machine names, `getTransitions`, the transition description
//!   strings, `m_lockedby`, `setDebugOutput`, the `DEBUG_LOG` output and the
//!   consistency walk at the start of `initDefaultState`. The shipped game is a
//!   release build. `lock(msg)` therefore takes no message.
//! - `peekSleepTill` is kept ([`StateMachine::peek_sleep_till`]); upstream has
//!   it only in debug builds.
//! - The destructor's work is [`StateMachine::destroy`] (it needs the context).
//!   Memory pools (`MEMORY_POOL_GLUE_*`, `deleteInstance`) have no counterpart.
//!
//! # Where upstream is undefined: Rust errors, counted
//!
//! Each returns an error that [`StateMachineError::is_undefined_upstream`]
//! reports, and adds one to [`StateMachine::undefined_inputs`]; the
//! differential test does not compare these with the C++ and counts them.
//!
//! - **A state id defined twice** (`defineState`): `std::map::insert` keeps
//!   the first state, the second `State` object (already given the id,
//!   successor ids and conditions) is orphaned and leaked. Here:
//!   [`StateMachineError::DuplicateStateId`], nothing changes.
//! - **Unbounded re-entrancy** (a state whose `onEnter` calls `setState` on
//!   itself): upstream recurses until the stack overflows. `StateMachine`
//!   counts the nesting of `update_state_machine` / `internal_set_state` and
//!   stops at [`MAX_NESTING`] with [`StateMachineError::RecursionLimit`].
//!   (The `checkfortransitionsnum >= 20` guard of the transition checks only
//!   bounds success/failure chains; it is translated, and a hit is the normal
//!   `STATE_FAILURE` result, not an error.)
//!
//! Things that look undefined and are not: an unknown state id silently
//! recovers to the default state in a release build (`internalGetState`), and
//! throws `ERROR_BAD_ARG` only when there is no default state either
//! ([`StateMachineError::BadArg`]); `StateReturn` carries any `int`, so
//! `STATE_SLEEP` of more than `i32::MAX` frames is a negative value, which
//! upstream treats as "not a sleep" and does not match `STATE_SUCCESS` or
//! `STATE_FAILURE` either (the machine then simply keeps running).
//!
//! # Upstream quirks kept as they are
//!
//! - `isInForceAttackState()` returns `isIdle()` of the current state (not a
//!   force-attack test); `isInAttackState()` is `true` with no current state.
//! - `m_sleepTill == 0` means "not sleeping", so a sleep that ends exactly at
//!   frame `2^32` (wrapping to 0) is not a sleep. Frame arithmetic wraps.
//! - Saving a machine with no current state first writes `INVALID_STATE_ID`,
//!   then sets the current state to the default one ("trying to heal").
//! - The "snapshot all states" branch of `xfer` is dead on save
//!   (`snapshotAllStates` is `false`) but is read from the stream on load,
//!   so it is translated.
//!
//! # Verification
//!
//! `tests/differential_state_machine.rs` compiles `StateMachine.cpp`
//! unchanged and compares it with this module; see there for the method and
//! the numbers.

use std::cell::Cell;
use std::collections::BTreeMap;

use super::base_type::Coord3D;
use super::game_type::{ObjectId, INVALID_ID};
use super::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};

/// `typedef UnsignedInt StateID`.
pub type StateId = u32;

/// `MACHINE_DONE_STATE_ID`: the "machine finished" pseudo state.
pub const MACHINE_DONE_STATE_ID: StateId = 999_998;
/// `INVALID_STATE_ID`.
pub const INVALID_STATE_ID: StateId = 999_999;
/// `EXIT_MACHINE_WITH_SUCCESS`: as a successor or condition target, "the
/// machine exits and returns `STATE_SUCCESS`".
pub const EXIT_MACHINE_WITH_SUCCESS: StateId = 9998;
/// `EXIT_MACHINE_WITH_FAILURE`.
pub const EXIT_MACHINE_WITH_FAILURE: StateId = 9999;

/// How deeply `update_state_machine` / `internal_set_state` may nest through
/// callbacks before [`StateMachineError::RecursionLimit`] (upstream has no
/// limit and overflows the stack).
pub const MAX_NESTING: u32 = 256;

/// The `checkfortransitionsnum >= 20` limit of `friend_checkForTransitions`
/// and `friend_checkForSleepTransitions`.
const MAX_TRANSITION_DEPTH: i32 = 20;

/// `enum StateReturnType`: the result of `onEnter`, `update` and of the
/// machine's own calls. `0` stays, `-1` succeeded, `-2` failed, a positive
/// value is a sleep of that many frames. Any `int` can be carried, as the
/// C++ enum is cast from arbitrary values (`STATE_SLEEP(n)` is
/// `(StateReturnType)n`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StateReturn(pub i32);

impl StateReturn {
    /// `STATE_CONTINUE`: stay in this state (only for `update`).
    pub const CONTINUE: StateReturn = StateReturn(0);
    /// `STATE_SUCCESS`: the state finished successfully, go to the next.
    pub const SUCCESS: StateReturn = StateReturn(-1);
    /// `STATE_FAILURE`: the state finished abnormally, go to the next.
    pub const FAILURE: StateReturn = StateReturn(-2);
    /// `STATE_SLEEP_FOREVER`: `STATE_SLEEP(0x3fffffff)` (small enough to add
    /// offsets without overflow: about 414 days at 30 fps).
    pub const SLEEP_FOREVER: StateReturn = StateReturn(0x3fff_ffff);

    /// `STATE_SLEEP(numFrames)`: `(StateReturnType)numFrames`, which wraps
    /// above `i32::MAX`.
    pub const fn sleep(frames: u32) -> StateReturn {
        StateReturn(frames as i32)
    }

    /// `IS_STATE_SLEEP(ret)`: `(Int)ret > 0`.
    pub const fn is_sleep(self) -> bool {
        self.0 > 0
    }

    /// `GET_STATE_SLEEP_FRAMES(ret)`: `(UnsignedInt)ret`.
    pub const fn sleep_frames(self) -> u32 {
        self.0 as u32
    }

    /// `CONVERT_SLEEP_TO_CONTINUE`: for states that enclose other machines,
    /// which must run every frame even if the enclosed one sleeps.
    pub const fn convert_sleep_to_continue(self) -> StateReturn {
        if self.is_sleep() {
            StateReturn::CONTINUE
        } else {
            self
        }
    }

    /// `MIN_SLEEP(encloserSleep, encloseeResult)`: the shorter of two sleeps
    /// (unsigned minimum); an enclosee that does not sleep wins as it is.
    pub fn min_sleep(encloser_sleep: u32, enclosee_result: StateReturn) -> StateReturn {
        if enclosee_result.is_sleep() {
            StateReturn::sleep(encloser_sleep.min(enclosee_result.sleep_frames()))
        } else {
            enclosee_result
        }
    }
}

/// `enum StateExitType`: the parameter of `onExit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateExitType {
    /// `EXIT_NORMAL`: the state exited by normal state transitioning.
    Normal,
    /// `EXIT_RESET`: the state exited because the machine was reset.
    Reset,
}

/// `struct StateConditionInfo` without its function pointer and `void*`: the
/// condition (a value of the caller's enum, carrying the `userData`) and the
/// state to go to when it holds. Upstream ends a list with a `NULL` test;
/// here a slice has its length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateConditionInfo<K: Copy> {
    /// `test` and `userData`.
    pub test: K,
    /// `toStateID` (may be `EXIT_MACHINE_WITH_SUCCESS` / `_FAILURE`).
    pub to_state_id: StateId,
}

/// What upstream throws or cannot do, see the module documentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateMachineError {
    /// `throw ERROR_BAD_ARG` (`internalGetState`: unknown id and no default).
    BadArg,
    /// `throw SC_INVALID_DATA` (`xfer`: count or id mismatch).
    InvalidData,
    /// `throw` of an `XferStatus` from the xfer.
    Xfer(XferStatus),
    /// Undefined upstream: the id is already defined.
    DuplicateStateId(StateId),
    /// Undefined upstream: nesting deeper than [`MAX_NESTING`].
    RecursionLimit,
}

impl StateMachineError {
    /// Whether this is one of the inputs the C++ does not define (and the
    /// differential test therefore does not compare).
    pub fn is_undefined_upstream(&self) -> bool {
        matches!(
            self,
            StateMachineError::DuplicateStateId(_) | StateMachineError::RecursionLimit
        )
    }
}

impl std::fmt::Display for StateMachineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StateMachineError {}

impl From<XferStatus> for StateMachineError {
    fn from(s: XferStatus) -> Self {
        StateMachineError::Xfer(s)
    }
}

/// The result of a call that can reach a callback or an unknown state.
pub type SmResult<T> = Result<T, StateMachineError>;

/// An object as the machine sees it: what `setGoalObject(const Object*)`
/// reads (`getID`, `getPosition`) and `getGoalObject()` returns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectRef {
    /// `Object::getID()`.
    pub id: ObjectId,
    /// `Object::getPosition()`.
    pub position: Coord3D,
}

/// The two engine calls the machine itself makes (the cut points).
pub trait StateContext {
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `TheGameLogic->findObjectByID(id)`: `None` for `NULL`.
    fn find_object(&self, id: ObjectId) -> Option<ObjectRef>;
}

/// The compile-time contract of a machine's states, implemented on the
/// caller's state enum (`class State` and its derived classes). See the module
/// documentation for how to write one.
pub trait StateBehavior: Sized {
    /// The owner object and the world the callbacks work on.
    type Ctx: StateContext;
    /// The caller's transition conditions (`StateTransFuncPtr` + `userData`).
    type Condition: Copy;

    /// `onEnter`: executed once when entering the state. Default:
    /// `STATE_CONTINUE`.
    fn on_enter(
        _sm: &mut StateMachine<Self>,
        _cx: &mut Self::Ctx,
        _id: StateId,
    ) -> SmResult<StateReturn> {
        Ok(StateReturn::CONTINUE)
    }

    /// `onExit`: executed once when leaving the state. Default: nothing.
    fn on_exit(
        _sm: &mut StateMachine<Self>,
        _cx: &mut Self::Ctx,
        _id: StateId,
        _status: StateExitType,
    ) -> SmResult<()> {
        Ok(())
    }

    /// `update`: the state's behavior; decides when to change state.
    fn update(
        sm: &mut StateMachine<Self>,
        cx: &mut Self::Ctx,
        id: StateId,
    ) -> SmResult<StateReturn>;

    /// A transition condition (`it->test(this, it->userData)`) of state `id`.
    fn test(
        sm: &mut StateMachine<Self>,
        cx: &mut Self::Ctx,
        id: StateId,
        condition: Self::Condition,
    ) -> SmResult<bool>;

    /// `isIdle` (default `false`).
    fn is_idle(&self) -> bool {
        false
    }
    /// `isAttack` (default `false`).
    fn is_attack(&self) -> bool {
        false
    }
    /// `isGuardIdle` (default `false`).
    fn is_guard_idle(&self) -> bool {
        false
    }
    /// `isBusy` (default `false`): only the explicit busy state.
    fn is_busy(&self) -> bool {
        false
    }

    /// `Snapshot::crc` of the state.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()>;
    /// `Snapshot::xfer` of the state.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()>;
    /// `Snapshot::loadPostProcess` of the state.
    fn load_post_process(&mut self);
}

/// The four utility states `StateMachine.h` defines. Put one in a variant of
/// the caller's state enum and delegate to it; the version-only xfer is
/// upstream's "STUBBED" snapshot interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UtilityState {
    /// `SuccessState`: immediately succeeds.
    Success,
    /// `FailureState`: immediately fails.
    Failure,
    /// `ContinueState`: never exits (except due to conditions).
    Continue,
    /// `SleepState`: sleeps forever.
    Sleep,
}

impl UtilityState {
    /// `onEnter` (the same as `update`).
    pub fn on_enter(self) -> StateReturn {
        self.update()
    }

    /// `update`.
    pub fn update(self) -> StateReturn {
        match self {
            UtilityState::Success => StateReturn::SUCCESS,
            UtilityState::Failure => StateReturn::FAILURE,
            UtilityState::Continue => StateReturn::CONTINUE,
            UtilityState::Sleep => StateReturn::SLEEP_FOREVER,
        }
    }

    /// `crc`: stubbed (does nothing).
    pub fn crc(self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer`: `XferVersion cv = 1; v = cv; xferVersion(&v, cv)`.
    pub fn xfer(self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)
    }
}

/// One state of the machine: `State`'s data members `m_ID` (the map key),
/// `m_successStateID`, `m_failureStateID`, `m_transitions`, and the state.
struct StateEntry<S: StateBehavior> {
    success_state_id: StateId,
    failure_state_id: StateId,
    transitions: Vec<StateConditionInfo<S::Condition>>,
    /// `None` only while the value is lent to `xfer`.
    kind: Option<S>,
}

thread_local! {
    /// `static Int checkfortransitionsnum` of `friend_checkForTransitions`.
    static CHECK_FOR_TRANSITIONS_NUM: Cell<i32> = const { Cell::new(0) };
    /// `static Int checkfortransitionsnum` of `friend_checkForSleepTransitions`.
    static CHECK_FOR_SLEEP_TRANSITIONS_NUM: Cell<i32> = const { Cell::new(0) };
}

/// `StIncrementer`: increments the function-local static for the length of
/// the call (also when the call returns an error).
struct StIncrementer {
    sleep: bool,
}

impl StIncrementer {
    /// Increment; returns the guard and the new value.
    fn new(sleep: bool) -> (StIncrementer, i32) {
        let n = if sleep {
            CHECK_FOR_SLEEP_TRANSITIONS_NUM.with(|c| {
                c.set(c.get() + 1);
                c.get()
            })
        } else {
            CHECK_FOR_TRANSITIONS_NUM.with(|c| {
                c.set(c.get() + 1);
                c.get()
            })
        };
        (StIncrementer { sleep }, n)
    }
}

impl Drop for StIncrementer {
    fn drop(&mut self) {
        if self.sleep {
            CHECK_FOR_SLEEP_TRANSITIONS_NUM.with(|c| c.set(c.get() - 1));
        } else {
            CHECK_FOR_TRANSITIONS_NUM.with(|c| c.set(c.get() - 1));
        }
    }
}

/// Lends a state value to the xfer as a [`Snapshot`] (upstream passes the
/// `State*`, which is a `Snapshot`).
struct StateSnapshot<S: StateBehavior> {
    kind: S,
}

impl<S: StateBehavior> Snapshot for StateSnapshot<S> {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.kind.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.kind.xfer(xfer)
    }
    fn load_post_process(&mut self) {
        self.kind.load_post_process();
    }
}

/// `class StateMachine`: a finite state machine over the caller's states `S`.
pub struct StateMachine<S: StateBehavior> {
    /// `m_stateMap`.
    state_map: BTreeMap<StateId, StateEntry<S>>,
    /// `m_sleepTill`: if nonzero, we are sleeping until this frame.
    sleep_till: u32,
    /// `m_defaultStateID`.
    default_state_id: StateId,
    /// `m_currentState`, as the id of the state (`None` for `NULL`).
    current_state: Option<StateId>,
    /// `m_goalObjectID`.
    goal_object_id: ObjectId,
    /// `m_goalPosition`.
    goal_position: Coord3D,
    /// `m_locked`.
    locked: bool,
    /// `m_defaultStateInited`.
    default_state_inited: bool,
    /// Nesting of re-entrant calls, for [`MAX_NESTING`].
    nesting: u32,
    /// Count of undefined-upstream inputs refused.
    undefined_inputs: u32,
    /// The error behind the last `Snapshot::xfer` failure (that trait can
    /// only carry an `XferStatus`).
    snapshot_error: Option<StateMachineError>,
}

impl<S: StateBehavior> Default for StateMachine<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: StateBehavior> StateMachine<S> {
    /// `StateMachine::StateMachine`: no states, no current state, not locked,
    /// goal cleared (`internalClear`). (The owner is the context.)
    pub fn new() -> Self {
        let mut sm = StateMachine {
            state_map: BTreeMap::new(),
            sleep_till: 0,
            default_state_id: INVALID_STATE_ID,
            current_state: None,
            goal_object_id: INVALID_ID,
            goal_position: Coord3D::default(),
            locked: false,
            default_state_inited: false,
            nesting: 0,
            undefined_inputs: 0,
            snapshot_error: None,
        };
        sm.internal_clear();
        sm
    }

    /// `StateMachine::~StateMachine`: "do not allow current state to exit" in
    /// the sense of the transition logic, but `onExit(EXIT_RESET)` is called
    /// on the current state; the states are then dropped.
    pub fn destroy(mut self, cx: &mut S::Ctx) -> SmResult<()> {
        if let Some(cur) = self.current_state {
            S::on_exit(&mut self, cx, cur, StateExitType::Reset)?;
        }
        Ok(())
    }

    /// How many undefined-upstream inputs this machine has refused.
    pub fn undefined_inputs(&self) -> u32 {
        self.undefined_inputs
    }

    /// Takes the error behind the last failed `Snapshot::xfer` (`Snapshot`
    /// can only return an `XferStatus`; `ERROR_BAD_ARG` and `SC_INVALID_DATA`
    /// map to `ErrorUnknown` there). Call it after every `xfer_snapshot` of
    /// the machine, so that an error of the xfer itself is not mistaken for a
    /// stale one. [`StateMachine::xfer_machine`] returns the error whole.
    pub fn take_snapshot_error(&mut self) -> Option<StateMachineError> {
        self.snapshot_error.take()
    }

    fn undefined(&mut self, e: StateMachineError) -> StateMachineError {
        self.undefined_inputs += 1;
        e
    }

    /// Enter one level of re-entrancy.
    fn enter_nesting(&mut self) -> SmResult<()> {
        if self.nesting >= MAX_NESTING {
            return Err(self.undefined(StateMachineError::RecursionLimit));
        }
        self.nesting += 1;
        Ok(())
    }

    // ---------------------------------------------------------------------
    // The states
    // ---------------------------------------------------------------------

    /// `defineState`: record `kind` as the state with this id; `success_id` /
    /// `failure_id` are where it goes when it returns `STATE_SUCCESS` /
    /// `STATE_FAILURE`; the first state defined is the default. A duplicate id
    /// is refused (undefined upstream).
    pub fn define_state(
        &mut self,
        id: StateId,
        kind: S,
        success_id: StateId,
        failure_id: StateId,
        conditions: &[StateConditionInfo<S::Condition>],
    ) -> SmResult<()> {
        if self.state_map.contains_key(&id) {
            return Err(self.undefined(StateMachineError::DuplicateStateId(id)));
        }
        self.state_map.insert(
            id,
            StateEntry {
                success_state_id: success_id,
                failure_state_id: failure_id,
                transitions: conditions.to_vec(),
                kind: Some(kind),
            },
        );
        if self.default_state_id == INVALID_STATE_ID {
            self.default_state_id = id;
        }
        Ok(())
    }

    /// The state with this id (`internalGetState(id)->...`, for the callbacks
    /// to reach their own data). `None` for an unknown id.
    pub fn state(&self, id: StateId) -> Option<&S> {
        self.state_map.get(&id).and_then(|e| e.kind.as_ref())
    }

    /// The state with this id, mutably.
    pub fn state_mut(&mut self, id: StateId) -> Option<&mut S> {
        self.state_map.get_mut(&id).and_then(|e| e.kind.as_mut())
    }

    /// `internalGetState`: the id of the state to use for `id`. An unknown id
    /// recovers to the default state ("Attempting to recover - locating
    /// default state..."); no default either is `ERROR_BAD_ARG`.
    fn internal_get_state(&self, id: StateId) -> SmResult<StateId> {
        if self.state_map.contains_key(&id) {
            return Ok(id);
        }
        if self.state_map.contains_key(&self.default_state_id) {
            Ok(self.default_state_id)
        } else {
            Err(StateMachineError::BadArg)
        }
    }

    // ---------------------------------------------------------------------
    // Queries
    // ---------------------------------------------------------------------

    /// `getCurrentStateID`: `INVALID_STATE_ID` without a current state.
    pub fn get_current_state_id(&self) -> StateId {
        self.current_state.unwrap_or(INVALID_STATE_ID)
    }

    fn current_kind(&self) -> Option<&S> {
        self.current_state.and_then(|id| self.state(id))
    }

    /// `isInIdleState`: stateless things are considered idle.
    pub fn is_in_idle_state(&self) -> bool {
        self.current_kind().is_none_or(|s| s.is_idle())
    }

    /// `isInAttackState`: `true` without a current state, as upstream (whose
    /// comment says "idle").
    pub fn is_in_attack_state(&self) -> bool {
        self.current_kind().is_none_or(|s| s.is_attack())
    }

    /// `isInForceAttackState`: upstream calls `isIdle()` here, not a
    /// force-attack test; kept.
    pub fn is_in_force_attack_state(&self) -> bool {
        self.current_kind().is_none_or(|s| s.is_idle())
    }

    /// `isInGuardIdleState`: stateless things are not guard idle.
    pub fn is_in_guard_idle_state(&self) -> bool {
        self.current_kind().is_some_and(|s| s.is_guard_idle())
    }

    /// `isInBusyState`: only when explicitly in the busy state.
    pub fn is_in_busy_state(&self) -> bool {
        self.current_kind().is_some_and(|s| s.is_busy())
    }

    /// `lock`: a locked machine cannot be reset, cleared, or given external
    /// `setState`s, goals, etc. (The upstream message is debug only.)
    pub fn lock(&mut self) {
        self.locked = true;
    }

    /// `unlock`.
    pub fn unlock(&mut self) {
        self.locked = false;
    }

    /// `isLocked`.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// `peekSleepTill` (debug builds upstream): the frame the machine sleeps
    /// until, or `0`.
    pub fn peek_sleep_till(&self) -> u32 {
        self.sleep_till
    }

    /// `m_defaultStateID`.
    pub fn default_state_id(&self) -> StateId {
        self.default_state_id
    }

    /// `m_defaultStateInited`.
    pub fn is_default_state_inited(&self) -> bool {
        self.default_state_inited
    }

    // ---------------------------------------------------------------------
    // The goal
    // ---------------------------------------------------------------------

    /// `setGoalObject`: ignored while locked. `None` is `NULL`.
    pub fn set_goal_object(&mut self, obj: Option<&ObjectRef>) {
        if self.locked {
            return;
        }
        self.internal_set_goal_object(obj);
    }

    /// `internalSetGoalObject`: an object sets the id and the goal position;
    /// `NULL` clears only the id.
    fn internal_set_goal_object(&mut self, obj: Option<&ObjectRef>) {
        match obj {
            Some(o) => {
                self.goal_object_id = o.id;
                self.internal_set_goal_position(Some(&o.position));
            }
            None => self.goal_object_id = INVALID_ID,
        }
    }

    /// `getGoalObject` (both overloads): found through the context.
    pub fn goal_object(&self, cx: &S::Ctx) -> Option<ObjectRef> {
        cx.find_object(self.goal_object_id)
    }

    /// `m_goalObjectID`.
    pub fn goal_object_id(&self) -> ObjectId {
        self.goal_object_id
    }

    /// `setGoalPosition`: ignored while locked; `None` (`NULL`) changes
    /// nothing.
    pub fn set_goal_position(&mut self, pos: Option<&Coord3D>) {
        if self.locked {
            return;
        }
        self.internal_set_goal_position(pos);
    }

    /// `internalSetGoalPosition`: does not clear the goal object ("or
    /// everything breaks. Like construction of buildings").
    fn internal_set_goal_position(&mut self, pos: Option<&Coord3D>) {
        if let Some(p) = pos {
            self.goal_position = *p;
        }
    }

    /// `getGoalPosition`.
    pub fn goal_position(&self) -> &Coord3D {
        &self.goal_position
    }

    /// `isGoalObjectDestroyed`: true if there was a goal object and it is
    /// gone.
    pub fn is_goal_object_destroyed(&self, cx: &S::Ctx) -> bool {
        if self.goal_object_id == 0 {
            return false; // never had a goal object
        }
        self.goal_object(cx).is_none()
    }

    // ---------------------------------------------------------------------
    // Running the machine
    // ---------------------------------------------------------------------

    /// `internalClear`: clear the goal.
    fn internal_clear(&mut self) {
        self.goal_object_id = INVALID_ID;
        self.goal_position = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
    }

    /// `clear`: exit the current state (`EXIT_RESET`) and clear the goal; a
    /// locked machine cannot be cleared.
    pub fn clear(&mut self, cx: &mut S::Ctx) -> SmResult<()> {
        if self.locked {
            return Ok(());
        }
        if let Some(cur) = self.current_state {
            S::on_exit(self, cx, cur, StateExitType::Reset)?;
        }
        self.current_state = None;
        self.internal_clear();
        Ok(())
    }

    /// `resetToDefaultState`: clear, then enter the default state.
    pub fn reset_to_default_state(&mut self, cx: &mut S::Ctx) -> SmResult<StateReturn> {
        if self.locked {
            return Ok(StateReturn::FAILURE);
        }
        if !self.default_state_inited {
            return Ok(StateReturn::FAILURE);
        }
        // allow current state to exit with EXIT_RESET if present
        if let Some(cur) = self.current_state {
            S::on_exit(self, cx, cur, StateExitType::Reset)?;
        }
        self.current_state = None;
        // the old state has exited: clear the internals before the new state
        // is set, since clearing afterwards might overwrite things its
        // transition causes
        self.internal_clear();
        self.internal_set_state(cx, self.default_state_id)
    }

    /// `initDefaultState`: must be called once before use; jumps into the
    /// default state, which may fail, so it cannot be the constructor.
    pub fn init_default_state(&mut self, cx: &mut S::Ctx) -> SmResult<StateReturn> {
        if self.default_state_inited {
            return Ok(StateReturn::FAILURE);
        }
        self.default_state_inited = true;
        self.internal_set_state(cx, self.default_state_id)
    }

    /// `setState`: change the current state (`onExit` of the old, `onEnter`
    /// of the new, which may cause further changes). A locked machine ignores
    /// it and returns `STATE_CONTINUE`.
    pub fn set_state(&mut self, cx: &mut S::Ctx, new_state_id: StateId) -> SmResult<StateReturn> {
        if self.locked {
            return Ok(StateReturn::CONTINUE);
        }
        self.internal_set_state(cx, new_state_id)
    }

    /// `halt`: stop and lock the machine in preparation for deleting it; the
    /// current state is not exited, only forgotten.
    pub fn halt(&mut self) {
        self.locked = true;
        self.current_state = None;
    }

    /// `updateStateMachine`: run one step of the machine.
    pub fn update_state_machine(&mut self, cx: &mut S::Ctx) -> SmResult<StateReturn> {
        self.enter_nesting()?;
        let r = self.update_state_machine_inner(cx);
        self.nesting -= 1;
        r
    }

    fn update_state_machine_inner(&mut self, cx: &mut S::Ctx) -> SmResult<StateReturn> {
        let now = cx.frame();
        if self.sleep_till != 0 && now < self.sleep_till {
            let Some(cur) = self.current_state else {
                return Ok(StateReturn::FAILURE);
            };
            return self.check_for_sleep_transitions(
                cx,
                cur,
                StateReturn::sleep(self.sleep_till.wrapping_sub(now)),
            );
        }

        // not sleeping anymore
        self.sleep_till = 0;

        let Some(state_before_update) = self.current_state else {
            // "State machine has no current state -- did you remember to call initDefaultState?"
            return Ok(StateReturn::FAILURE);
        };

        // update() can change the current state, so save it for a moment...
        let mut status = S::update(self, cx, state_before_update)?;

        // it is possible that update() causes the state to be destroyed
        let Some(current) = self.current_state else {
            return Ok(StateReturn::FAILURE);
        };

        // if the state changed, ignore any sleep result and pretend we got
        // STATE_CONTINUE, so that the new state will be called immediately
        if state_before_update != current {
            status = StateReturn::CONTINUE;
        }

        if status.is_sleep() {
            self.sleep_till = now.wrapping_add(status.sleep_frames());
            self.check_for_sleep_transitions(
                cx,
                current,
                StateReturn::sleep(self.sleep_till.wrapping_sub(now)),
            )
        } else {
            // check for state transitions, possibly exiting this machine
            self.check_for_transitions(cx, current, status)
        }
    }

    /// `internalSetState`: change the current state (for internal use; not
    /// stopped by the lock).
    pub fn internal_set_state(
        &mut self,
        cx: &mut S::Ctx,
        new_state_id: StateId,
    ) -> SmResult<StateReturn> {
        self.enter_nesting()?;
        let r = self.internal_set_state_inner(cx, new_state_id);
        self.nesting -= 1;
        r
    }

    fn internal_set_state_inner(
        &mut self,
        cx: &mut S::Ctx,
        mut new_state_id: StateId,
    ) -> SmResult<StateReturn> {
        let mut new_state: Option<StateId> = None;

        // anytime the state changes, stop sleeping
        self.sleep_till = 0;

        // if we're not setting the "done" state ID we will continue with the actual transition
        if new_state_id != MACHINE_DONE_STATE_ID {
            // if incoming state is invalid, go to the machine's default state
            if new_state_id == INVALID_STATE_ID {
                new_state_id = self.default_state_id;
                if new_state_id == INVALID_STATE_ID {
                    // "you may NEVER set the current state to an invalid state id."
                    return Ok(StateReturn::FAILURE);
                }
            }
            // extract the state associated with the given ID
            new_state = Some(self.internal_get_state(new_state_id)?);
        }

        // invoke the old state's onExit()
        if let Some(cur) = self.current_state {
            S::on_exit(self, cx, cur, StateExitType::Normal)?;
        }

        // set the new state
        self.current_state = new_state;

        // invoke the new state's onEnter()
        let Some(state_before_enter) = self.current_state else {
            return Ok(StateReturn::CONTINUE); // irrelevant return code, but we must return something
        };

        // onEnter() could conceivably change the current state, so save it for a moment...
        let mut status = S::on_enter(self, cx, state_before_enter)?;

        // it is possible that onEnter() causes the state to be destroyed
        let Some(current) = self.current_state else {
            return Ok(StateReturn::FAILURE);
        };

        // if the state changed, ignore any sleep result and pretend we got STATE_CONTINUE
        if state_before_enter != current {
            status = StateReturn::CONTINUE;
        }

        if status.is_sleep() {
            let now = cx.frame();
            self.sleep_till = now.wrapping_add(status.sleep_frames());
            self.check_for_sleep_transitions(
                cx,
                current,
                StateReturn::sleep(self.sleep_till.wrapping_sub(now)),
            )
        } else {
            // check for state transitions, possibly exiting this machine
            self.check_for_transitions(cx, current, status)
        }
    }

    /// `State::friend_checkForTransitions( status )` for state `id`: given a
    /// return code (not a sleep), handle state transitions.
    fn check_for_transitions(
        &mut self,
        cx: &mut S::Ctx,
        id: StateId,
        status: StateReturn,
    ) -> SmResult<StateReturn> {
        let (_inc, depth) = StIncrementer::new(false);
        if depth >= MAX_TRANSITION_DEPTH {
            // "checkfortransitionsnum is > 20"
            return Ok(StateReturn::FAILURE);
        }

        let (success_id, failure_id) = match self.state_map.get(&id) {
            Some(e) => (e.success_state_id, e.failure_state_id),
            None => return Ok(StateReturn::CONTINUE),
        };

        // handle transitions
        match status {
            StateReturn::SUCCESS => {
                // check if machine should exit
                if success_id == EXIT_MACHINE_WITH_SUCCESS {
                    self.internal_set_state(cx, MACHINE_DONE_STATE_ID)?;
                    return Ok(StateReturn::SUCCESS);
                } else if success_id == EXIT_MACHINE_WITH_FAILURE {
                    self.internal_set_state(cx, MACHINE_DONE_STATE_ID)?;
                    return Ok(StateReturn::FAILURE);
                }
                // move to new state
                return self.internal_set_state(cx, success_id);
            }
            StateReturn::FAILURE => {
                if failure_id == EXIT_MACHINE_WITH_SUCCESS {
                    self.internal_set_state(cx, MACHINE_DONE_STATE_ID)?;
                    return Ok(StateReturn::SUCCESS);
                } else if failure_id == EXIT_MACHINE_WITH_FAILURE {
                    self.internal_set_state(cx, MACHINE_DONE_STATE_ID)?;
                    return Ok(StateReturn::FAILURE);
                }
                return self.internal_set_state(cx, failure_id);
            }
            StateReturn::CONTINUE => {
                // check transition condition list
                let mut i = 0;
                while let Some(info) = self
                    .state_map
                    .get(&id)
                    .and_then(|e| e.transitions.get(i))
                    .copied()
                {
                    i += 1;
                    if S::test(self, cx, id, info.test)? {
                        // test returned true, change to associated state;
                        // check if machine should exit
                        if info.to_state_id == EXIT_MACHINE_WITH_SUCCESS {
                            return Ok(StateReturn::SUCCESS);
                        } else if info.to_state_id == EXIT_MACHINE_WITH_FAILURE {
                            return Ok(StateReturn::FAILURE);
                        }
                        // move to new state
                        return self.internal_set_state(cx, info.to_state_id);
                    }
                }
            }
            // any other value: the switch has no case for it
            _ => {}
        }

        // the machine keeps running
        Ok(StateReturn::CONTINUE)
    }

    /// `State::friend_checkForSleepTransitions( status )` for state `id`:
    /// given a sleep, handle state transitions.
    fn check_for_sleep_transitions(
        &mut self,
        cx: &mut S::Ctx,
        id: StateId,
        status: StateReturn,
    ) -> SmResult<StateReturn> {
        let (_inc, depth) = StIncrementer::new(true);
        if depth >= MAX_TRANSITION_DEPTH {
            // "checkforsleeptransitionsnum is > 20"
            return Ok(StateReturn::FAILURE);
        }

        // check transition condition list
        let mut i = 0;
        while let Some(info) = self
            .state_map
            .get(&id)
            .and_then(|e| e.transitions.get(i))
            .copied()
        {
            i += 1;
            if !S::test(self, cx, id, info.test)? {
                continue;
            }
            // test returned true, change to associated state;
            // check if machine should exit
            if info.to_state_id == EXIT_MACHINE_WITH_SUCCESS {
                return Ok(StateReturn::SUCCESS);
            } else if info.to_state_id == EXIT_MACHINE_WITH_FAILURE {
                return Ok(StateReturn::FAILURE);
            }
            // move to new state
            return self.internal_set_state(cx, info.to_state_id);
        }

        Ok(status)
    }

    // ---------------------------------------------------------------------
    // Snapshot
    // ---------------------------------------------------------------------

    /// `xferSnapshot( state )` for the state with this id: the value is lent
    /// to the xfer and put back, whatever the result.
    fn xfer_state(&mut self, xfer: &mut dyn Xfer, id: StateId) -> XferResult<()> {
        let Some(kind) = self.state_map.get_mut(&id).and_then(|e| e.kind.take()) else {
            return Err(XferStatus::InvalidParameters);
        };
        let mut snap = StateSnapshot { kind };
        let r = xfer.xfer_snapshot(&mut snap);
        if let Some(e) = self.state_map.get_mut(&id) {
            e.kind = Some(snap.kind);
        }
        r
    }

    /// `StateMachine::xfer`, version 1: the sleep frame, the default state
    /// id, the current state id (on load the machine jumps into that state
    /// without `onEnter`/`onExit`: it was already active when saved), the
    /// snapshot-all-states flag, the current state's snapshot (or all of
    /// them, when a loaded stream says so), the goal, the lock and the
    /// default-state flag.
    pub fn xfer_machine(&mut self, xfer: &mut dyn Xfer) -> SmResult<()> {
        // version
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        xfer.xfer_unsigned_int(&mut self.sleep_till)?;
        xfer.xfer_unsigned_int(&mut self.default_state_id)?;
        let mut cur_state_id = self.get_current_state_id();
        xfer.xfer_unsigned_int(&mut cur_state_id)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            // we jump into the current state; onEnter and onExit are not
            // called, because the state was already active when we saved
            self.current_state = Some(self.internal_get_state(cur_state_id)?);
        }

        let mut snapshot_all_states = false;
        xfer.xfer_bool(&mut snapshot_all_states)?;
        if snapshot_all_states {
            let count = self.state_map.len() as i32;
            let mut save_count = count;
            xfer.xfer_int(&mut save_count)?;
            if save_count != count {
                // "State count mismatch"
                return Err(StateMachineError::InvalidData);
            }
            let ids: Vec<StateId> = self.state_map.keys().copied().collect();
            for state_id in ids {
                let mut id = state_id;
                xfer.xfer_unsigned_int(&mut id)?;
                if id != state_id {
                    // "State ID mismatch"
                    return Err(StateMachineError::InvalidData);
                }
                self.xfer_state(xfer, state_id)?;
            }
        } else {
            if self.current_state.is_none() {
                // "currentState was NULL on xfer, trying to heal..."
                self.current_state = Some(self.internal_get_state(self.default_state_id)?);
            }
            if let Some(cur) = self.current_state {
                self.xfer_state(xfer, cur)?;
            }
        }

        xfer.xfer_object_id(&mut self.goal_object_id)?;
        xfer.xfer_coord3d(&mut self.goal_position)?;
        xfer.xfer_bool(&mut self.locked)?;
        xfer.xfer_bool(&mut self.default_state_inited)?;
        Ok(())
    }
}

/// The machine is a `Snapshot`, so a state that encloses one (or the AI that
/// owns one) can `xfer_snapshot` it. `crc` does nothing and `load_post_process`
/// does nothing, as upstream's. `Snapshot::xfer` can only return an
/// `XferStatus`: for the two errors upstream throws that are not one it
/// returns [`XferStatus::ErrorUnknown`] and keeps the real error in
/// [`StateMachine::take_snapshot_error`].
impl<S: StateBehavior> Snapshot for StateMachine<S> {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.snapshot_error = None;
        match self.xfer_machine(xfer) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.snapshot_error = Some(e);
                match e {
                    StateMachineError::Xfer(s) => Err(s),
                    _ => Err(XferStatus::ErrorUnknown),
                }
            }
        }
    }

    fn load_post_process(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::xfer_crc::XferCrc;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;

    /// A world with a frame and a few objects.
    struct World {
        frame: u32,
        objects: Vec<ObjectRef>,
        log: Vec<String>,
    }

    impl StateContext for World {
        fn frame(&self) -> u32 {
            self.frame
        }
        fn find_object(&self, id: ObjectId) -> Option<ObjectRef> {
            self.objects.iter().find(|o| o.id == id).copied()
        }
    }

    fn world() -> World {
        World {
            frame: 100,
            objects: Vec::new(),
            log: Vec::new(),
        }
    }

    /// States: a utility one, a counting one that returns what it is told,
    /// and one that jumps to another state from `update`.
    #[derive(Clone, Debug)]
    enum T {
        Util(UtilityState),
        Ret {
            enter: StateReturn,
            update: StateReturn,
            idle: bool,
            attack: bool,
            guard: bool,
            busy: bool,
            count: u32,
        },
        Jump {
            to: StateId,
        },
        Again,
    }

    #[derive(Clone, Copy)]
    enum C {
        Always,
        Never,
    }

    fn ret(enter: StateReturn, update: StateReturn) -> T {
        T::Ret {
            enter,
            update,
            idle: false,
            attack: false,
            guard: false,
            busy: false,
            count: 0,
        }
    }

    impl StateBehavior for T {
        type Ctx = World;
        type Condition = C;
        fn on_enter(
            sm: &mut StateMachine<Self>,
            cx: &mut World,
            id: StateId,
        ) -> SmResult<StateReturn> {
            cx.log.push(format!("enter {id}"));
            match sm.state_mut(id) {
                Some(T::Util(u)) => Ok(u.on_enter()),
                Some(T::Ret { enter, count, .. }) => {
                    *count += 1;
                    Ok(*enter)
                }
                Some(T::Again) => sm.set_state(cx, id),
                _ => Ok(StateReturn::CONTINUE),
            }
        }
        fn on_exit(
            _sm: &mut StateMachine<Self>,
            cx: &mut World,
            id: StateId,
            status: StateExitType,
        ) -> SmResult<()> {
            cx.log.push(format!("exit {id} {status:?}"));
            Ok(())
        }
        fn update(
            sm: &mut StateMachine<Self>,
            cx: &mut World,
            id: StateId,
        ) -> SmResult<StateReturn> {
            cx.log.push(format!("update {id}"));
            match sm.state_mut(id).map(|s| s.clone()) {
                Some(T::Util(u)) => Ok(u.update()),
                Some(T::Ret { update, .. }) => Ok(update),
                Some(T::Jump { to }) => {
                    sm.set_state(cx, to)?;
                    Ok(StateReturn::sleep(50)) // ignored: the state changed
                }
                _ => Ok(StateReturn::FAILURE),
            }
        }
        fn test(
            _sm: &mut StateMachine<Self>,
            _cx: &mut World,
            _id: StateId,
            c: C,
        ) -> SmResult<bool> {
            Ok(matches!(c, C::Always))
        }
        fn is_idle(&self) -> bool {
            matches!(self, T::Ret { idle: true, .. })
        }
        fn is_attack(&self) -> bool {
            matches!(self, T::Ret { attack: true, .. })
        }
        fn is_guard_idle(&self) -> bool {
            matches!(self, T::Ret { guard: true, .. })
        }
        fn is_busy(&self) -> bool {
            matches!(self, T::Ret { busy: true, .. })
        }
        fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
            match self {
                T::Util(u) => u.crc(xfer),
                T::Ret { count, .. } => xfer.xfer_unsigned_int(count),
                _ => Ok(()),
            }
        }
        fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
            match self {
                T::Util(u) => u.xfer(xfer),
                T::Ret { count, .. } => {
                    let mut v: XferVersion = 1;
                    xfer.xfer_version(&mut v, 1)?;
                    xfer.xfer_unsigned_int(count)
                }
                _ => Ok(()),
            }
        }
        fn load_post_process(&mut self) {}
    }

    fn two_state_machine() -> StateMachine<T> {
        let mut sm = StateMachine::<T>::new();
        sm.define_state(
            1,
            ret(StateReturn::CONTINUE, StateReturn::SUCCESS),
            2,
            INVALID_STATE_ID,
            &[],
        )
        .unwrap();
        sm.define_state(
            2,
            ret(StateReturn::CONTINUE, StateReturn::CONTINUE),
            INVALID_STATE_ID,
            INVALID_STATE_ID,
            &[],
        )
        .unwrap();
        sm
    }

    #[test]
    fn state_return_helpers() {
        assert!(StateReturn::sleep(5).is_sleep());
        assert!(!StateReturn::CONTINUE.is_sleep());
        assert!(!StateReturn::SUCCESS.is_sleep());
        assert_eq!(StateReturn::sleep(5).sleep_frames(), 5);
        assert_eq!(
            StateReturn::sleep(5).convert_sleep_to_continue(),
            StateReturn::CONTINUE
        );
        assert_eq!(
            StateReturn::FAILURE.convert_sleep_to_continue(),
            StateReturn::FAILURE
        );
        assert_eq!(
            StateReturn::min_sleep(3, StateReturn::sleep(9)),
            StateReturn::sleep(3)
        );
        assert_eq!(
            StateReturn::min_sleep(30, StateReturn::sleep(9)),
            StateReturn::sleep(9)
        );
        assert_eq!(
            StateReturn::min_sleep(3, StateReturn::SUCCESS),
            StateReturn::SUCCESS
        );
        // a sleep of more than i32::MAX frames is a negative value, not a sleep
        assert!(!StateReturn::sleep(0x8000_0000).is_sleep());
        assert_eq!(StateReturn::SLEEP_FOREVER.sleep_frames(), 0x3fff_ffff);
    }

    #[test]
    fn utility_states() {
        assert_eq!(UtilityState::Success.on_enter(), StateReturn::SUCCESS);
        assert_eq!(UtilityState::Failure.update(), StateReturn::FAILURE);
        assert_eq!(UtilityState::Continue.update(), StateReturn::CONTINUE);
        assert_eq!(UtilityState::Sleep.update(), StateReturn::SLEEP_FOREVER);
        let mut save = XferSave::new();
        save.open(b"x").unwrap();
        UtilityState::Sleep.xfer(&mut save).unwrap();
        UtilityState::Sleep.crc(&mut save).unwrap();
        save.close().unwrap();
        assert_eq!(save.bytes(), &[1]);
        // a version above the current one is refused
        let mut load = XferLoad::new(vec![2]);
        load.open(b"x").unwrap();
        assert_eq!(
            UtilityState::Success.xfer(&mut load),
            Err(XferStatus::InvalidVersion)
        );
    }

    #[test]
    fn enter_update_exit_protocol() {
        let mut cx = world();
        let mut sm = two_state_machine();
        // not initialised: update fails, reset fails
        assert_eq!(sm.update_state_machine(&mut cx), Ok(StateReturn::FAILURE));
        assert_eq!(sm.reset_to_default_state(&mut cx), Ok(StateReturn::FAILURE));
        assert_eq!(sm.get_current_state_id(), INVALID_STATE_ID);
        assert_eq!(sm.init_default_state(&mut cx), Ok(StateReturn::CONTINUE));
        assert_eq!(sm.init_default_state(&mut cx), Ok(StateReturn::FAILURE)); // twice
        assert_eq!(sm.get_current_state_id(), 1);
        // update returns SUCCESS, so the machine moves to state 2
        assert_eq!(sm.update_state_machine(&mut cx), Ok(StateReturn::CONTINUE));
        assert_eq!(sm.get_current_state_id(), 2);
        assert_eq!(cx.log, ["enter 1", "update 1", "exit 1 Normal", "enter 2"]);
        // reset exits with EXIT_RESET and re-enters the default state
        cx.log.clear();
        assert_eq!(
            sm.reset_to_default_state(&mut cx),
            Ok(StateReturn::CONTINUE)
        );
        assert_eq!(cx.log, ["exit 2 Reset", "enter 1"]);
        assert_eq!(sm.default_state_id(), 1);
        assert!(sm.is_default_state_inited());
        // clear exits and leaves no state
        cx.log.clear();
        sm.clear(&mut cx).unwrap();
        assert_eq!(sm.get_current_state_id(), INVALID_STATE_ID);
        assert_eq!(cx.log, ["exit 1 Reset"]);
        // destroy exits the current state
        sm.set_state(&mut cx, 2).unwrap();
        cx.log.clear();
        sm.destroy(&mut cx).unwrap();
        assert_eq!(cx.log, ["exit 2 Reset"]);
    }

    #[test]
    fn sleeping_states_wake_on_their_frame() {
        let mut cx = world();
        let mut sm = StateMachine::<T>::new();
        sm.define_state(
            1,
            ret(StateReturn::sleep(10), StateReturn::sleep(5)),
            2,
            2,
            &[],
        )
        .unwrap();
        sm.define_state(
            2,
            T::Util(UtilityState::Sleep),
            INVALID_STATE_ID,
            INVALID_STATE_ID,
            &[],
        )
        .unwrap();
        // entering sleeps 10 frames from frame 100
        assert_eq!(sm.init_default_state(&mut cx), Ok(StateReturn::sleep(10)));
        assert_eq!(sm.peek_sleep_till(), 110);
        cx.frame = 104;
        assert_eq!(sm.update_state_machine(&mut cx), Ok(StateReturn::sleep(6)));
        assert!(!cx.log.iter().any(|l| l == "update 1"));
        cx.frame = 110;
        assert_eq!(sm.update_state_machine(&mut cx), Ok(StateReturn::sleep(5)));
        assert_eq!(sm.peek_sleep_till(), 115);
        // an external state change stops the sleep
        sm.set_state(&mut cx, 2).unwrap();
        assert_eq!(sm.peek_sleep_till(), 0x3fff_ffff + 110);
        // the sleep frame arithmetic wraps; a sleep ending at frame 0 is no sleep
        cx.frame = u32::MAX - 4;
        sm.set_state(&mut cx, 1).unwrap();
        assert_eq!(sm.peek_sleep_till(), 5);
    }

    #[test]
    fn a_state_that_changes_state_in_update_ignores_its_sleep() {
        let mut cx = world();
        let mut sm = StateMachine::<T>::new();
        sm.define_state(
            1,
            T::Jump { to: 2 },
            INVALID_STATE_ID,
            INVALID_STATE_ID,
            &[],
        )
        .unwrap();
        sm.define_state(
            2,
            ret(StateReturn::CONTINUE, StateReturn::CONTINUE),
            INVALID_STATE_ID,
            INVALID_STATE_ID,
            &[],
        )
        .unwrap();
        sm.init_default_state(&mut cx).unwrap();
        assert_eq!(sm.update_state_machine(&mut cx), Ok(StateReturn::CONTINUE));
        assert_eq!(sm.get_current_state_id(), 2);
        assert_eq!(sm.peek_sleep_till(), 0);
    }

    #[test]
    fn exits_conditions_and_the_transition_depth_limit() {
        let mut cx = world();
        // success -> EXIT_MACHINE_WITH_SUCCESS; failure -> EXIT_MACHINE_WITH_FAILURE
        let mut sm = StateMachine::<T>::new();
        sm.define_state(
            1,
            T::Util(UtilityState::Success),
            EXIT_MACHINE_WITH_SUCCESS,
            EXIT_MACHINE_WITH_FAILURE,
            &[],
        )
        .unwrap();
        assert_eq!(sm.init_default_state(&mut cx), Ok(StateReturn::SUCCESS));
        assert_eq!(sm.get_current_state_id(), INVALID_STATE_ID); // the "done" state has no state
        let mut sm = StateMachine::<T>::new();
        sm.define_state(
            1,
            T::Util(UtilityState::Failure),
            EXIT_MACHINE_WITH_SUCCESS,
            EXIT_MACHINE_WITH_SUCCESS,
            &[],
        )
        .unwrap();
        assert_eq!(sm.init_default_state(&mut cx), Ok(StateReturn::SUCCESS));
        let mut sm = StateMachine::<T>::new();
        sm.define_state(
            1,
            T::Util(UtilityState::Success),
            EXIT_MACHINE_WITH_FAILURE,
            EXIT_MACHINE_WITH_FAILURE,
            &[],
        )
        .unwrap();
        assert_eq!(sm.init_default_state(&mut cx), Ok(StateReturn::FAILURE));
        let mut sm = StateMachine::<T>::new();
        sm.define_state(
            1,
            T::Util(UtilityState::Failure),
            1,
            EXIT_MACHINE_WITH_FAILURE,
            &[],
        )
        .unwrap();
        assert_eq!(sm.init_default_state(&mut cx), Ok(StateReturn::FAILURE));

        // conditions: the first true one wins; exit targets return directly
        for (to, want, cur) in [
            (3, StateReturn::CONTINUE, 3),
            (EXIT_MACHINE_WITH_SUCCESS, StateReturn::SUCCESS, 1),
            (EXIT_MACHINE_WITH_FAILURE, StateReturn::FAILURE, 1),
        ] {
            let mut sm = StateMachine::<T>::new();
            let conds = [
                StateConditionInfo {
                    test: C::Never,
                    to_state_id: 2,
                },
                StateConditionInfo {
                    test: C::Always,
                    to_state_id: to,
                },
                StateConditionInfo {
                    test: C::Always,
                    to_state_id: 2,
                },
            ];
            sm.define_state(
                1,
                ret(StateReturn::CONTINUE, StateReturn::CONTINUE),
                INVALID_STATE_ID,
                INVALID_STATE_ID,
                &conds,
            )
            .unwrap();
            sm.define_state(
                2,
                ret(StateReturn::CONTINUE, StateReturn::CONTINUE),
                INVALID_STATE_ID,
                INVALID_STATE_ID,
                &[],
            )
            .unwrap();
            sm.define_state(
                3,
                ret(StateReturn::CONTINUE, StateReturn::CONTINUE),
                INVALID_STATE_ID,
                INVALID_STATE_ID,
                &[],
            )
            .unwrap();
            assert_eq!(sm.init_default_state(&mut cx), Ok(want));
            assert_eq!(sm.get_current_state_id(), cur);
        }
        // a sleeping state is checked against its conditions too
        let mut sm = StateMachine::<T>::new();
        sm.define_state(
            1,
            T::Util(UtilityState::Sleep),
            INVALID_STATE_ID,
            INVALID_STATE_ID,
            &[StateConditionInfo {
                test: C::Always,
                to_state_id: EXIT_MACHINE_WITH_FAILURE,
            }],
        )
        .unwrap();
        assert_eq!(sm.init_default_state(&mut cx), Ok(StateReturn::FAILURE));

        // a ring of states that all succeed at once stops at the depth limit
        let mut sm = StateMachine::<T>::new();
        sm.define_state(1, T::Util(UtilityState::Success), 2, 2, &[])
            .unwrap();
        sm.define_state(2, T::Util(UtilityState::Success), 1, 1, &[])
            .unwrap();
        assert_eq!(sm.init_default_state(&mut cx), Ok(StateReturn::FAILURE));
        CHECK_FOR_TRANSITIONS_NUM.with(|c| assert_eq!(c.get(), 0));
        CHECK_FOR_SLEEP_TRANSITIONS_NUM.with(|c| assert_eq!(c.get(), 0));
        // an update that returns an odd negative value keeps running
        let mut sm = StateMachine::<T>::new();
        sm.define_state(1, ret(StateReturn::CONTINUE, StateReturn(-7)), 2, 2, &[])
            .unwrap();
        sm.init_default_state(&mut cx).unwrap();
        assert_eq!(sm.update_state_machine(&mut cx), Ok(StateReturn::CONTINUE));
        assert_eq!(sm.get_current_state_id(), 1);
    }

    #[test]
    fn unknown_ids_recover_to_the_default_state() {
        let mut cx = world();
        let mut sm = two_state_machine();
        sm.init_default_state(&mut cx).unwrap();
        sm.set_state(&mut cx, 2).unwrap();
        sm.set_state(&mut cx, 77).unwrap(); // unknown: the default state
        assert_eq!(sm.get_current_state_id(), 1);
        sm.set_state(&mut cx, 2).unwrap();
        sm.set_state(&mut cx, INVALID_STATE_ID).unwrap(); // invalid: the default state
        assert_eq!(sm.get_current_state_id(), 1);
        // MACHINE_DONE_STATE_ID leaves no state
        sm.set_state(&mut cx, MACHINE_DONE_STATE_ID).unwrap();
        assert_eq!(sm.get_current_state_id(), INVALID_STATE_ID);
        // no states at all: an unknown id has no default to recover to
        let mut empty = StateMachine::<T>::new();
        assert_eq!(empty.set_state(&mut cx, 5), Err(StateMachineError::BadArg));
        assert_eq!(
            empty.internal_set_state(&mut cx, INVALID_STATE_ID),
            Ok(StateReturn::FAILURE)
        );
        assert_eq!(empty.undefined_inputs(), 0);
    }

    #[test]
    fn lock_and_halt() {
        let mut cx = world();
        let mut sm = two_state_machine();
        sm.init_default_state(&mut cx).unwrap();
        sm.lock();
        assert!(sm.is_locked());
        assert_eq!(sm.set_state(&mut cx, 2), Ok(StateReturn::CONTINUE));
        assert_eq!(sm.get_current_state_id(), 1);
        assert_eq!(sm.reset_to_default_state(&mut cx), Ok(StateReturn::FAILURE));
        sm.clear(&mut cx).unwrap();
        assert_eq!(sm.get_current_state_id(), 1);
        sm.set_goal_position(Some(&Coord3D {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        }));
        assert_eq!(sm.goal_position().x, 0.0);
        // internal_set_state ignores the lock
        sm.internal_set_state(&mut cx, 2).unwrap();
        assert_eq!(sm.get_current_state_id(), 2);
        sm.unlock();
        assert!(!sm.is_locked());
        // halt locks and forgets the state without exiting it
        cx.log.clear();
        sm.halt();
        assert!(sm.is_locked());
        assert_eq!(sm.get_current_state_id(), INVALID_STATE_ID);
        assert!(cx.log.is_empty());
        assert_eq!(sm.update_state_machine(&mut cx), Ok(StateReturn::FAILURE));
    }

    #[test]
    fn queries_and_their_stateless_defaults() {
        let mut cx = world();
        let mut sm = StateMachine::<T>::new();
        // no state: idle, attack and force-attack are true, guard idle and busy false
        assert!(sm.is_in_idle_state() && sm.is_in_attack_state() && sm.is_in_force_attack_state());
        assert!(!sm.is_in_guard_idle_state() && !sm.is_in_busy_state());
        let flags = |idle, attack, guard, busy| T::Ret {
            enter: StateReturn::CONTINUE,
            update: StateReturn::CONTINUE,
            idle,
            attack,
            guard,
            busy,
            count: 0,
        };
        sm.define_state(1, flags(false, false, false, false), 1, 1, &[])
            .unwrap();
        sm.define_state(2, flags(true, false, true, false), 1, 1, &[])
            .unwrap();
        sm.define_state(3, flags(false, true, false, true), 1, 1, &[])
            .unwrap();
        sm.init_default_state(&mut cx).unwrap();
        assert!(!sm.is_in_idle_state() && !sm.is_in_attack_state() && !sm.is_in_busy_state());
        sm.set_state(&mut cx, 2).unwrap();
        assert!(
            sm.is_in_idle_state() && sm.is_in_force_attack_state() && sm.is_in_guard_idle_state()
        );
        sm.set_state(&mut cx, 3).unwrap();
        assert!(sm.is_in_attack_state() && sm.is_in_busy_state());
        assert!(!sm.is_in_force_attack_state()); // isIdle(), not an attack test
    }

    #[test]
    fn goals() {
        let mut cx = world();
        cx.objects.push(ObjectRef {
            id: 7,
            position: Coord3D {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
        });
        let mut sm = two_state_machine();
        assert!(!sm.is_goal_object_destroyed(&cx)); // never had one
        sm.set_goal_object(Some(&cx.objects[0].clone()));
        assert_eq!(sm.goal_object_id(), 7);
        assert_eq!(
            *sm.goal_position(),
            Coord3D {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
        );
        assert_eq!(sm.goal_object(&cx).map(|o| o.id), Some(7));
        assert!(!sm.is_goal_object_destroyed(&cx));
        // setting a position leaves the object; a null position changes nothing
        sm.set_goal_position(Some(&Coord3D {
            x: 9.0,
            y: 9.0,
            z: 9.0,
        }));
        sm.set_goal_position(None);
        assert_eq!(sm.goal_object_id(), 7);
        assert_eq!(sm.goal_position().x, 9.0);
        // the object is destroyed
        cx.objects.clear();
        assert!(sm.goal_object(&cx).is_none());
        assert!(sm.is_goal_object_destroyed(&cx));
        // a null goal object clears the id only
        sm.set_goal_object(None);
        assert_eq!(sm.goal_object_id(), INVALID_ID);
        assert_eq!(sm.goal_position().x, 9.0);
        assert!(!sm.is_goal_object_destroyed(&cx));
    }

    #[test]
    fn xfer_round_trip_and_the_healing_save() {
        let mut cx = world();
        let mut sm = two_state_machine();
        sm.init_default_state(&mut cx).unwrap();
        sm.set_goal_position(Some(&Coord3D {
            x: 1.5,
            y: -2.0,
            z: 0.25,
        }));
        sm.set_state(&mut cx, 2).unwrap();
        let mut save = XferSave::new();
        save.open(b"sm").unwrap();
        sm.xfer(&mut save).unwrap();
        save.close().unwrap();
        let bytes = save.bytes().to_vec();
        // version, sleepTill, default, current, flag, state (version, count), goal object, position, locked, inited
        assert_eq!(bytes.len(), 1 + 4 + 4 + 4 + 1 + (1 + 4) + 4 + 12 + 1 + 1);
        assert_eq!(&bytes[9..13], &2u32.to_le_bytes());

        let mut other = two_state_machine();
        let mut load = XferLoad::new(bytes.clone());
        load.open(b"sm").unwrap();
        other.xfer(&mut load).unwrap();
        assert_eq!(other.get_current_state_id(), 2);
        assert_eq!(other.goal_position().x, 1.5);
        assert!(other.is_default_state_inited());
        assert_eq!(load.post_process_requests(), 1);
        Snapshot::load_post_process(&mut other);

        // the light CRC covers nothing; the deep walk does
        let mut crc = XferCrc::new_without_nan_guard();
        crc.xfer_snapshot(&mut sm).unwrap();
        assert_eq!(crc.get_crc(), 0);
        let mut crc = XferCrc::new_without_nan_guard();
        Snapshot::xfer(&mut sm, &mut crc).unwrap();
        assert_ne!(crc.get_crc(), 0);

        // saving a machine with no current state writes INVALID_STATE_ID, then heals
        let mut fresh = two_state_machine();
        let mut save = XferSave::new();
        save.open(b"sm").unwrap();
        fresh.xfer(&mut save).unwrap();
        save.close().unwrap();
        assert_eq!(&save.bytes()[9..13], &INVALID_STATE_ID.to_le_bytes());
        assert_eq!(fresh.get_current_state_id(), 1);

        // a truncated stream and a bad version are xfer errors
        let mut load = XferLoad::new(bytes[..7].to_vec());
        load.open(b"sm").unwrap();
        assert_eq!(
            other.xfer_machine(&mut load),
            Err(StateMachineError::Xfer(XferStatus::ReadError))
        );
        assert_eq!(
            Snapshot::xfer(&mut other, &mut XferLoad::new(Vec::new())),
            Err(XferStatus::ReadError)
        );
        assert_eq!(
            other.take_snapshot_error(),
            Some(StateMachineError::Xfer(XferStatus::ReadError))
        );
    }

    #[test]
    fn xfer_all_states_branch_on_load() {
        let mut cx = world();
        let mut sm = two_state_machine();
        sm.init_default_state(&mut cx).unwrap();
        // a stream whose flag says "all states": count, then id + snapshot of each
        let mut s = vec![1u8];
        s.extend(0u32.to_le_bytes());
        s.extend(1u32.to_le_bytes());
        s.extend(1u32.to_le_bytes());
        s.push(1); // snapshotAllStates
        let tail = |mut v: Vec<u8>| {
            v.extend(0i32.to_le_bytes()); // goal object
            v.extend([0u8; 12]); // goal position
            v.extend([0, 1]); // locked, inited
            v
        };
        let mut good = s.clone();
        good.extend(2i32.to_le_bytes());
        for id in [1u32, 2] {
            good.extend(id.to_le_bytes());
            good.push(1);
            good.extend((10 * id).to_le_bytes());
        }
        let mut load = XferLoad::new(tail(good));
        load.open(b"x").unwrap();
        sm.xfer_machine(&mut load).unwrap();
        assert_eq!(
            sm.state(1).map(|s| matches!(s, T::Ret { count: 10, .. })),
            Some(true)
        );
        assert_eq!(
            sm.state(2).map(|s| matches!(s, T::Ret { count: 20, .. })),
            Some(true)
        );
        assert_eq!(load.post_process_requests(), 2);
        // wrong count
        let mut bad = s.clone();
        bad.extend(3i32.to_le_bytes());
        let mut load = XferLoad::new(bad);
        load.open(b"x").unwrap();
        assert_eq!(
            sm.xfer_machine(&mut load),
            Err(StateMachineError::InvalidData)
        );
        assert_eq!(
            Snapshot::xfer(&mut sm, &mut {
                let mut l = XferLoad::new(tail(s.clone()));
                l.open(b"x").unwrap();
                l
            }),
            Err(XferStatus::ErrorUnknown)
        );
        assert_eq!(
            sm.take_snapshot_error(),
            Some(StateMachineError::InvalidData)
        );
        // wrong id
        let mut bad = s.clone();
        bad.extend(2i32.to_le_bytes());
        bad.extend(5u32.to_le_bytes());
        let mut load = XferLoad::new(bad);
        load.open(b"x").unwrap();
        assert_eq!(
            sm.xfer_machine(&mut load),
            Err(StateMachineError::InvalidData)
        );
        // an unknown current state id recovers to the default one; with no states it is BAD_ARG
        let mut unknown = vec![1u8];
        unknown.extend(0u32.to_le_bytes());
        unknown.extend(1u32.to_le_bytes());
        unknown.extend(555u32.to_le_bytes());
        let mut load = XferLoad::new(unknown.clone());
        load.open(b"x").unwrap();
        let _ = sm.xfer_machine(&mut load);
        assert_eq!(sm.get_current_state_id(), 1);
        let mut empty = StateMachine::<T>::new();
        let mut load = XferLoad::new(unknown);
        load.open(b"x").unwrap();
        assert_eq!(
            empty.xfer_machine(&mut load),
            Err(StateMachineError::BadArg)
        );
        // xfer_state on an id that does not exist
        let mut save = XferSave::new();
        assert_eq!(
            sm.xfer_state(&mut save, 9),
            Err(XferStatus::InvalidParameters)
        );
    }

    #[test]
    fn undefined_upstream_inputs_are_errors_and_counted() {
        let mut cx = world();
        let mut sm = two_state_machine();
        assert_eq!(
            sm.define_state(1, T::Util(UtilityState::Success), 1, 1, &[]),
            Err(StateMachineError::DuplicateStateId(1))
        );
        assert!(StateMachineError::DuplicateStateId(1).is_undefined_upstream());
        assert!(!StateMachineError::BadArg.is_undefined_upstream());
        assert_eq!(sm.undefined_inputs(), 1);
        assert!(matches!(sm.state(1), Some(T::Ret { .. }))); // the first state is kept
                                                             // a state that sets itself from onEnter would recurse forever
        let mut sm = StateMachine::<T>::new();
        sm.define_state(1, T::Again, INVALID_STATE_ID, INVALID_STATE_ID, &[])
            .unwrap();
        assert_eq!(
            sm.init_default_state(&mut cx),
            Err(StateMachineError::RecursionLimit)
        );
        assert!(sm.undefined_inputs() >= 1);
        assert_eq!(
            format!("{}", StateMachineError::RecursionLimit),
            "RecursionLimit"
        );
        let e: StateMachineError = XferStatus::Eof.into();
        assert_eq!(e, StateMachineError::Xfer(XferStatus::Eof));
        // the machine can be used again after the error
        sm.nesting = 0;
        assert_eq!(
            StateMachine::<T>::default().get_current_state_id(),
            INVALID_STATE_ID
        );
    }

    #[test]
    fn state_access_follows_the_map() {
        let mut sm = two_state_machine();
        assert!(sm.state(3).is_none());
        assert!(sm.state_mut(3).is_none());
        if let Some(T::Ret { count, .. }) = sm.state_mut(1) {
            *count = 4;
        }
        assert!(matches!(sm.state(1), Some(T::Ret { count: 4, .. })));
    }
}
