// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::state_machine` against upstream
//! `StateMachine.cpp` (commit 0a05454d), compiled unchanged by g++ with
//! `StateMachine.h` and the Xfer headers unchanged and the real `XferSave`,
//! `XferLoad`, `XferCRC` and `XferDeepCRC` driving it (see
//! `differential/wrap_statemachine.cpp`; the engine pieces it needs are the
//! stand-ins of `differential/shim_statemachine`: the frame and
//! `findObjectByID` of `GameLogic`, `Object::getID`/`getPosition`).
//!
//! # Method
//!
//! A **case** is a random state graph and a random list of operations on the
//! machine. The same case is run by an interpreter on each side, against the
//! real C++ classes and against the Rust translation, with the same test
//! states on both: the four utility states of `StateMachine.h`
//! (`SuccessState`, `FailureState`, `ContinueState`, `SleepState`, here
//! [`UtilityState`]) and a **scripted state** whose `onEnter`, `update` and
//! `onExit` return values come from per-callback lists used cyclically, may
//! carry a **re-entrant action** (the machine's own `setState`,
//! `internalSetState`, `clear`, `resetToDefaultState`, `halt`, `lock`,
//! `unlock`, `setGoalObject`, `setGoalPosition`, called from inside the
//! callback, at most 2 deep), count every callback, and xfer that counter.
//! Conditions answer true/false from cyclic sequences. The state graph is
//! random: successor and failure ids that are defined, unknown, invalid,
//! `EXIT_MACHINE_WITH_SUCCESS`/`_FAILURE` or the "done" id; 0-3 conditions per
//! state with the same kinds of targets; rings of states that all succeed at
//! once (which run into the `checkfortransitionsnum >= 20` limit); sleeps of
//! 1-39 frames, forever, `i32::MAX`, and odd negative returns.
//!
//! The operations: `initDefaultState`, `updateStateMachine`, `setState`,
//! `internalSetState`, `resetToDefaultState`, `clear`, `lock`, `unlock`,
//! `halt`, `setGoalObject` (live, destroyed or null object), `setGoalPosition`
//! (or null), the frame (including values that make `m_sleepTill` wrap),
//! destroying and reviving objects, **save** (real `XferSave`), the three
//! **CRCs** (light `Snapshot::crc`, `xfer()` through `XferCRC`, and through
//! `XferDeepCRC`, with its bytes), **load** of the last save mutated (cut
//! short, version, sleep frame, default state id and current state id
//! changed) and **load** of crafted streams that set `snapshotAllStates`
//! (upstream only ever saves `false`, but it reads the flag back), with a
//! wrong count or a wrong id as often as a right stream. Every case ends with
//! the destructor (`onExit(EXIT_RESET)` of the current state).
//!
//! Four **categories** of results are compared word for word, each its own
//! check, over all cases:
//!
//! 1. *events*: every callback made (enter, update and exit with their
//!    arguments and returns, every condition evaluated and its answer, every
//!    re-entrant action), i.e. every transition the machine makes, in order;
//! 2. *results*: per operation, what was thrown (type and value:
//!    `ERROR_BAD_ARG`, `SC_INVALID_DATA`, an `XferStatus`), the return code,
//!    and the machine's whole view (current state id, `m_sleepTill`, default
//!    state, goal object id and position, lock, default-state flag, the four
//!    state queries including the force-attack quirk, `isGoalObjectDestroyed`,
//!    `getGoalObject`, the post-process hand-offs of the load, and each
//!    scripted state's counters);
//! 3. *bytes*: the saved stream and the deep-CRC file of every save and CRC
//!    operation;
//! 4. *CRCs*: the three CRC values.
//!
//! For each category a **planted bug** is run: the Rust result altered by one
//! word (one bit of one value) must be caught by the same comparison.
//! Separately, bugs were planted in the translation itself, one at a time,
//! and the whole test run (the table under *Results*), then removed.
//!
//! # Results
//!
//! 2026-10-18, upstream `0a05454d`, g++ (x86-64, SSE), seed `0x5A7E0001`:
//!
//! - 6,000 random cases; 5,897 run on both sides; 103 define a state id twice
//!   (undefined upstream): the Rust refused each (`DuplicateStateId`, counted:
//!   103 refusals in 103 cases) and the C++ was not run.
//! - 131,922 operations, 746,502 callbacks (`onEnter`/`update`/`onExit`),
//!   278,595 condition evaluations, 66,723 re-entrant actions; 412 cases with
//!   an operation that entered 20 or more states (the
//!   `checkfortransitionsnum >= 20` limit); 44,040 operations that left the
//!   machine asleep; 6,324 saves, 3,632 CRC operations (three CRCs each),
//!   18,981 loads; `ERROR_BAD_ARG` thrown 3,169 times, `SC_INVALID_DATA`
//!   1,041, `XFER_READ_ERROR` 9,258, `XFER_INVALID_VERSION` 996.
//! - **No mismatch** in any of the four categories (events, results, bytes,
//!   CRCs); each comparison also catches its planted bug in all 5,897 cases.
//! - Bugs planted in the translation itself, one at a time, each run through
//!   this test and then removed (the first failing category is the one named):
//!   wake one frame late (`now <= m_sleepTill`): events, 57 cases;
//!   `isInForceAttackState` testing `isAttack`: results, 2,831; `xfer` writing
//!   the default state id before the sleep frame: events (a load reads the
//!   swapped words), 1,829; the light CRC covering `m_sleepTill`: CRCs, 958;
//!   the transition limit 21 instead of 20: events, 526; a state change in
//!   `update` no longer cancelling the sleep: events, 19; `resetToDefaultState`
//!   clearing the goal after entering the default state: results, 57.
//! - Found: the short-read difference described under *Restrictions*.
//!
//! # Restrictions (where the C++ is undefined)
//!
//! - A state id defined twice (`defineState`): upstream keeps the first and
//!   orphans the second; the Rust refuses it. About one case in 50 has one;
//!   the Rust must return `DuplicateStateId` (counted), and the case is not
//!   run on the C++.
//! - **A short read of a field.** Upstream's `XferLoad::xferImplementation`
//!   `fread`s straight into the destination, so a stream that ends inside a
//!   field (3 of the 4 bytes of the default state id) overwrites its low bytes
//!   before the `XFER_READ_ERROR`; the Rust typed Xfer methods read into a
//!   temporary and assign only on success, so the field keeps its old value.
//!   Found by this test (case 48 of the first run, a stream cut after 8
//!   bytes). It belongs to the shared Xfer translation (`common::xfer`), not
//!   to the machine, and is only visible in the state of a machine whose load
//!   has just failed. Streams are therefore cut only at field boundaries.
//! - **A `bool` byte other than 0 or 1** (upstream stores the raw byte into a
//!   `bool`; the Rust reads any nonzero byte as `true`). A stream whose layout
//!   does not match the machine's current state is read from shifted offsets
//!   and reaches such bytes; so mutations of a saved stream never change the
//!   default or current state id (which change the layout), and the streams
//!   that do (unknown, invalid, wrong ids; `snapshotAllStates`) are crafted
//!   whole, with the layout the machine will read.
//! - The C++ loads through a `GuardedLoad` (wrapper) that notices both
//!   situations and logs a marker; the test fails if one is ever generated, so
//!   the restrictions are checked, not assumed.
//! - Re-entrant actions are limited to 2 deep in the test states on both
//!   sides; a state that sets itself from `onEnter` forever overflows the C++
//!   stack, and the Rust stops at `MAX_NESTING` (tested without the C++ in
//!   the unit tests of `state_machine.rs`).
//!
//! # Script encoding
//!
//! `[nConds] ([len] bits..)*  [nObjects] ([id] [x] [y] [z])*  [nStates]
//! ([kind] [id] [succ] [fail] [nTrans] ([cond] [to])*  (kind 4 only: [flags]
//! then enter, update, exit as [n] ([ret] [act] [arg])*))*  [nOps] ops...`.
//! Kinds 0-3 are the utility states, 4 the scripted state. Ops: see `Op`.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::errors::ERROR_BAD_ARG;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::state_machine::*;
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer, XferResult, XferStatus, XferVersion};
use rts_lockstep_engine::common::xfer_crc::{XferCrc, XferDeepCrc};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use std::collections::{BTreeMap, BTreeSet};
use support::{Check, Gen};

// ---------------------------------------------------------------------------
// The case
// ---------------------------------------------------------------------------

/// One callback's script entry: the return value and the action.
#[derive(Clone, Copy, Debug)]
struct Act {
    ret: i32,
    act: u32,
    arg: u32,
}

#[derive(Clone, Debug)]
struct StateSpec {
    /// 0 Success, 1 Failure, 2 Continue, 3 Sleep, 4 scripted.
    kind: u32,
    id: u32,
    succ: u32,
    fail: u32,
    trans: Vec<(u32, u32)>,
    flags: u32,
    enter: Vec<Act>,
    update: Vec<Act>,
    exit: Vec<Act>,
}

#[derive(Clone, Debug)]
enum Op {
    Init,
    Update,
    SetState(u32),
    Reset,
    Clear,
    Lock,
    Unlock,
    Halt,
    GoalObject(u32),
    GoalPosition(bool, [u32; 3]),
    Frame(u32),
    Kill(u32),
    Revive(u32),
    Save,
    Crc,
    /// Load the last save mutated: kind (1 truncate to a, 2 set byte a to b,
    /// 3 set the u32 at a to b), a, b.
    LoadLast(u32, u32, u32),
    LoadBytes(Vec<u8>),
    Internal(u32),
}

#[derive(Clone, Debug)]
struct Case {
    conds: Vec<Vec<u32>>,
    objects: Vec<(i32, [u32; 3])>,
    states: Vec<StateSpec>,
    ops: Vec<Op>,
}

impl Case {
    fn has_duplicate_ids(&self) -> bool {
        let mut seen = BTreeSet::new();
        self.states.iter().any(|s| !seen.insert(s.id))
    }
}

fn encode(case: &Case) -> Vec<u32> {
    let mut w = vec![case.conds.len() as u32];
    for c in &case.conds {
        w.push(c.len() as u32);
        w.extend(c);
    }
    w.push(case.objects.len() as u32);
    for (id, pos) in &case.objects {
        w.push(*id as u32);
        w.extend(pos);
    }
    w.push(case.states.len() as u32);
    for s in &case.states {
        w.extend([s.kind, s.id, s.succ, s.fail, s.trans.len() as u32]);
        for (c, to) in &s.trans {
            w.extend([*c, *to]);
        }
        if s.kind == 4 {
            w.push(s.flags);
            for list in [&s.enter, &s.update, &s.exit] {
                w.push(list.len() as u32);
                for a in list {
                    w.extend([a.ret as u32, a.act, a.arg]);
                }
            }
        }
    }
    w.push(case.ops.len() as u32);
    for op in &case.ops {
        match op {
            Op::Init => w.push(0),
            Op::Update => w.push(1),
            Op::SetState(id) => w.extend([2, *id]),
            Op::Reset => w.push(3),
            Op::Clear => w.push(4),
            Op::Lock => w.push(5),
            Op::Unlock => w.push(6),
            Op::Halt => w.push(7),
            Op::GoalObject(i) => w.extend([8, *i]),
            Op::GoalPosition(has, p) => {
                w.extend([9, u32::from(*has)]);
                w.extend(p);
            }
            Op::Frame(f) => w.extend([10, *f]),
            Op::Kill(i) => w.extend([11, *i]),
            Op::Revive(i) => w.extend([12, *i]),
            Op::Save => w.push(13),
            Op::Crc => w.push(14),
            Op::LoadLast(k, a, b) => w.extend([15, *k, *a, *b]),
            Op::LoadBytes(b) => {
                w.extend([16, b.len() as u32]);
                w.extend(b.iter().map(|&x| u32::from(x)));
            }
            Op::Internal(id) => w.extend([17, *id]),
        }
    }
    w
}

// ---------------------------------------------------------------------------
// The Rust model of the test states (mirror of differential/wrap_statemachine.cpp)
// ---------------------------------------------------------------------------

const T_ENTER_BEGIN: u32 = 10;
const T_ENTER_END: u32 = 11;
const T_UPDATE_BEGIN: u32 = 12;
const T_UPDATE_END: u32 = 13;
const T_EXIT_BEGIN: u32 = 14;
const T_EXIT_END: u32 = 15;
const T_COND: u32 = 16;
const T_ACTION: u32 = 17;
const MAX_ACT_DEPTH: u32 = 2;

#[derive(Clone, Debug)]
struct Scripted {
    flags: u32,
    enter: Vec<Act>,
    update: Vec<Act>,
    exit: Vec<Act>,
    n_enter: u32,
    n_update: u32,
    n_exit: u32,
    counter: u32,
}

#[derive(Clone, Debug)]
enum TState {
    Util(UtilityState),
    Scripted(Scripted),
}

/// The condition: an index into the cyclic answer sequences.
#[derive(Clone, Copy, Debug)]
enum Cond {
    Sequence(u32),
}

struct World {
    frame: u32,
    objects: Vec<ObjectRef>,
    live: BTreeSet<i32>,
    conds: Vec<Vec<u32>>,
    cond_calls: Vec<u32>,
    act_depth: u32,
    no_actions: bool,
    ev: Vec<u32>,
}

impl StateContext for World {
    fn frame(&self) -> u32 {
        self.frame
    }
    fn find_object(&self, id: ObjectId) -> Option<ObjectRef> {
        if !self.live.contains(&id) {
            return None;
        }
        self.objects.iter().find(|o| o.id == id).copied()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Enter,
    Update,
    Exit,
}

/// `doAction` of the wrapper.
fn do_action(sm: &mut StateMachine<TState>, cx: &mut World, id: StateId, e: Act) -> SmResult<()> {
    if cx.no_actions || cx.act_depth >= MAX_ACT_DEPTH || e.act == 0 {
        return Ok(());
    }
    cx.act_depth += 1;
    cx.ev.extend([T_ACTION, id, e.act, e.arg]);
    let r = match e.act {
        1 => sm.set_state(cx, e.arg).map(|_| ()),
        2 => sm.internal_set_state(cx, e.arg).map(|_| ()),
        3 => sm.clear(cx),
        4 => sm.reset_to_default_state(cx).map(|_| ()),
        5 => {
            sm.halt();
            Ok(())
        }
        6 => {
            sm.lock();
            Ok(())
        }
        7 => {
            sm.unlock();
            Ok(())
        }
        8 => {
            let o = cx.objects.get(e.arg as usize).copied();
            sm.set_goal_object(o.as_ref());
            Ok(())
        }
        9 => {
            let p = Coord3D {
                x: e.arg as f32,
                y: e.arg as f32 * 2.0,
                z: -(e.arg as f32),
            };
            sm.set_goal_position(Some(&p));
            Ok(())
        }
        _ => Ok(()),
    };
    cx.act_depth -= 1;
    r
}

/// `ScriptState::onEnter` / `update` / `onExit`.
fn scripted_call(
    sm: &mut StateMachine<TState>,
    cx: &mut World,
    id: StateId,
    which: Which,
    exit_status: u32,
) -> SmResult<StateReturn> {
    let (begin, end) = match which {
        Which::Enter => (T_ENTER_BEGIN, T_ENTER_END),
        Which::Update => (T_UPDATE_BEGIN, T_UPDATE_END),
        Which::Exit => (T_EXIT_BEGIN, T_EXIT_END),
    };
    cx.ev.extend([begin, id]);
    if which == Which::Exit {
        cx.ev.push(exit_status);
    }
    let Some(TState::Scripted(s)) = sm.state_mut(id) else {
        return Ok(StateReturn::CONTINUE);
    };
    let (list, n) = match which {
        Which::Enter => (&s.enter, &mut s.n_enter),
        Which::Update => (&s.update, &mut s.n_update),
        Which::Exit => (&s.exit, &mut s.n_exit),
    };
    let mut e = Act {
        ret: 0,
        act: 0,
        arg: 0,
    };
    if !list.is_empty() {
        e = list[(*n % list.len() as u32) as usize];
        *n = n.wrapping_add(1);
    }
    s.counter = s.counter.wrapping_add(1);
    do_action(sm, cx, id, e)?;
    if which == Which::Exit {
        cx.ev.extend([end, id]);
    } else {
        cx.ev.extend([end, id, e.ret as u32]);
    }
    Ok(StateReturn(e.ret))
}

impl StateBehavior for TState {
    type Ctx = World;
    type Condition = Cond;

    fn on_enter(sm: &mut StateMachine<Self>, cx: &mut World, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(TState::Util(u)) => Ok(u.on_enter()),
            _ => scripted_call(sm, cx, id, Which::Enter, 0),
        }
    }

    fn on_exit(
        sm: &mut StateMachine<Self>,
        cx: &mut World,
        id: StateId,
        status: StateExitType,
    ) -> SmResult<()> {
        // UtilityState has no onExit
        if matches!(sm.state(id), Some(TState::Scripted(_))) {
            let s = match status {
                StateExitType::Normal => 0,
                StateExitType::Reset => 1,
            };
            scripted_call(sm, cx, id, Which::Exit, s)?;
        }
        Ok(())
    }

    fn update(sm: &mut StateMachine<Self>, cx: &mut World, id: StateId) -> SmResult<StateReturn> {
        match sm.state(id) {
            Some(TState::Util(u)) => Ok(u.update()),
            _ => scripted_call(sm, cx, id, Which::Update, 0),
        }
    }

    fn test(
        _sm: &mut StateMachine<Self>,
        cx: &mut World,
        id: StateId,
        cond: Cond,
    ) -> SmResult<bool> {
        let Cond::Sequence(idx) = cond;
        let seq = &cx.conds[idx as usize];
        let r = if seq.is_empty() {
            false
        } else {
            let k = cx.cond_calls[idx as usize];
            cx.cond_calls[idx as usize] = k.wrapping_add(1);
            seq[(k % seq.len() as u32) as usize] != 0
        };
        cx.ev.extend([T_COND, id, idx, u32::from(r)]);
        Ok(r)
    }

    fn is_idle(&self) -> bool {
        matches!(self, TState::Scripted(s) if s.flags & 1 != 0)
    }
    fn is_attack(&self) -> bool {
        matches!(self, TState::Scripted(s) if s.flags & 2 != 0)
    }
    fn is_guard_idle(&self) -> bool {
        matches!(self, TState::Scripted(s) if s.flags & 4 != 0)
    }
    fn is_busy(&self) -> bool {
        matches!(self, TState::Scripted(s) if s.flags & 8 != 0)
    }

    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            TState::Util(u) => u.crc(xfer),
            TState::Scripted(s) => {
                xfer.xfer_unsigned_int(&mut s.counter)?;
                xfer.xfer_unsigned_int(&mut s.n_update)
            }
        }
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        match self {
            TState::Util(u) => u.xfer(xfer),
            TState::Scripted(s) => {
                let mut v: XferVersion = 1;
                xfer.xfer_version(&mut v, 1)?;
                xfer.xfer_unsigned_int(&mut s.counter)
            }
        }
    }
    fn load_post_process(&mut self) {}
}

// ---------------------------------------------------------------------------
// The Rust interpreter
// ---------------------------------------------------------------------------

/// The four result categories.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Logs {
    ev: Vec<u32>,
    res: Vec<u32>,
    byt: Vec<u32>,
    crc: Vec<u32>,
}

thread_local! {
    /// Every (type, value) status the interpreter logged, for the coverage tallies.
    static STATUSES: std::cell::RefCell<Vec<[u32; 2]>> = const { std::cell::RefCell::new(Vec::new()) };
}

struct RustRun {
    logs: Logs,
    /// The status of every operation (type, value), (0, 0) for none.
    statuses: Vec<[u32; 2]>,
    /// The most `onEnter`s any one operation made.
    max_enters_per_op: u32,
    /// Operations after which the machine was asleep.
    sleeping_ops: u32,
    /// Undefined-upstream inputs the machine refused.
    undefined: u32,
}

/// (type, value) of an error as the wrapper logs it: 1 `ErrorCode`, 2
/// `XferStatus`, 3 `SaveCode`.
fn status_words(e: &StateMachineError) -> [u32; 2] {
    match e {
        StateMachineError::BadArg => [1, ERROR_BAD_ARG],
        StateMachineError::InvalidData => [3, 6], // SC_INVALID_DATA
        StateMachineError::Xfer(s) => [2, *s as u32],
        StateMachineError::DuplicateStateId(_) | StateMachineError::RecursionLimit => [4, 0],
    }
}

fn push_status(res: &mut Vec<u32>, r: &SmResult<()>) -> bool {
    let (words, ok) = match r {
        Ok(()) => ([0, 0], true),
        Err(e) => (status_words(e), false),
    };
    res.extend(words);
    STATUSES.with(|s| s.borrow_mut().push(words));
    ok
}

/// The status of an xfer run through `Xfer::xfer_snapshot` /
/// `Snapshot::xfer`: the machine's own error when it has one.
fn xfer_status(sm: &mut StateMachine<TState>, r: XferResult<()>) -> SmResult<()> {
    let own = sm.take_snapshot_error();
    match (r, own) {
        (Ok(()), _) => Ok(()),
        (Err(_), Some(e)) => Err(e),
        (Err(s), None) => Err(StateMachineError::Xfer(s)),
    }
}

/// The C++ `mutate`.
fn mutate(s: &mut Vec<u8>, kind: u32, a: u32, b: u32) {
    let a = a as usize;
    match kind {
        1 => {
            if a < s.len() {
                s.truncate(a);
            }
        }
        2 if a < s.len() => s[a] = b as u8,
        3 if a + 4 <= s.len() => s[a..a + 4].copy_from_slice(&b.to_le_bytes()),
        _ => {}
    }
}

fn view(
    res: &mut Vec<u32>,
    sm: &StateMachine<TState>,
    cx: &World,
    post_process: u32,
    scripted_ids: &BTreeSet<u32>,
) {
    res.push(sm.get_current_state_id());
    res.push(sm.peek_sleep_till());
    res.push(sm.default_state_id());
    res.push(sm.goal_object_id() as u32);
    let p = sm.goal_position();
    res.extend([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
    res.push(u32::from(sm.is_locked()));
    res.push(u32::from(sm.is_default_state_inited()));
    res.push(u32::from(sm.is_in_idle_state()));
    res.push(u32::from(sm.is_in_attack_state()));
    res.push(u32::from(sm.is_in_force_attack_state()));
    res.push(u32::from(sm.is_in_guard_idle_state()));
    res.push(u32::from(sm.is_in_busy_state()));
    res.push(u32::from(sm.is_locked()));
    res.push(u32::from(sm.is_goal_object_destroyed(cx)));
    res.push(sm.goal_object(cx).map_or(0, |o| o.id as u32));
    res.push(post_process);
    for id in scripted_ids {
        if let Some(TState::Scripted(s)) = sm.state(*id) {
            res.extend([s.counter, s.n_enter, s.n_update, s.n_exit]);
        }
    }
}

/// The width in words of the event that starts with `tag`.
fn event_width(tag: u32) -> usize {
    match tag {
        T_ENTER_BEGIN | T_UPDATE_BEGIN | T_EXIT_END => 2,
        T_ENTER_END | T_UPDATE_END | T_EXIT_BEGIN => 3,
        T_COND | T_ACTION => 4,
        _ => 1,
    }
}

fn count_enters(ev: &[u32]) -> u32 {
    let (mut i, mut n) = (0, 0);
    while i < ev.len() {
        if ev[i] == T_ENTER_BEGIN {
            n += 1;
        }
        i += event_width(ev[i]);
    }
    n
}

fn run_rust(case: &Case) -> RustRun {
    let mut cx = World {
        frame: 0,
        objects: case
            .objects
            .iter()
            .map(|(id, p)| ObjectRef {
                id: *id,
                position: Coord3D {
                    x: f32::from_bits(p[0]),
                    y: f32::from_bits(p[1]),
                    z: f32::from_bits(p[2]),
                },
            })
            .collect(),
        live: case.objects.iter().map(|(id, _)| *id).collect(),
        conds: case.conds.clone(),
        cond_calls: vec![0; case.conds.len()],
        act_depth: 0,
        no_actions: false,
        ev: Vec::new(),
    };
    let mut sm = StateMachine::<TState>::new();
    let mut undefined = 0;
    let mut scripted_ids = BTreeSet::new();
    for s in &case.states {
        let kind = match s.kind {
            0 => TState::Util(UtilityState::Success),
            1 => TState::Util(UtilityState::Failure),
            2 => TState::Util(UtilityState::Continue),
            3 => TState::Util(UtilityState::Sleep),
            _ => {
                scripted_ids.insert(s.id);
                TState::Scripted(Scripted {
                    flags: s.flags,
                    enter: s.enter.clone(),
                    update: s.update.clone(),
                    exit: s.exit.clone(),
                    n_enter: 0,
                    n_update: 0,
                    n_exit: 0,
                    counter: 0,
                })
            }
        };
        let conds: Vec<StateConditionInfo<Cond>> = s
            .trans
            .iter()
            .map(|(c, to)| StateConditionInfo {
                test: Cond::Sequence(*c),
                to_state_id: *to,
            })
            .collect();
        if let Err(e) = sm.define_state(s.id, kind, s.succ, s.fail, &conds) {
            assert!(e.is_undefined_upstream());
            undefined += 1;
        }
    }
    STATUSES.with(|s| s.borrow_mut().clear());
    let mut max_enters_per_op = 0u32;
    let mut sleeping_ops = 0u32;
    let mut res: Vec<u32> = Vec::new();
    let mut byt: Vec<u32> = Vec::new();
    let mut crc: Vec<u32> = Vec::new();
    let mut last_save: Vec<u8> = Vec::new();
    let mut post_process = 0u32;
    for op in &case.ops {
        let code = match op {
            Op::Init => 0,
            Op::Update => 1,
            Op::SetState(_) => 2,
            Op::Reset => 3,
            Op::Clear => 4,
            Op::Lock => 5,
            Op::Unlock => 6,
            Op::Halt => 7,
            Op::GoalObject(_) => 8,
            Op::GoalPosition(..) => 9,
            Op::Frame(_) => 10,
            Op::Kill(_) => 11,
            Op::Revive(_) => 12,
            Op::Save => 13,
            Op::Crc => 14,
            Op::LoadLast(..) => 15,
            Op::LoadBytes(_) => 16,
            Op::Internal(_) => 17,
        };
        res.push(100 + code);
        let ev_before = cx.ev.len();
        let ret_op = |res: &mut Vec<u32>, r: SmResult<StateReturn>| match r {
            Ok(v) => {
                push_status(res, &Ok(()));
                res.push(v.0 as u32);
            }
            Err(e) => {
                push_status(res, &Err(e));
            }
        };
        match op {
            Op::Init => {
                let r = sm.init_default_state(&mut cx);
                ret_op(&mut res, r);
            }
            Op::Update => {
                let r = sm.update_state_machine(&mut cx);
                ret_op(&mut res, r);
            }
            Op::SetState(id) => {
                let r = sm.set_state(&mut cx, *id);
                ret_op(&mut res, r);
            }
            Op::Reset => {
                let r = sm.reset_to_default_state(&mut cx);
                ret_op(&mut res, r);
            }
            Op::Clear => {
                let r = sm.clear(&mut cx);
                push_status(&mut res, &r);
            }
            Op::Lock => {
                sm.lock();
                push_status(&mut res, &Ok(()));
            }
            Op::Unlock => {
                sm.unlock();
                push_status(&mut res, &Ok(()));
            }
            Op::Halt => {
                sm.halt();
                push_status(&mut res, &Ok(()));
            }
            Op::GoalObject(i) => {
                let o = cx.objects.get(*i as usize).copied();
                sm.set_goal_object(o.as_ref());
                push_status(&mut res, &Ok(()));
            }
            Op::GoalPosition(has, b) => {
                let p = Coord3D {
                    x: f32::from_bits(b[0]),
                    y: f32::from_bits(b[1]),
                    z: f32::from_bits(b[2]),
                };
                sm.set_goal_position(if *has { Some(&p) } else { None });
                push_status(&mut res, &Ok(()));
            }
            Op::Frame(f) => {
                cx.frame = *f;
                push_status(&mut res, &Ok(()));
            }
            Op::Kill(i) => {
                if let Some(o) = cx.objects.get(*i as usize) {
                    cx.live.remove(&o.id);
                }
                push_status(&mut res, &Ok(()));
            }
            Op::Revive(i) => {
                if let Some(o) = cx.objects.get(*i as usize) {
                    cx.live.insert(o.id);
                }
                push_status(&mut res, &Ok(()));
            }
            Op::Save => {
                let mut xs = XferSave::new();
                let r = xs.open(b"s").and_then(|_| xs.xfer_snapshot(&mut sm));
                let r = xfer_status(&mut sm, r);
                push_status(&mut res, &r);
                let _ = xs.close();
                last_save = xs.bytes().to_vec();
                byt.push(last_save.len() as u32);
                byt.extend(last_save.iter().map(|&b| u32::from(b)));
            }
            Op::Crc => {
                let mut xc = XferCrc::new_without_nan_guard();
                let r = xc.open(b"crc").and_then(|_| xc.xfer_snapshot(&mut sm));
                let r = xfer_status(&mut sm, r);
                if push_status(&mut res, &r) {
                    crc.push(xc.get_crc());
                }
                let mut xc2 = XferCrc::new_without_nan_guard();
                let r = xc2
                    .open(b"crc2")
                    .and_then(|_| Snapshot::xfer(&mut sm, &mut xc2));
                let r = xfer_status(&mut sm, r);
                if push_status(&mut res, &r) {
                    crc.push(xc2.get_crc());
                }
                let mut xd = XferDeepCrc::new_without_nan_guard();
                let r = xd
                    .open(b"deep")
                    .and_then(|_| Snapshot::xfer(&mut sm, &mut xd));
                let r = xfer_status(&mut sm, r);
                let ok = push_status(&mut res, &r);
                let _ = xd.close();
                if ok {
                    crc.push(xd.get_crc());
                }
                byt.push(xd.bytes().len() as u32);
                byt.extend(xd.bytes().iter().map(|&b| u32::from(b)));
            }
            Op::LoadLast(..) | Op::LoadBytes(_) => {
                let bytes = match op {
                    Op::LoadLast(k, a, b) => {
                        let mut s = last_save.clone();
                        mutate(&mut s, *k, *a, *b);
                        s
                    }
                    Op::LoadBytes(b) => b.clone(),
                    _ => unreachable!(),
                };
                let mut xl = XferLoad::new(bytes);
                let r = xl.open(b"l").and_then(|_| xl.xfer_snapshot(&mut sm));
                let r = xfer_status(&mut sm, r);
                push_status(&mut res, &r);
                let _ = xl.close();
                post_process = xl.post_process_requests() as u32;
            }
            Op::Internal(id) => {
                let r = sm.internal_set_state(&mut cx, *id);
                ret_op(&mut res, r);
            }
        }
        max_enters_per_op = max_enters_per_op.max(count_enters(&cx.ev[ev_before..]));
        sleeping_ops += u32::from(sm.peek_sleep_till() != 0);
        view(&mut res, &sm, &cx, post_process, &scripted_ids);
    }
    // the destructor: onExit(EXIT_RESET) of the current state, no actions
    res.push(200);
    cx.no_actions = true;
    let _ = sm.destroy(&mut cx);
    res.push(201);
    let statuses = STATUSES.with(|s| s.borrow().clone());
    RustRun {
        logs: Logs {
            ev: cx.ev,
            res,
            byt,
            crc,
        },
        statuses,
        max_enters_per_op,
        sleeping_ops,
        undefined,
    }
}

// ---------------------------------------------------------------------------
// The C++ side
// ---------------------------------------------------------------------------

/// The C++ keeps its frame, its object registry, its condition counters and
/// the post-process count in globals, and the wrapper writes scratch files
/// next to a path: runs are serialised, and share one log buffer.
static CPP: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

const LOG_CAP: usize = 1 << 24;

fn run_cpp(case: &Case) -> Logs {
    let script = encode(case);
    let mut buf = CPP.lock().unwrap_or_else(|e| e.into_inner());
    if buf.len() < LOG_CAP {
        buf.resize(LOG_CAP, 0);
    }
    let mut path = std::env::temp_dir();
    path.push(format!("rts-lockstep-sm-{}", std::process::id()));
    let cpath = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let n = unsafe {
        cpp::ref_sm_run(
            cpath.as_ptr(),
            script.as_ptr(),
            script.len() as i32,
            buf.as_mut_ptr(),
            LOG_CAP as i32,
        )
    };
    assert!(n >= 0, "malformed script");
    assert!(
        (n as usize) <= LOG_CAP,
        "log overflow: {n} words for {case:#?}"
    );
    let mut at = 0usize;
    let mut take = || {
        let len = buf[at] as usize;
        let v = buf[at + 1..at + 1 + len].to_vec();
        at += 1 + len;
        v
    };
    let logs = Logs {
        ev: take(),
        res: take(),
        byt: take(),
        crc: take(),
    };
    assert!(
        !logs.res.contains(&0xDEAD_0001),
        "the generator made a load that is undefined upstream: {case:#?}"
    );
    for ext in ["sav", "deep", "load"] {
        let _ = std::fs::remove_file(format!("{}.{ext}", path.to_str().unwrap()));
    }
    logs
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Where a saved stream may be cut short: only at the boundary of a field
/// (see *Restrictions*). `0 1 5 9 13 14` end the header fields; 15 ends the
/// version byte of a state that has a counter, or the whole state otherwise;
/// 19, 23, 27, 31, 35 are 4-byte boundaries in both layouts.
const CUTS: [u32; 12] = [0, 1, 5, 9, 13, 14, 15, 19, 23, 27, 31, 35];
const ID_POOL: [u32; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 100_000, 0];

fn pick_target(g: &mut Gen, ids: &[u32]) -> u32 {
    let r = g.below(100);
    if r < 60 && !ids.is_empty() {
        ids[g.below(ids.len() as u32) as usize]
    } else if r < 68 {
        INVALID_STATE_ID
    } else if r < 76 {
        EXIT_MACHINE_WITH_SUCCESS
    } else if r < 84 {
        EXIT_MACHINE_WITH_FAILURE
    } else if r < 90 {
        MACHINE_DONE_STATE_ID
    } else if r < 96 {
        77
    } else {
        0xFFFF_FFFF
    }
}

fn gen_ret(g: &mut Gen) -> i32 {
    match g.below(100) {
        0..=39 => 0,
        40..=54 => -1,
        55..=64 => -2,
        65..=84 => 1 + g.below(39) as i32,
        85..=87 => 0x3fff_ffff,
        88 => i32::MAX,
        89 => i32::MIN,
        90 => -7,
        _ => 1 + g.below(3) as i32,
    }
}

fn gen_act(g: &mut Gen, ids: &[u32], n_objects: u32) -> Act {
    let ret = gen_ret(g);
    let act = if g.below(100) < 80 { 0 } else { 1 + g.below(9) };
    let arg = match act {
        1 | 2 => pick_target(g, ids),
        8 => g.below(n_objects + 2),
        9 => g.below(100),
        _ => 0,
    };
    Act { ret, act, arg }
}

fn gen_acts(g: &mut Gen, ids: &[u32], n_objects: u32, max: u32) -> Vec<Act> {
    (0..g.below(max + 1))
        .map(|_| gen_act(g, ids, n_objects))
        .collect()
}

fn gen_pos_bits(g: &mut Gen) -> [u32; 3] {
    [
        g.uniform(-500.0, 500.0).to_bits(),
        g.uniform(-500.0, 500.0).to_bits(),
        g.uniform(-50.0, 50.0).to_bits(),
    ]
}

/// The size of a state's snapshot: a version byte, and a counter for the
/// scripted state.
fn snapshot_size(kind: u32) -> usize {
    if kind == 4 {
        5
    } else {
        1
    }
}

/// A stream for the machine of the case (all its states defined once) that
/// the C++ reads without an undefined or unspecified step: the header (with a
/// default state and a current state id that are defined, unknown or
/// invalid), the snapshot of the state the machine will resolve them to --
/// or, with `snapshotAllStates` set, of all states, with a wrong count or a
/// wrong id as often as not -- and a valid tail (`bool`s 0 or 1); sometimes cut
/// at a field boundary.
fn craft_stream(g: &mut Gen, states: &BTreeMap<u32, u32>, ids: &[u32]) -> Vec<u8> {
    let valid = |g: &mut Gen| {
        if ids.is_empty() {
            77
        } else {
            ids[g.below(ids.len() as u32) as usize]
        }
    };
    let all = g.below(2) == 0;
    let d = match g.below(8) {
        0 => 77,
        1 => INVALID_STATE_ID,
        2 => 0xFFFF_FFFF,
        _ => valid(g),
    };
    let c = match g.below(8) {
        0 => 77,
        1 => INVALID_STATE_ID,
        _ => valid(g),
    };
    let mut b = vec![if g.below(10) == 0 { 0u8 } else { 1u8 }];
    b.extend(g.below(3).to_le_bytes()); // sleepTill
    b.extend(d.to_le_bytes());
    b.extend(c.to_le_bytes());
    b.push(u8::from(all));
    // an unknown current state recovers to the (just loaded) default state
    let resolved = if states.contains_key(&c) {
        Some(c)
    } else if states.contains_key(&d) {
        Some(d)
    } else {
        None
    };
    let Some(resolved) = resolved else {
        return b; // ERROR_BAD_ARG is thrown here, nothing more is read
    };
    let mut cut_list: &[usize] = &[0, 1, 5, 9, 13, 14, 15];
    if all {
        cut_list = &[0, 1, 5, 9, 13, 14, 18];
        let fault = g.below(4); // 0..1 none, 2 count, 3 id
        let count = states.len() as i32 + i32::from(fault == 2);
        b.extend(count.to_le_bytes());
        for (n, (id, kind)) in states.iter().enumerate() {
            let id = if fault == 3 && n == 0 { id + 1 } else { *id };
            b.extend(id.to_le_bytes());
            b.push(1); // state version
            if *kind == 4 {
                b.extend(g.below(1000).to_le_bytes()); // counter
            }
        }
    } else {
        b.push(1); // state version
        if snapshot_size(states[&resolved]) == 5 {
            b.extend(g.below(1000).to_le_bytes());
        }
    }
    b.extend((g.below(3) as i32).to_le_bytes()); // goal object
    for _ in 0..3 {
        b.extend(g.uniform(-9.0, 9.0).to_le_bytes());
    }
    b.push(g.below(2) as u8); // locked
    b.push(g.below(2) as u8); // inited
    if g.below(12) == 0 {
        b.truncate(cut_list[g.below(cut_list.len() as u32) as usize]);
    }
    b
}

fn gen_case(g: &mut Gen) -> Case {
    let n_conds = 1 + g.below(4);
    let conds: Vec<Vec<u32>> = (0..n_conds)
        .map(|_| (0..1 + g.below(5)).map(|_| g.below(2)).collect())
        .collect();
    let n_obj = g.below(4);
    let objects: Vec<(i32, [u32; 3])> = (0..n_obj)
        .map(|i| (5 + i as i32, gen_pos_bits(g)))
        .collect();

    let n_states = match g.below(100) {
        0..=3 => 0,
        n => 1 + (n - 4) % 7,
    };
    // distinct ids, in a random order (the first defined is the default)
    let mut pool = ID_POOL.to_vec();
    for i in (1..pool.len()).rev() {
        let j = g.below(i as u32 + 1) as usize;
        pool.swap(i, j);
    }
    let mut ids: Vec<u32> = pool[..n_states as usize].to_vec();
    let dup = n_states >= 2 && g.below(50) == 0;
    if dup {
        ids[1] = ids[0];
    }
    let mut specs = Vec::new();
    for &id in &ids {
        let kind = match g.below(100) {
            0..=9 => 0,
            10..=19 => 1,
            20..=26 => 2,
            27..=33 => 3,
            _ => 4,
        };
        let trans: Vec<(u32, u32)> = if g.below(100) < 35 {
            (0..1 + g.below(3))
                .map(|_| (g.below(n_conds), pick_target(g, &ids)))
                .collect()
        } else {
            Vec::new()
        };
        let mut s = StateSpec {
            kind,
            id,
            succ: pick_target(g, &ids),
            fail: pick_target(g, &ids),
            trans,
            flags: 0,
            enter: Vec::new(),
            update: Vec::new(),
            exit: Vec::new(),
        };
        if kind == 4 {
            s.flags = g.below(16);
            s.enter = gen_acts(g, &ids, n_obj, 3);
            s.update = gen_acts(g, &ids, n_obj, 3);
            s.exit = gen_acts(g, &ids, n_obj, 2);
        }
        specs.push(s);
    }

    let by_id: BTreeMap<u32, u32> = specs.iter().map(|s| (s.id, s.kind)).collect();
    let mut ops = Vec::new();
    let mut frame = 0u32;
    if g.below(10) != 0 {
        ops.push(Op::Init);
    }
    for _ in 0..4 + g.below(36) {
        let op = match g.below(100) {
            0..=33 => Op::Update,
            34..=40 => Op::SetState(pick_target(g, &ids)),
            41..=43 => Op::Reset,
            44..=46 => Op::Clear,
            47..=48 => Op::Lock,
            49..=51 => Op::Unlock,
            52 => Op::Halt,
            53..=55 => Op::GoalObject(g.below(n_obj + 2)),
            56..=57 => Op::GoalPosition(g.below(4) != 0, gen_pos_bits(g)),
            58..=69 => {
                frame = match g.below(60) {
                    0 => 0xFFFF_FFF0 - g.below(10),
                    1 => frame.wrapping_add(1000),
                    _ => frame.wrapping_add(g.below(25)),
                };
                Op::Frame(frame)
            }
            70..=71 => Op::Kill(g.below(n_obj + 1)),
            72 => Op::Revive(g.below(n_obj + 1)),
            73..=77 => Op::Save,
            78..=80 => Op::Crc,
            81..=91 => {
                // mutations that keep the layout and every bool byte 0 or 1:
                // the sleep frame, the version bytes, or a cut at a field boundary
                match g.below(6) {
                    0 | 1 => Op::LoadLast(0, 0, 0),
                    2 => Op::LoadLast(1, CUTS[g.below(CUTS.len() as u32) as usize], 0),
                    3 => Op::LoadLast(2, 0, g.below(4)),
                    4 => Op::LoadLast(2, 14, g.below(4)),
                    _ => Op::LoadLast(3, 1, g.below(60)),
                }
            }
            92..=95 if !dup => Op::LoadBytes(craft_stream(g, &by_id, &ids)),
            _ => Op::Internal(pick_target(g, &ids)),
        };
        ops.push(op);
    }
    Case {
        conds,
        objects,
        states: specs,
        ops,
    }
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

/// The first position where two logs differ: `(index, rust, cpp)` with `None`
/// for a missing word.
fn first_diff(r: &[u32], c: &[u32]) -> Option<(usize, Option<u32>, Option<u32>)> {
    (0..r.len().max(c.len()))
        .find(|&i| r.get(i) != c.get(i))
        .map(|i| (i, r.get(i).copied(), c.get(i).copied()))
}

/// Tallies of what the cases exercised.
#[derive(Default, Debug)]
struct Coverage {
    cases: u64,
    compared_cases: u64,
    undefined_cases: u64,
    undefined_inputs: u64,
    ops: u64,
    callbacks: u64,
    conditions: u64,
    actions: u64,
    /// Cases in which one operation entered 20 or more states (the ring that
    /// meets the `checkfortransitionsnum >= 20` limit).
    long_chains: u64,
    thrown_error_code: u64,
    thrown_save_code: u64,
    thrown_xfer: BTreeMap<u32, u64>,
    saves: u64,
    crc_ops: u64,
    loads: u64,
    /// Operations after which the machine was asleep.
    sleeping_ops: u64,
}

fn tally(cov: &mut Coverage, case: &Case, r: &RustRun) {
    cov.ops += case.ops.len() as u64;
    let ev = &r.logs.ev;
    let mut i = 0;
    while i < ev.len() {
        match ev[i] {
            T_ENTER_BEGIN | T_UPDATE_BEGIN | T_EXIT_BEGIN => cov.callbacks += 1,
            T_COND => cov.conditions += 1,
            T_ACTION => cov.actions += 1,
            _ => {}
        }
        i += event_width(ev[i]);
    }
    if r.max_enters_per_op >= 20 {
        cov.long_chains += 1;
    }
    for op in &case.ops {
        match op {
            Op::Save => cov.saves += 1,
            Op::Crc => cov.crc_ops += 1,
            Op::LoadLast(..) | Op::LoadBytes(_) => cov.loads += 1,
            _ => {}
        }
    }
    for st in &r.statuses {
        match st[0] {
            1 => cov.thrown_error_code += 1,
            3 => cov.thrown_save_code += 1,
            2 => *cov.thrown_xfer.entry(st[1]).or_insert(0) += 1,
            _ => {}
        }
    }
    cov.sleeping_ops += u64::from(r.sleeping_ops);
}

const CASES: usize = 6000;

#[test]
fn state_machine_matches_upstream() {
    let mut g = Gen::new(0x5A7E_0001);
    let mut ev = Check::new("StateMachine events (every transition)");
    let mut res = Check::new("StateMachine results (status, return, view)");
    let mut byt = Check::new("StateMachine xfer bytes (save, deep CRC)");
    let mut crc = Check::new("StateMachine CRC values");
    let mut ev_bug = Check::new("planted: one callback event altered");
    let mut res_bug = Check::new("planted: one return code altered");
    let mut byt_bug = Check::new("planted: one saved byte altered");
    let mut crc_bug = Check::new("planted: one CRC bit altered");
    let mut cov = Coverage::default();

    for i in 0..CASES {
        let case = gen_case(&mut g);
        cov.cases += 1;
        let r = run_rust(&case);
        if case.has_duplicate_ids() {
            // undefined upstream: the Rust refuses it (counted), the C++ is not run
            assert!(
                r.undefined >= 1,
                "case {i}: a duplicate state id must be refused"
            );
            cov.undefined_cases += 1;
            cov.undefined_inputs += u64::from(r.undefined);
            continue;
        }
        assert_eq!(r.undefined, 0, "case {i}: nothing else is undefined");
        let c = run_cpp(&case);
        cov.compared_cases += 1;
        tally(&mut cov, &case, &r);

        let what = || format!("case {i}: {case:#?}");
        ev.eq(first_diff(&r.logs.ev, &c.ev), None, what);
        res.eq(first_diff(&r.logs.res, &c.res), None, what);
        byt.eq(first_diff(&r.logs.byt, &c.byt), None, what);
        crc.eq(first_diff(&r.logs.crc, &c.crc), None, what);

        // planted: the Rust log altered by one bit of one word must differ
        let planted = |v: &[u32], salt: u32| -> Vec<u32> {
            let mut p = v.to_vec();
            if p.is_empty() {
                p.push(1);
            } else {
                let k = (salt as usize * 7 + i) % p.len();
                p[k] ^= 1 << (i % 5);
            }
            p
        };
        ev_bug.eq(first_diff(&planted(&r.logs.ev, 1), &c.ev), None, || {
            format!("case {i}")
        });
        res_bug.eq(first_diff(&planted(&r.logs.res, 2), &c.res), None, || {
            format!("case {i}")
        });
        byt_bug.eq(first_diff(&planted(&r.logs.byt, 3), &c.byt), None, || {
            format!("case {i}")
        });
        crc_bug.eq(first_diff(&planted(&r.logs.crc, 4), &c.crc), None, || {
            format!("case {i}")
        });
    }

    println!("COVERAGE {cov:#?}");
    // the generator must reach the paths the comparison is about
    assert!(cov.compared_cases > 5000);
    assert!(cov.callbacks > 50_000, "callbacks: {}", cov.callbacks);
    assert!(cov.conditions > 5_000 && cov.actions > 5_000);
    assert!(
        cov.long_chains > 20,
        "no case reached the transition depth limit"
    );
    assert!(cov.thrown_error_code > 0, "ERROR_BAD_ARG never thrown");
    assert!(cov.thrown_save_code > 0, "SC_INVALID_DATA never thrown");
    assert!(
        cov.thrown_xfer
            .contains_key(&(XferStatus::InvalidVersion as u32)),
        "no invalid version"
    );
    assert!(
        cov.thrown_xfer
            .contains_key(&(XferStatus::ReadError as u32)),
        "no truncated load"
    );
    assert!(cov.sleeping_ops > 5000);
    assert!(cov.saves > 2000 && cov.crc_ops > 1000 && cov.loads > 3000);
    assert!(cov.undefined_cases > 50 && cov.undefined_cases == cov.undefined_inputs);

    ev.finish();
    res.finish();
    byt.finish();
    crc.finish();
    ev_bug.finish_planted();
    res_bug.finish_planted();
    byt_bug.finish_planted();
    crc_bug.finish_planted();
}

// ---------------------------------------------------------------------------
// Fixed scenarios (each also run on the C++)
// ---------------------------------------------------------------------------

fn scripted(
    id: u32,
    succ: u32,
    fail: u32,
    enter: Vec<Act>,
    update: Vec<Act>,
    exit: Vec<Act>,
) -> StateSpec {
    StateSpec {
        kind: 4,
        id,
        succ,
        fail,
        trans: Vec::new(),
        flags: 0,
        enter,
        update,
        exit,
    }
}

fn util(kind: u32, id: u32, succ: u32, fail: u32) -> StateSpec {
    StateSpec {
        kind,
        id,
        succ,
        fail,
        trans: Vec::new(),
        flags: 0,
        enter: Vec::new(),
        update: Vec::new(),
        exit: Vec::new(),
    }
}

fn same(case: &Case, name: &str) -> Logs {
    let r = run_rust(case);
    let c = run_cpp(case);
    assert_eq!(r.logs, c, "{name}");
    c
}

fn act(ret: i32, act: u32, arg: u32) -> Act {
    Act { ret, act, arg }
}

#[test]
fn fixed_scenarios_match_upstream() {
    // a ring of success states: the 20-deep limit ends it with STATE_FAILURE
    let ring = Case {
        conds: vec![vec![1]],
        objects: vec![],
        states: vec![util(0, 1, 2, 2), util(0, 2, 3, 3), util(0, 3, 1, 1)],
        ops: vec![Op::Init, Op::Update, Op::Update],
    };
    let l = same(&ring, "ring");
    // 20 deep: the guard returns FAILURE (-2) from the innermost check
    assert!(l
        .res
        .windows(3)
        .any(|w| w[0] == 0 && w[1] == 0 && w[2] == (-2i32) as u32));

    // sleeping: enter sleeps 10 frames from frame 5, wake exactly at 15, and the sleep wraps at 2^32
    let sleeper = Case {
        conds: vec![vec![0]],
        objects: vec![],
        states: vec![scripted(
            1,
            1,
            1,
            vec![act(10, 0, 0)],
            vec![act(3, 0, 0), act(-1, 0, 0)],
            vec![],
        )],
        ops: vec![
            Op::Frame(5),
            Op::Init,
            Op::Frame(14),
            Op::Update,
            Op::Frame(15),
            Op::Update,
            Op::Frame(0xFFFF_FFFE),
            Op::Update,
            Op::Frame(0xFFFF_FFFF),
            Op::Update,
            Op::Frame(2),
            Op::Update,
        ],
    };
    same(&sleeper, "sleeper");

    // a state that changes state from inside update ignores its own sleep
    let jumper = Case {
        conds: vec![vec![0]],
        objects: vec![],
        states: vec![
            scripted(1, 2, 2, vec![], vec![act(50, 1, 2)], vec![]),
            scripted(
                2,
                1,
                1,
                vec![act(0, 0, 0)],
                vec![act(0, 0, 0)],
                vec![act(0, 0, 0)],
            ),
        ],
        ops: vec![Op::Init, Op::Update, Op::Update, Op::Save, Op::Crc],
    };
    let l = same(&jumper, "jumper");
    assert!(!l.byt.is_empty());

    // a state that clears the machine / halts it from update ("the state was destroyed")
    for action in [3, 5, 4, 6] {
        let c = Case {
            conds: vec![vec![0]],
            objects: vec![],
            states: vec![
                scripted(1, 1, 1, vec![], vec![act(-1, action, 0)], vec![]),
                util(2, 2, 2, 2),
            ],
            ops: vec![
                Op::Init,
                Op::Update,
                Op::Update,
                Op::Save,
                Op::Reset,
                Op::Update,
            ],
        };
        same(&c, "destroyed from update");
    }

    // unknown ids recover to the default; no default: ERROR_BAD_ARG
    let unknown = Case {
        conds: vec![vec![0]],
        objects: vec![],
        states: vec![util(2, 1, 1, 1), util(2, 2, 2, 2)],
        ops: vec![
            Op::Init,
            Op::SetState(2),
            Op::SetState(77),
            Op::Internal(2),
            Op::Internal(INVALID_STATE_ID),
            Op::Internal(MACHINE_DONE_STATE_ID),
            Op::Update,
        ],
    };
    same(&unknown, "unknown ids");
    let none = Case {
        conds: vec![],
        objects: vec![],
        states: vec![],
        ops: vec![
            Op::Init,
            Op::SetState(3),
            Op::Update,
            Op::Save,
            Op::Crc,
            Op::Reset,
        ],
    };
    let l = same(&none, "no states");
    assert!(l.res.windows(2).any(|w| w == [1, ERROR_BAD_ARG]));

    // goals: object, destroyed object, position, null; lock blocks them
    let goals = Case {
        conds: vec![vec![0]],
        objects: vec![
            (5, [1.5f32.to_bits(), 2.5f32.to_bits(), 3.5f32.to_bits()]),
            (6, [0; 3]),
        ],
        states: vec![util(2, 1, 1, 1)],
        ops: vec![
            Op::Init,
            Op::GoalObject(0),
            Op::Kill(0),
            Op::GoalPosition(true, [9f32.to_bits(), 8f32.to_bits(), 7f32.to_bits()]),
            Op::GoalObject(5),
            Op::GoalObject(1),
            Op::Revive(0),
            Op::Lock,
            Op::GoalObject(0),
            Op::GoalPosition(false, [0; 3]),
            Op::Unlock,
            Op::GoalObject(0),
            Op::Save,
            Op::Clear,
        ],
    };
    same(&goals, "goals");

    // the all-states load with a right stream: both states loaded, two post-process hand-offs
    let two = vec![
        scripted(1, 2, 2, vec![act(0, 0, 0)], vec![], vec![]),
        util(2, 2, 2, 2),
    ];
    let mut b = vec![1u8];
    b.extend(0u32.to_le_bytes());
    b.extend(1u32.to_le_bytes());
    b.extend(1u32.to_le_bytes());
    b.push(1);
    b.extend(2i32.to_le_bytes());
    b.extend(1u32.to_le_bytes());
    b.push(1);
    b.extend(77u32.to_le_bytes());
    b.extend(2u32.to_le_bytes());
    b.push(1);
    b.extend(0i32.to_le_bytes());
    b.extend([0u8; 12]);
    b.extend([1, 1]);
    let all = Case {
        conds: vec![vec![0]],
        objects: vec![],
        states: two,
        ops: vec![Op::LoadBytes(b), Op::Save],
    };
    let l = same(&all, "all-states load");
    // the counter of state 1 became 77 and two snapshots were handed to post-processing
    assert!(l.res.windows(2).any(|w| w == [2, 77]) || l.res.contains(&77));
}
