// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/SparseMatchFinder.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, March 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `template<class MATCHABLE, class BITSET> class SparseMatchFinder`: picks,
//! from a list of entries each carrying one or more "yes" condition masks,
//! the entry that best matches a set of condition bits. Logic uses it for
//! armor sets and weapon sets (the client for model conditions).
//!
//! The best match has the most bits in common with the query; among those,
//! the fewest bits of its mask that the query lacks. On a tie the **first**
//! candidate found wins, scanning entries forward and each entry's masks
//! **backwards** (from the last to the first), as upstream does. Every
//! candidate beats the starting score (0 matches, 999 extraneous bits), so
//! the result is `None` only if no entry has a mask.
//!
//! Upstream caches results in a `std::map` keyed by the query bits (its
//! comparator orders masks lexicographically from bit 0, a strict weak
//! order on the bits, so the map is an exact-bits lookup and is never
//! iterated). The cache stores a pointer into the entry vector and is not
//! keyed by the vector: calling `findBestInfo` with a different or changed
//! vector without `clear()` returns a stale (possibly dangling) entry,
//! undefined upstream. Here the cache stores the index and the caller keeps
//! upstream's contract: `clear()` whenever the entries change.

use std::collections::BTreeMap;

use super::bit_flags::{BitFlagNames, BitFlags};

/// What `MATCHABLE` provides: `getConditionsYesCount()` and
/// `getNthConditionsYes(i)`.
pub trait SparseMatchable<T: BitFlagNames> {
    /// `getConditionsYesCount()`.
    fn conditions_yes_count(&self) -> i32;
    /// `getNthConditionsYes(i)`, `0 <= i < conditions_yes_count()`.
    fn nth_conditions_yes(&self, i: i32) -> &BitFlags<T>;
}

/// `SparseMatchFinder<MATCHABLE, BITSET>`.
#[derive(Clone, Debug, Default)]
pub struct SparseMatchFinder {
    /// `m_bestMatches`: query bits -> index of the best entry.
    best_matches: BTreeMap<[u64; 4], usize>,
}

impl SparseMatchFinder {
    /// An empty finder.
    pub fn new() -> Self {
        Self::default()
    }

    /// `clear()`: forget the cached results.
    pub fn clear(&mut self) {
        self.best_matches.clear();
    }

    /// `findBestInfoSlow(v, bits)`: the index of the best entry, or `None`.
    pub fn find_best_info_slow<T: BitFlagNames, M: SparseMatchable<T>>(v: &[M], bits: &BitFlags<T>) -> Option<usize> {
        let mut result = None;
        let mut best_yes_match: i32 = 0;
        let mut best_yes_extraneous_bits: i32 = 999;
        for (idx, it) in v.iter().enumerate() {
            let mut i = it.conditions_yes_count() - 1;
            while i >= 0 {
                let yes_flags = it.nth_conditions_yes(i);
                let yes_match = bits.count_intersection(yes_flags);
                let yes_extraneous_bits = bits.count_inverse_intersection(yes_flags);
                if (yes_match > best_yes_match)
                    || (yes_match >= best_yes_match && yes_extraneous_bits < best_yes_extraneous_bits)
                {
                    result = Some(idx);
                    best_yes_match = yes_match;
                    best_yes_extraneous_bits = yes_extraneous_bits;
                }
                i -= 1;
            }
        }
        result
    }

    /// `findBestInfo(v, bits)`: the cached result for `bits`, or
    /// [`SparseMatchFinder::find_best_info_slow`] (cached when found).
    pub fn find_best_info<T: BitFlagNames, M: SparseMatchable<T>>(&mut self, v: &[M], bits: &BitFlags<T>) -> Option<usize> {
        if let Some(&i) = self.best_matches.get(&bits.words()) {
            return Some(i);
        }
        let info = Self::find_best_info_slow(v, bits);
        if let Some(i) = info {
            self.best_matches.insert(bits.words(), i);
        }
        info
    }
}
