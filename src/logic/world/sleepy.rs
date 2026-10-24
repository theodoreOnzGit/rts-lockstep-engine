// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   (`m_sleepyUpdates` and its maintenance: lines 2701-3033, and the module
//   loop of `GameLogic::update`, lines 3697-3745)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Sleepy updates: Steven Johnson
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The sleepy-update scheduler: the binary heap of update modules that decides
//! **which module updates in what order each frame**.
//!
//! # Why this is in the world core
//!
//! Update order is simulation state. `GameLogic::update` pops modules off a
//! min-heap ordered by `m_nextCallFrameAndPhase` and calls them until the top
//! is due later than now. **Modules with an equal key are called in whatever
//! order the heap's swaps left them**, which depends on the exact sequence of
//! pushes, erases and rebalances since the game began. A "cleaner" scheduler
//! (a `BinaryHeap`, a sort, a bucket queue) would order ties differently and
//! diverge from upstream, so the maintenance functions are translated step for
//! step, including the index each module keeps of its own position
//! (`m_indexInLogic`, which here lives in [`UpdateModule`]).
//!
//! # Shape
//!
//! The heap is a `Vec<UpdateHandle>`. A handle names a module by
//! (object id, index in the object's behaviour list), which is what upstream's
//! `UpdateModulePtr` is made of. The key and the heap index are read and
//! written through the [`ObjectArena`] (they are fields of the module's
//! [`UpdateModule`] half, which stays in the arena even while the module's
//! behaviour half is checked out for its own `update`, see
//! `docs/world-design.md`). Every maintenance function therefore takes the
//! arena as an argument next to `&mut self`: two disjoint fields of the world.
//!
//! # Panics
//!
//! A handle in the heap whose object or module is not in the arena is a
//! corrupted scheduler (upstream dereferences a dangling pointer). It cannot
//! happen through this API: [`SleepyScheduler::erase_object_updates`] removes an
//! object's handles before the object leaves the arena. The accessors panic
//! with that message rather than carry a `Result` through every heap swap.
//!
//! # Not translated
//!
//! `ALLOW_NONSLEEPY_UPDATES` (`m_normalUpdates`): `GameLogic.h` defines
//! `NO_ALLOW_NONSLEEPY_UPDATES`, so the shipped game has only the heap.
//! `validateSleepyUpdate` is empty unless `_DEBUG`. `USE_PERF_TIMER`s.

use crate::logic::module::update_module::UpdateModule;
use crate::logic::world::ids::ObjectId;
use crate::logic::world::object::ObjectArena;

/// An `UpdateModulePtr`: object id and index in that object's behaviours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UpdateHandle {
    /// The owner (`friend_getObject()->getID()`).
    pub object: ObjectId,
    /// Position in `Object::behaviors`.
    pub index: u16,
}

/// `eraseSleepyUpdate`'s cap in `processDestroyList`: `MAX_SUO`.
pub const MAX_SUO: usize = 256;

/// One module update the loop ran, recorded when tracing is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateTraceEntry {
    /// `getFrame()` when it ran.
    pub frame: u32,
    /// Which module.
    pub handle: UpdateHandle,
    /// Its key before the call (`friend_getPriority()`).
    pub priority: u32,
}

/// `m_sleepyUpdates` and `m_curUpdateModule`.
#[derive(Clone, Debug, Default)]
pub struct SleepyScheduler {
    /// `m_sleepyUpdates`.
    heap: Vec<UpdateHandle>,
    /// `m_curUpdateModule`: the module whose `update()` is running.
    cur: Option<UpdateHandle>,
    /// When `Some`, every update the loop runs is appended (a debugging aid
    /// with no upstream counterpart; it does not change behaviour).
    pub trace: Option<Vec<UpdateTraceEntry>>,
}

fn module(objs: &ObjectArena, h: UpdateHandle) -> &UpdateModule {
    objs.update_module(h).expect("sleepy heap entry names an update module in the arena")
}

fn module_mut(objs: &mut ObjectArena, h: UpdateHandle) -> &mut UpdateModule {
    objs.update_module_mut(h).expect("sleepy heap entry names an update module in the arena")
}

impl SleepyScheduler {
    /// An empty scheduler.
    pub fn new() -> SleepyScheduler {
        SleepyScheduler::default()
    }

    /// `getNumberSleepyUpdates()`.
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// No sleepy updates.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// The heap array in index order (for tests and diagnostics).
    pub fn heap(&self) -> &[UpdateHandle] {
        &self.heap
    }

    /// `m_curUpdateModule`.
    pub fn current(&self) -> Option<UpdateHandle> {
        self.cur
    }

    /// `m_curUpdateModule = u` (`None` for `NULL`).
    pub fn set_current(&mut self, u: Option<UpdateHandle>) {
        self.cur = u;
    }

    /// `isLowerPriority(a, b)`: `a`'s key is larger ("lower ordinal value
    /// means higher priority").
    fn is_lower_priority(objs: &ObjectArena, a: UpdateHandle, b: UpdateHandle) -> bool {
        module(objs, a).friend_get_priority() > module(objs, b).friend_get_priority()
    }

    /// `peekSleepyUpdate()`: the top (smallest key), `None` when empty.
    pub fn peek(&self) -> Option<UpdateHandle> {
        self.heap.first().copied()
    }

    /// `pushSleepyUpdate(u)`.
    pub fn push(&mut self, objs: &mut ObjectArena, u: UpdateHandle) {
        self.heap.push(u);
        let last = self.heap.len() - 1;
        module_mut(objs, u).friend_set_index_in_logic(last as i32);
        self.rebalance_parent(objs, last as i32);
    }

    /// `loadPostProcess`'s `m_sleepyUpdates.push_back(u);
    /// u->friend_setIndexInLogic(size - 1)`: append without rebalancing (the
    /// caller then calls [`SleepyScheduler::remake`]).
    pub fn push_unsorted(&mut self, objs: &mut ObjectArena, u: UpdateHandle) {
        self.heap.push(u);
        let last = self.heap.len() - 1;
        module_mut(objs, u).friend_set_index_in_logic(last as i32);
    }

    /// `popSleepyUpdate()`. Does nothing when empty (`DEBUG_CRASH` upstream).
    pub fn pop(&mut self, objs: &mut ObjectArena) {
        let sz = self.heap.len();
        if sz == 0 {
            return;
        }
        module_mut(objs, self.heap[0]).friend_set_index_in_logic(-1);
        if sz > 1 {
            self.heap[0] = self.heap[sz - 1];
            module_mut(objs, self.heap[0]).friend_set_index_in_logic(0);
            self.heap.pop();
            self.rebalance_child(objs, 0);
        } else {
            self.heap.pop();
        }
    }

    /// `eraseSleepyUpdate(i)`: swap with the final item, drop the final item,
    /// rebalance. An index outside the heap does nothing (undefined upstream).
    pub fn erase(&mut self, objs: &mut ObjectArena, i: i32) {
        if i < 0 || i as usize >= self.heap.len() {
            return;
        }
        let i = i as usize;
        module_mut(objs, self.heap[i]).friend_set_index_in_logic(-1);
        let fin = self.heap.len() - 1;
        if i < fin {
            self.heap[i] = self.heap[fin];
            module_mut(objs, self.heap[i]).friend_set_index_in_logic(i as i32);
            self.heap.pop();
            self.rebalance(objs, i as i32);
        } else {
            self.heap.pop();
        }
    }

    /// `rebalanceParentSleepyUpdate(i)`: sift toward the root; returns the
    /// final index.
    pub fn rebalance_parent(&mut self, objs: &mut ObjectArena, mut i: i32) -> i32 {
        if i < 0 || i as usize >= self.heap.len() {
            return i;
        }
        let mut parent = ((i + 1) >> 1) - 1;
        while parent >= 0 && Self::is_lower_priority(objs, self.heap[parent as usize], self.heap[i as usize]) {
            let a = self.heap[parent as usize];
            let b = self.heap[i as usize];
            self.heap[i as usize] = a;
            self.heap[parent as usize] = b;
            module_mut(objs, a).friend_set_index_in_logic(i);
            module_mut(objs, b).friend_set_index_in_logic(parent);
            i = parent;
            parent = ((parent + 1) >> 1) - 1;
        }
        i
    }

    /// `rebalanceChildSleepyUpdate(i)`: sift toward the leaves; returns the
    /// final index. (Upstream's hand-unrolled pointer loop and its "pristine
    /// non-unrolled version" are the same algorithm; this is the latter.)
    pub fn rebalance_child(&mut self, objs: &mut ObjectArena, mut i: i32) -> i32 {
        let sz = self.heap.len() as i32;
        let mut child = ((i + 1) << 1) - 1;
        while child >= 0 && child < sz {
            // choose the higher-priority of the two children
            if child < sz - 1 && Self::is_lower_priority(objs, self.heap[child as usize], self.heap[child as usize + 1]) {
                child += 1;
            }
            // if we're higher-pri than our children, we're done
            if !Self::is_lower_priority(objs, self.heap[i as usize], self.heap[child as usize]) {
                break;
            }
            let a = self.heap[child as usize];
            let b = self.heap[i as usize];
            self.heap[i as usize] = a;
            self.heap[child as usize] = b;
            module_mut(objs, a).friend_set_index_in_logic(i);
            module_mut(objs, b).friend_set_index_in_logic(child);
            i = child;
            child = ((i + 1) << 1) - 1;
        }
        i
    }

    /// `rebalanceSleepyUpdate(i)`: parent pass, then child pass from where it
    /// ended.
    pub fn rebalance(&mut self, objs: &mut ObjectArena, i: i32) {
        let i = self.rebalance_parent(objs, i);
        self.rebalance_child(objs, i);
    }

    /// `remakeSleepyUpdate()`: heapify the whole array (after a load).
    pub fn remake(&mut self, objs: &mut ObjectArena) {
        let mut parent = (self.heap.len() / 2) as i32;
        loop {
            self.rebalance_child(objs, parent);
            if parent == 0 {
                break;
            }
            parent -= 1;
        }
    }

    /// The first half of `processDestroyList`'s per-object work: erase every
    /// sleepy update of `object`. Upstream scans the heap in index order,
    /// collects up to [`MAX_SUO`] modules of the object, then erases them
    /// **last collected first**, re-reading each module's index "since each
    /// call to erase might change others". The erase order decides the heap
    /// layout, which decides tie order, so it is reproduced exactly.
    pub fn erase_object_updates(&mut self, objs: &mut ObjectArena, object: ObjectId) {
        let mut found: Vec<UpdateHandle> = Vec::new();
        for &u in &self.heap {
            if u.object == object && found.len() < MAX_SUO {
                found.push(u);
            }
        }
        for u in found.into_iter().rev() {
            let idx = module(objs, u).friend_get_index_in_logic();
            self.erase(objs, idx);
        }
    }

    // --- p1w6-object ---
    /// `Object::setID` on a load: the handles of that object's modules name it
    /// by its new id (upstream's heap holds pointers).
    pub fn rename_object(&mut self, old: ObjectId, new: ObjectId) {
        for u in &mut self.heap {
            if u.object == old {
                u.object = new;
            }
        }
        if let Some(c) = &mut self.cur {
            if c.object == old {
                c.object = new;
            }
        }
    }
    // --- end p1w6-object ---

    /// `setDefaults` / `loadPostProcess`: "blow away the sleepy update list",
    /// setting every module's index back to -1 first.
    pub fn clear(&mut self, objs: &mut ObjectArena) {
        for &u in &self.heap {
            if let Some(m) = objs.update_module_mut(u) {
                m.friend_set_index_in_logic(-1);
            }
        }
        self.heap.clear();
        self.cur = None;
    }
}
