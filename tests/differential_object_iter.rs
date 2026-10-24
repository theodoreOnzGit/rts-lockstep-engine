// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::object_iter` (`SimpleObjectIterator`)
//! against upstream `GameLogic/Object/SimpleObjectIterator.cpp` (commit
//! 0a05454d), compiled unchanged in `differential/wrap_object_iter.cpp` on
//! the stand-ins of `differential/shim_thing` (see build.rs). Phase 1 layer 5.
//!
//! # Methodology
//!
//! A **program** is a random set of objects 1..n, each with a build cost, and
//! 1 to 40 operations on one iterator: `insert` (at the head, with a numeric
//! key), `sort` (any of the five orders, `ITER_FASTEST` included), a walk
//! with `firstWithNumeric`/`nextWithNumeric` to the end, a walk with
//! `first`/`next`, `getCount`, and `makeEmpty`. The same program runs in the
//! Rust and in the C++; every element a walk returns is compared (its object
//! id and the numeric key's bits), the end-of-walk numeric is compared, and
//! so is every count.
//!
//! The inputs are chosen to reach the places where a merge sort can differ
//! from a different sort:
//!
//! - **ties**: numeric keys from a few small integers, with both zeros, so
//!   that the tie rule (`cmp <= 0` takes the earlier run) decides the order;
//! - **costs** from a small range (ties), from the full `int` range (where
//!   upstream's `Int` subtraction wraps, `-fwrapv`), and at the extremes;
//! - **NaN and infinite keys**, where `cmp <= 0` is false and the second run
//!   is taken, and where a library sort would be allowed to panic.
//!
//! **Undefined upstream inputs** are not sent to the C++ (it would store a
//! null clump, which ends an iteration early, and crash in a cost sort). The
//! Rust must return `NullObject` for them, and leave the iterator as it was.
//! The test `iter_undefined_inputs_are_errors` checks that.
//!
//! Comparisons are on bits (`support::Check`), with the NaN-payload rule of
//! the other differential tests.
//!
//! **Planted bugs.** Each comparison was also run against one deliberately
//! wrong translation (see the results), planted by hand in
//! `src/logic/object_iter.rs`, observed to fail, and removed.
//!
//! # Results
//!
//! See the `DIFF` lines of `cargo test --release --features differential
//! --test differential_object_iter -- --nocapture`, and the summary in
//! `docs/phase1-porting-graph.md`.

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::object_iter::{IterOrderType, ObjectIterError, SimpleObjectIterator};
use support::{Check, Gen};

/// Programs per run.
const PROGRAMS: usize = 6000;
/// Operation codes, as `ref_object_iter_run` reads them.
const OP_NAMES: [&str; 6] = ["insert", "sort", "iterate with numeric", "iterate ids", "getCount", "makeEmpty"];
/// The end marker of a walk (no object has this id).
const END: u32 = 0xFFFF_FFFF;

#[derive(Clone, Debug)]
struct Program {
    /// `costs[i]` is the build cost of object `i` (index 0 unused).
    costs: Vec<i32>,
    /// (code, argument, numeric key), with the codes of `ref_object_iter_run`.
    ops: Vec<(i32, i32, f32)>,
}

fn gen_cost(g: &mut Gen) -> i32 {
    match g.below(4) {
        0 | 1 => g.below(7) as i32 - 3,
        2 => g.u32() as i32,
        _ => [i32::MIN, i32::MAX, 0, -1, 1][g.below(5) as usize],
    }
}

fn gen_numeric(g: &mut Gen) -> f32 {
    match g.below(10) {
        0..=3 => g.below(5) as f32 - 2.0,
        4 => {
            if g.below(2) == 0 {
                0.0
            } else {
                -0.0
            }
        }
        5 => g.special(),
        _ => g.mixed(),
    }
}

fn gen_program(g: &mut Gen) -> Program {
    let n_objs = 1 + g.below(40) as usize;
    let mut costs = vec![0i32];
    for _ in 0..n_objs {
        costs.push(gen_cost(g));
    }
    let n_ops = 1 + g.below(40) as usize;
    let mut ops = Vec::with_capacity(n_ops);
    for _ in 0..n_ops {
        let op = match g.below(100) {
            0..=39 => (0, 1 + g.below(n_objs as u32) as i32, gen_numeric(g)),
            40..=59 => (1, g.below(5) as i32, 0.0),
            60..=74 => (2, 0, 0.0),
            75..=84 => (3, 0, 0.0),
            85..=94 => (4, 0, 0.0),
            _ => (5, 0, 0.0),
        };
        ops.push(op);
    }
    Program { costs, ops }
}

fn order_of(k: i32) -> IterOrderType {
    match k {
        0 => IterOrderType::Fastest,
        1 => IterOrderType::SortedNearToFar,
        2 => IterOrderType::SortedFarToNear,
        3 => IterOrderType::SortedCheapToExpensive,
        4 => IterOrderType::SortedExpensiveToCheap,
        _ => panic!("order {k}"),
    }
}

/// The words of one program, with each operation's code and its range.
struct Trace {
    words: Vec<u32>,
    segments: Vec<(i32, std::ops::Range<usize>)>,
}

fn run_rust(p: &Program) -> Trace {
    let mut it: SimpleObjectIterator<u32> = SimpleObjectIterator::new();
    let mut words = Vec::new();
    let mut segments = Vec::new();
    for &(code, arg, num) in &p.ops {
        let start = words.len();
        match code {
            0 => it.insert(Some(arg as u32), num).expect("a non-null object"),
            1 => it.sort(order_of(arg), |id: &u32| p.costs[*id as usize]),
            2 => {
                let (mut obj, mut n) = it.first_with_numeric();
                while let Some(o) = obj {
                    words.push(o);
                    words.push(n.to_bits());
                    (obj, n) = it.next_with_numeric();
                }
                words.push(END);
                words.push(n.to_bits());
            }
            3 => {
                let mut obj = it.first();
                while let Some(o) = obj {
                    words.push(o);
                    obj = it.next();
                }
                words.push(END);
            }
            4 => words.push(it.count() as u32),
            5 => it.make_empty(),
            c => panic!("unknown op {c}"),
        }
        segments.push((code, start..words.len()));
    }
    Trace { words, segments }
}

fn run_cpp(p: &Program) -> Vec<u32> {
    let mut ops = Vec::with_capacity(3 * p.ops.len());
    let mut nums = Vec::with_capacity(p.ops.len());
    for &(code, arg, num) in &p.ops {
        ops.extend([code, arg, 0]);
        nums.push(num);
    }
    let cap = 4096 * p.ops.len() + 4096;
    let mut out = vec![0u32; cap];
    let n = unsafe {
        cpp::ref_object_iter_run(
            (p.costs.len() - 1) as i32,
            p.costs.as_ptr(),
            p.ops.len() as i32,
            ops.as_ptr(),
            nums.as_ptr(),
            out.as_mut_ptr(),
            cap as i32,
        )
    };
    assert!(n >= 0, "ref_object_iter_run returned {n}");
    out.truncate(n as usize);
    out
}

#[test]
fn iter_programs_match_cpp() {
    let mut g = Gen::new(0x0173);
    let mut checks: Vec<Check> = OP_NAMES.iter().map(|n| Check::new(&format!("SimpleObjectIterator {n}"))).collect();
    let mut lengths = Check::new("SimpleObjectIterator words per program");
    let (mut programs, mut ops_run) = (0u64, 0u64);
    for i in 0..PROGRAMS {
        let p = gen_program(&mut g);
        let trace = run_rust(&p);
        let cppw = run_cpp(&p);
        programs += 1;
        ops_run += p.ops.len() as u64;
        lengths.eq(trace.words.len(), cppw.len(), || format!("program {i}: {p:?}"));
        if trace.words.len() != cppw.len() {
            continue;
        }
        for (j, (code, range)) in trace.segments.iter().enumerate() {
            if range.is_empty() {
                continue;
            }
            let c = &mut checks[*code as usize];
            for k in range.clone() {
                c.eq(trace.words[k], cppw[k], || format!("program {i} op {j} (code {code}) word {k}: {p:?}"));
            }
        }
    }
    println!("DIFF iterator programs {programs}, operations {ops_run}");
    lengths.finish();
    // `insert`, `sort` and `makeEmpty` write no words of their own: their
    // effect is in the walks and counts that follow, which are codes 2 to 4.
    for c in checks.into_iter().skip(2).take(3) {
        c.finish();
    }
}

/// `insert(NULL, ...)` is undefined upstream: the Rust returns `NullObject`
/// and the iterator is unchanged, with the same walk afterwards.
#[test]
fn iter_undefined_inputs_are_errors() {
    let mut it: SimpleObjectIterator<u32> = SimpleObjectIterator::new();
    it.insert(Some(7), 1.0).expect("object");
    it.insert(Some(9), -2.0).expect("object");
    let before: Vec<u32> = {
        let mut v = Vec::new();
        let mut o = it.first();
        while let Some(x) = o {
            v.push(x);
            o = it.next();
        }
        v
    };
    let mut n_null = 0u64;
    let mut g = Gen::new(0x0174);
    for _ in 0..1000 {
        let num = gen_numeric(&mut g);
        match it.insert(None, num) {
            Err(ObjectIterError::NullObject) => n_null += 1,
            other => panic!("insert(NULL): expected NullObject, got {other:?}"),
        }
    }
    let after: Vec<u32> = {
        let mut v = Vec::new();
        let mut o = it.first();
        while let Some(x) = o {
            v.push(x);
            o = it.next();
        }
        v
    };
    assert_eq!(before, after, "a refused insert changed the iterator");
    assert_eq!(it.count(), 2);
    println!("DIFF iterator undefined inputs: insert(NULL) refused {n_null} times, the list unchanged");
}
