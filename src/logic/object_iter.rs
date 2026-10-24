// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ObjectIter.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SimpleObjectIterator.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SimpleObjectIterator`: a list of objects, each with a numeric key
//! (typically the squared distance), that can be sorted by that key or by
//! the objects' build cost.
//!
//! # Cut points
//!
//! - **The object handle** `O` (`Object *`). Upstream's `Object` is layer 8.
//!   The iterator stores the handle and never looks inside it.
//! - **The build cost** of an object (`obj->getTemplate()->friend_getBuildCost()`,
//!   used by the two cost orders). [`SimpleObjectIterator::sort`] takes it as
//!   a closure, so the template lookup stays with the owner.
//! - **The abstract `ObjectIterator`** (`first`/`next` as virtuals, with
//!   `PartitionManager` the only producer). That producer is layer 8, so
//!   until it exists this module has no second iterator to dispatch to, and
//!   the enum that will hold the variants is not written yet.
//!
//! # Translation choices
//!
//! - **The list is an arena.** Upstream links `Clump` nodes through pointers
//!   and sorts them with a bottom-up merge sort over those links. Here the
//!   clumps live in a `Vec` and link by index, and the sort is the same
//!   algorithm step for step. A stable library sort would give the same
//!   order on ties, but it may panic on the NaN keys that comparisons can
//!   produce, and it would not reproduce upstream's choice for `NaN`
//!   (`cmp <= 0` is false, so the second run is taken).
//! - **Costs are compared as integers.** `friend_getBuildCost()` is an `Int`,
//!   so upstream's `a - b` is an integer subtraction (it wraps, with
//!   `-fwrapv`) and is converted to `Real` only afterwards. Here it is
//!   `wrapping_sub`, then the conversion.
//!
//! # Where upstream is undefined
//!
//! `insert(NULL, ...)` is rejected with [`ObjectIterError::NullObject`].
//! Upstream stores a null clump, which ends an iteration early (`next()`
//! returns null, its end marker), and dereferences it in the cost sorts.

/// `enum IterOrderType`: the order `sort` puts the iterator in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IterOrderType {
    /// `ITER_FASTEST`: arbitrary order; `sort` does nothing.
    Fastest = 0,
    /// `ITER_SORTED_NEAR_TO_FAR`: ascending numeric key.
    SortedNearToFar = 1,
    /// `ITER_SORTED_FAR_TO_NEAR`: descending numeric key.
    SortedFarToNear = 2,
    /// `ITER_SORTED_CHEAP_TO_EXPENSIVE`: ascending build cost.
    SortedCheapToExpensive = 3,
    /// `ITER_SORTED_EXPENSIVE_TO_CHEAP`: descending build cost.
    SortedExpensiveToCheap = 4,
}

/// The input upstream does not define: a null object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectIterError {
    /// `insert(NULL, ...)`: "sorry, no nulls allowed here".
    NullObject,
}

/// One element of the list (upstream's `SimpleObjectIterator::Clump`).
#[derive(Clone, Copy, Debug)]
struct Clump<O> {
    next: Option<usize>,
    obj: O,
    numeric: f32,
}

/// `class SimpleObjectIterator`.
#[derive(Clone, Debug)]
pub struct SimpleObjectIterator<O> {
    /// Every clump ever inserted since the last `make_empty`, by slot.
    clumps: Vec<Clump<O>>,
    /// `m_firstClump`.
    first_clump: Option<usize>,
    /// `m_curClump`: the next one `next` returns.
    cur_clump: Option<usize>,
}

impl<O: Copy> Default for SimpleObjectIterator<O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Copy> SimpleObjectIterator<O> {
    /// `SimpleObjectIterator()`: empty.
    pub fn new() -> Self {
        SimpleObjectIterator {
            clumps: Vec::new(),
            first_clump: None,
            cur_clump: None,
        }
    }

    /// `getCount()`.
    pub fn count(&self) -> usize {
        self.clumps.len()
    }

    /// `insert(obj, numeric)`: added at the head of the list, as upstream does.
    pub fn insert(&mut self, obj: Option<O>, numeric: f32) -> Result<(), ObjectIterError> {
        let obj = obj.ok_or(ObjectIterError::NullObject)?;
        self.clumps.push(Clump {
            next: self.first_clump,
            obj,
            numeric,
        });
        self.first_clump = Some(self.clumps.len() - 1);
        Ok(())
    }

    /// `makeEmpty()`: throws away every element.
    pub fn make_empty(&mut self) {
        self.clumps.clear();
        self.first_clump = None;
        self.cur_clump = None;
    }

    /// `reset()`: the next `next` returns the first element again.
    fn reset(&mut self) {
        self.cur_clump = self.first_clump;
    }

    /// `first()`: reset, then the first element (`None` when empty).
    pub fn first(&mut self) -> Option<O> {
        self.first_with_numeric().0
    }

    /// `next()`: the element after the last one returned. Not `Iterator::next`:
    /// upstream's walk also keeps a numeric key, and `first` resets it.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<O> {
        self.next_with_numeric().0
    }

    /// `firstWithNumeric(num)`: reset, then as [`first`](Self::first), also
    /// returning the numeric key (0 when there is no element).
    pub fn first_with_numeric(&mut self) -> (Option<O>, f32) {
        self.reset();
        self.next_with_numeric()
    }

    /// `nextWithNumeric(num)`: as [`next`](Self::next), also returning the
    /// numeric key of the element (0 when there is none).
    pub fn next_with_numeric(&mut self) -> (Option<O>, f32) {
        match self.cur_clump {
            Some(i) => {
                let c = self.clumps[i];
                self.cur_clump = c.next;
                (Some(c.obj), c.numeric)
            }
            None => (None, 0.0),
        }
    }

    /// The comparison of upstream's `theClumpCompareProcs`, for the two
    /// numeric orders and the two cost orders. `Fastest` has none.
    fn compare(
        &self,
        order: IterOrderType,
        a: usize,
        b: usize,
        build_cost: &impl Fn(&O) -> i32,
    ) -> Option<f32> {
        let (ca, cb) = (&self.clumps[a], &self.clumps[b]);
        Some(match order {
            IterOrderType::Fastest => return None,
            IterOrderType::SortedNearToFar => ca.numeric - cb.numeric,
            IterOrderType::SortedFarToNear => cb.numeric - ca.numeric,
            // `Int` subtraction, then the conversion to `Real`.
            IterOrderType::SortedCheapToExpensive => {
                build_cost(&ca.obj).wrapping_sub(build_cost(&cb.obj)) as f32
            }
            IterOrderType::SortedExpensiveToCheap => {
                build_cost(&cb.obj).wrapping_sub(build_cost(&ca.obj)) as f32
            }
        })
    }

    /// `sort(order)`: upstream's bottom-up merge sort over the list, with
    /// its tie rule (the earlier run wins when `cmp <= 0`). `build_cost` is
    /// asked only by the two cost orders. Afterwards the iterator is reset.
    pub fn sort(&mut self, order: IterOrderType, build_cost: impl Fn(&O) -> i32) {
        if self.clumps.is_empty() {
            return;
        }
        if order == IterOrderType::Fastest {
            return; // my, that was easy
        }

        // do a basic mergesort, which works nicely for linked lists,
        // and is reasonably efficient (N log N).
        let mut n: i32 = 1;
        loop {
            let mut to_do = self.first_clump;
            let mut tail: Option<usize> = None;
            self.first_clump = None;

            let mut merge_count = 0;

            while to_do.is_some() {
                merge_count += 1;

                let mut to_do_count: i32 = 0;

                // make two lists of length 'n' (to_do is one, sub is the other)
                let mut sub = to_do;
                for _ in 0..n {
                    to_do_count += 1;
                    sub = self.clumps[sub.expect("sub is set while counting")].next;
                    if sub.is_none() {
                        break;
                    }
                }
                let mut sub_count: i32 = if sub.is_some() { n } else { 0 };

                // merge the two lists.
                while to_do_count + sub_count > 0 {
                    // Each branch takes a present element: the counts say
                    // which list is non-empty (upstream's invariant).
                    let tmp = if sub_count == 0 {
                        let t = to_do.expect("to_do_count > 0");
                        to_do = self.clumps[t].next;
                        to_do_count -= 1;
                        t
                    } else if to_do_count == 0 {
                        let s = sub.expect("sub_count > 0");
                        sub = self.clumps[s].next;
                        sub_count -= 1;
                        s
                    } else {
                        let t = to_do.expect("to_do_count > 0");
                        let s = sub.expect("sub_count > 0");
                        // `cmp <= 0` is false for NaN, so the second run is taken, as upstream.
                        if self
                            .compare(order, t, s, &build_cost)
                            .expect("order has a comparison")
                            <= 0.0
                        {
                            to_do = self.clumps[t].next;
                            to_do_count -= 1;
                            t
                        } else {
                            sub = self.clumps[s].next;
                            sub_count -= 1;
                            s
                        }
                    };
                    if sub.is_none() {
                        sub_count = 0;
                    }
                    if to_do.is_none() {
                        to_do_count = 0;
                    }

                    match tail {
                        Some(t) => self.clumps[t].next = Some(tmp),
                        None => self.first_clump = Some(tmp),
                    }
                    tail = Some(tmp);
                }

                to_do = sub;
            }
            if let Some(t) = tail {
                self.clumps[t].next = None;
            }

            // when we have done just one (or none) swap, we're done
            if merge_count <= 1 {
                break;
            }
            n *= 2;
        }

        // always reset after sorting, to prevent weirdness
        self.reset();
    }
}
