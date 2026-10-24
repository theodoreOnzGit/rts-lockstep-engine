// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/PartitionSolver.h
//   GeneralsMD/Code/GameEngine/Source/Common/PartitionSolver.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Created: John K. McDonald, Jr., 4/2/2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PartitionSolver`: a greedy first-fit-decreasing packer. Given entries
//! (an id and a size) and spaces (an id and a capacity), it puts each entry,
//! largest first, into the first space (largest first) that still has room,
//! and skips an entry that fits nowhere. `PartitionManager` and
//! `ScriptActions` use it.
//!
//! # Sort order for ties (read this before relying on replays)
//!
//! Upstream orders both lists with `std::sort` and a comparator on the size
//! alone. `std::sort` is not stable, so among entries (or spaces) of equal
//! size the order was whatever the shipped STLport 4.x introsort produced,
//! and equal sizes are common. **This translation uses a documented
//! deterministic order instead: a stable sort by size, largest first, so
//! equal sizes keep their original (input) order.** Equal-sized entries are
//! therefore tried in the order they were given, and equal-sized spaces are
//! filled in the order they were given. Rust-to-Rust lockstep is exact,
//! because the order is a pure function of the input.
//!
//! **Matching original replays needs STLport's sort reproduced.** STLport is
//! not in the repository (the `stlport.diff` at its root only names the
//! files EA patched), so that is not done; wherever the sizes in a list are
//! all distinct the result is the same under any correct sort and agrees
//! with the original, and the differential test compares only such inputs.
//!
//! # Other notes
//!
//! - `PREFER_CORRECT_SOLUTION` is unimplemented upstream (a `DEBUG_CRASH`
//!   `@todo`, compiled out in release): `solve` then leaves the solution
//!   empty, and so does this translation.
//! - Upstream also sums the sizes of the entries and of the spaces into two
//!   locals it never reads. They are not computed here (the sums could
//!   overflow `Int`, with no effect).
//! - There is no floating-point arithmetic, so nothing goes through
//!   `petir::Real`.

use super::game_type::ObjectId;

/// `typedef std::pair<ObjectID, UnsignedInt> PairObjectIDAndUInt`: an id and
/// a size (for an entry) or a capacity (for a space).
pub type PairObjectIdAndUInt = (ObjectId, u32);
/// `typedef std::pair<ObjectID, ObjectID> PairObjectID`: (entry id, space id).
pub type PairObjectId = (ObjectId, ObjectId);
/// `EntriesVec`: the identifying id of each entry and its size.
pub type EntriesVec = Vec<PairObjectIdAndUInt>;
/// `SpacesVec`: the identifying id of each space and the room in it.
pub type SpacesVec = Vec<PairObjectIdAndUInt>;
/// `SolutionVec`: (entry id, id of the space it goes in), in placement order.
pub type SolutionVec = Vec<PairObjectId>;

/// `enum SolutionType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SolutionType {
    /// `PREFER_FAST_SOLUTION = 0`.
    PreferFastSolution = 0,
    /// `PREFER_CORRECT_SOLUTION = 0x7FFFFFFF`.
    PreferCorrectSolution = 0x7FFF_FFFF,
}

/// `class PartitionSolver`.
#[derive(Clone, Debug)]
pub struct PartitionSolver {
    how_to_solve: SolutionType,
    data: EntriesVec,
    spaces_for_data: SpacesVec,
    current_solution: SolutionVec,
    current_solution_leftovers: u32,
    best_solution: SolutionVec,
}

impl PartitionSolver {
    /// `PartitionSolver(elements, spaces, solveHow)`.
    pub fn new(
        elements: &[PairObjectIdAndUInt],
        spaces: &[PairObjectIdAndUInt],
        solve_how: SolutionType,
    ) -> Self {
        PartitionSolver {
            how_to_solve: solve_how,
            data: elements.to_vec(),
            spaces_for_data: spaces.to_vec(),
            current_solution: Vec::new(),
            current_solution_leftovers: 0,
            best_solution: Vec::new(),
        }
    }

    /// `solve()`. Sorts the stored entries and spaces (largest first, equal
    /// sizes in their current order: see the module docs) and then places
    /// each entry in the first space that has room.
    pub fn solve(&mut self) {
        self.best_solution.clear();
        self.current_solution.clear();
        self.current_solution_leftovers = 0x7fff_ffff;

        // `greater_than`: a.second > b.second. Stable, so ties keep input order.
        self.data.sort_by_key(|e| std::cmp::Reverse(e.1));
        self.spaces_for_data.sort_by_key(|e| std::cmp::Reverse(e.1));

        let mut spaces_still_available = self.spaces_for_data.clone();

        if self.how_to_solve == SolutionType::PreferFastSolution {
            for entry in &self.data {
                for space in spaces_still_available.iter_mut() {
                    if entry.1 <= space.1 {
                        space.1 -= entry.1;
                        self.best_solution.push((entry.0, space.0));
                        break;
                    }
                }
            }
        }
        // else: DEBUG_CRASH("PREFER_CORRECT_SOLUTION @todo impl"), compiled out.
    }

    /// `getSolution()`.
    pub fn get_solution(&self) -> &SolutionVec {
        &self.best_solution
    }

    /// The entries as `solve` last ordered them (the order is observable
    /// upstream through the protected `m_data`).
    pub fn data(&self) -> &EntriesVec {
        &self.data
    }

    /// The spaces as `solve` last ordered them (`m_spacesForData`).
    pub fn spaces(&self) -> &SpacesVec {
        &self.spaces_for_data
    }

    /// `m_currentSolutionLeftovers`: 0 after construction, `0x7fffffff`
    /// after `solve` (upstream never updates it).
    pub fn current_solution_leftovers(&self) -> u32 {
        self.current_solution_leftovers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SolutionType::*;

    fn solve(e: &[PairObjectIdAndUInt], s: &[PairObjectIdAndUInt]) -> SolutionVec {
        let mut p = PartitionSolver::new(e, s, PreferFastSolution);
        p.solve();
        p.get_solution().clone()
    }

    #[test]
    fn largest_first_into_first_space_with_room() {
        // sizes 3, 5, 4 sorted to 5, 4, 3; spaces 4, 9 sorted to 9, 4.
        let r = solve(&[(1, 3), (2, 5), (3, 4)], &[(10, 4), (20, 9)]);
        // 5 -> space 20 (4 left); 4 -> space 20 (0 left); 3 -> space 10 (1 left).
        assert_eq!(r, vec![(2, 20), (3, 20), (1, 10)]);
    }

    #[test]
    fn entry_that_fits_nowhere_is_skipped() {
        let r = solve(&[(1, 100), (2, 2)], &[(10, 5)]);
        assert_eq!(r, vec![(2, 10)]);
    }

    #[test]
    fn empty_inputs_and_zero_sizes() {
        assert!(solve(&[], &[(1, 5)]).is_empty());
        assert!(solve(&[(1, 5)], &[]).is_empty());
        // A zero-size entry fits even a zero-capacity space.
        assert_eq!(solve(&[(7, 0)], &[(9, 0)]), vec![(7, 9)]);
    }

    #[test]
    fn size_is_unsigned() {
        let big = 0x8000_0000u32;
        // `big` sorts first and fills the space; the entry of size 1 then fits nowhere.
        assert_eq!(solve(&[(1, 1), (2, big)], &[(5, big)]), vec![(2, 5)]);
    }

    /// The documented tie order: a stable sort by size, largest first.
    #[test]
    fn tie_order_is_stable_by_input_index() {
        let mut p = PartitionSolver::new(
            &[(1, 2), (2, 7), (3, 2), (4, 7), (5, 2)],
            &[(10, 4), (20, 4), (30, 9)],
            PreferFastSolution,
        );
        p.solve();
        assert_eq!(p.data(), &vec![(2, 7), (4, 7), (1, 2), (3, 2), (5, 2)]);
        assert_eq!(p.spaces(), &vec![(30, 9), (10, 4), (20, 4)]);
        // 7 (id 2) -> 30 (2 left); 7 (id 4) fits nowhere; 2 (id 1) -> 30 (0 left);
        // 2 (id 3) -> 10 (2 left); 2 (id 5) -> 10 (0 left).
        assert_eq!(p.get_solution(), &vec![(2, 30), (1, 30), (3, 10), (5, 10)]);
    }

    /// Equal-sized spaces are filled in input order; reversing the input
    /// order of the spaces reverses which one is used.
    #[test]
    fn equal_spaces_fill_in_input_order() {
        assert_eq!(solve(&[(1, 3)], &[(10, 5), (20, 5)]), vec![(1, 10)]);
        assert_eq!(solve(&[(1, 3)], &[(20, 5), (10, 5)]), vec![(1, 20)]);
    }

    /// Solving twice gives the same answer (the stored lists stay sorted, and
    /// the sort is stable).
    #[test]
    fn solve_is_repeatable() {
        let mut p = PartitionSolver::new(
            &[(1, 2), (2, 7), (3, 2)],
            &[(10, 4), (20, 4), (30, 9)],
            PreferFastSolution,
        );
        p.solve();
        let first = p.get_solution().clone();
        p.solve();
        assert_eq!(p.get_solution(), &first);
        assert_eq!(p.current_solution_leftovers(), 0x7fff_ffff);
    }

    #[test]
    fn leftovers_start_at_zero_and_correct_solution_is_unimplemented() {
        let mut p = PartitionSolver::new(&[(1, 1)], &[(2, 5)], PreferCorrectSolution);
        assert_eq!(p.current_solution_leftovers(), 0);
        p.solve();
        assert!(p.get_solution().is_empty());
        assert_eq!(SolutionType::PreferCorrectSolution as i32, 0x7FFF_FFFF);
    }
}
