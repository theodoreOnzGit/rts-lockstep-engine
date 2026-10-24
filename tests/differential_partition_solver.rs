// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::partition_solver` against upstream
//! `GameEngine/Source/Common/PartitionSolver.cpp` and
//! `Include/Common/PartitionSolver.h` (commit 0a05454d), compiled as
//! `differential/wrap_partition_solver.cpp` describes.
//!
//! # Method
//!
//! 20,000 random problems (0 to 12 entries and 0 to 8 spaces; sizes from a
//! small range, from the full `u32` range and around `2^31`, some zero; ids
//! arbitrary `i32`), solved by `PREFER_FAST_SOLUTION` in both, plus
//! `PREFER_CORRECT_SOLUTION` (unimplemented upstream: an empty solution).
//! The (entry id, space id) pairs are compared exactly, in order.
//!
//! # Ties are kept out
//!
//! Upstream sorts with `std::sort`, which is not stable, so with equal sizes
//! the order is the sort implementation's (the original used STLport's, not
//! in the repository; the reference here is libstdc++'s). The translation
//! uses a stable sort instead (see the module docs of
//! `common::partition_solver`). So the differential comparison covers only
//! problems whose sizes are distinct within the entries and distinct within
//! the spaces; problems with a tie in either list are counted, reported and
//! **not** compared with the C++. The tie order is unit-tested in the module.
//! The generator makes about half of the problems tie-free by construction
//! (distinct sizes) and lets the rest tie by chance.
//!
//! Planted bugs, each of which must be caught: spaces sorted smallest first;
//! the fit test `<` instead of `<=`.
//!
//! Results: see the `DIFF` lines printed by `cargo test --release --features
//! differential --test differential_partition_solver -- --nocapture`.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::partition_solver::{
    PairObjectIdAndUInt, PartitionSolver, SolutionType,
};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

fn sizes(g: &mut Gen, n: usize, distinct: bool) -> Vec<u32> {
    let range: u32 = match g.below(4) {
        // 13 distinct values do not fit in 0..6, so a distinct list needs a wider range
        0 if !distinct => 6,
        0 => 40,
        1 => 40,
        2 => 1000,
        _ => u32::MAX,
    };
    let mut v: Vec<u32> = Vec::new();
    while v.len() < n {
        let x = match g.below(12) {
            0 => 0,
            1 => 0x8000_0000 - g.below(3),
            2 => 0x8000_0000 + g.below(3),
            _ => {
                if range == u32::MAX {
                    g.u32()
                } else {
                    g.u32() % range
                }
            }
        };
        if !distinct || !v.contains(&x) {
            v.push(x);
        }
    }
    v
}

fn has_tie(v: &[PairObjectIdAndUInt]) -> bool {
    (0..v.len()).any(|i| (i + 1..v.len()).any(|j| v[i].1 == v[j].1))
}

fn cpp_solve(how: i32, e: &[PairObjectIdAndUInt], s: &[PairObjectIdAndUInt]) -> Vec<(i32, i32)> {
    let (ei, es): (Vec<i32>, Vec<u32>) = e.iter().copied().unzip();
    let (si, ss): (Vec<i32>, Vec<u32>) = s.iter().copied().unzip();
    let mut out = vec![0i32; 2 * 32];
    let n = unsafe {
        cpp::ref_partition_solve(
            how,
            ei.as_ptr(),
            es.as_ptr(),
            e.len() as i32,
            si.as_ptr(),
            ss.as_ptr(),
            s.len() as i32,
            out.as_mut_ptr(),
            32,
        )
    };
    assert!(n >= 0, "output buffer");
    (0..n as usize)
        .map(|i| (out[2 * i], out[2 * i + 1]))
        .collect()
}

/// The planted-bug variants, written out: `(spaces smallest first, strict fit)`.
fn planted(
    e: &[PairObjectIdAndUInt],
    s: &[PairObjectIdAndUInt],
    smallest_first: bool,
    strict: bool,
) -> Vec<(i32, i32)> {
    let mut data = e.to_vec();
    let mut spaces = s.to_vec();
    data.sort_by_key(|e| std::cmp::Reverse(e.1));
    if smallest_first {
        spaces.sort_by_key(|e| e.1);
    } else {
        spaces.sort_by_key(|e| std::cmp::Reverse(e.1));
    }
    let mut out = Vec::new();
    for d in &data {
        for sp in spaces.iter_mut() {
            if if strict { d.1 < sp.1 } else { d.1 <= sp.1 } {
                sp.1 -= d.1;
                out.push((d.0, sp.0));
                break;
            }
        }
    }
    out
}

#[test]
fn partition_solver_matches() {
    let mut g = Gen::new(401);
    let mut ok = Check::new("PartitionSolver (tie-free problems)");
    let mut bug_order = Check::new("PartitionSolver (tie-free problems)");
    let mut bug_fit = Check::new("PartitionSolver (tie-free problems)");
    let mut correct = Check::new("PartitionSolver PREFER_CORRECT_SOLUTION");
    let (mut total, mut tied, mut placed, mut skipped) = (0u32, 0u32, 0u64, 0u64);
    for _ in 0..20_000 {
        total += 1;
        let (ne, ns) = (g.below(13) as usize, g.below(9) as usize);
        let distinct = g.below(2) == 0;
        let es = sizes(&mut g, ne, distinct);
        let ss = sizes(&mut g, ns, distinct);
        let e: Vec<PairObjectIdAndUInt> = es.iter().map(|&z| (g.i32(), z)).collect();
        let s: Vec<PairObjectIdAndUInt> = ss.iter().map(|&z| (g.i32(), z)).collect();
        if has_tie(&e) || has_tie(&s) {
            tied += 1;
            continue;
        }
        let mut p = PartitionSolver::new(&e, &s, SolutionType::PreferFastSolution);
        p.solve();
        let want = cpp_solve(0, &e, &s);
        placed += want.len() as u64;
        skipped += (e.len() - want.len()) as u64;
        ok.eq(p.get_solution().clone(), want.clone(), || {
            format!("entries {e:?} spaces {s:?}")
        });
        bug_order.eq(planted(&e, &s, true, false), want.clone(), || {
            format!("entries {e:?} spaces {s:?}")
        });
        bug_fit.eq(planted(&e, &s, false, true), want, || {
            format!("entries {e:?} spaces {s:?}")
        });
        let mut q = PartitionSolver::new(&e, &s, SolutionType::PreferCorrectSolution);
        q.solve();
        correct.eq(q.get_solution().clone(), cpp_solve(1, &e, &s), || {
            format!("entries {e:?} spaces {s:?}")
        });
    }
    println!(
        "PartitionSolver: {total} problems generated, {tied} had a tie in the entry or space sizes (kept out), {} compared; {placed} placements, {skipped} entries left unplaced",
        total - tied
    );
    assert!(
        tied > 1000 && total - tied > 5000,
        "the generator must give both tied and tie-free problems"
    );
    assert!(
        placed > 5000 && skipped > 1000,
        "the problems must exercise both placing and skipping"
    );
    ok.finish();
    correct.finish();
    bug_order.finish_planted();
    bug_fit.finish_planted();
}
