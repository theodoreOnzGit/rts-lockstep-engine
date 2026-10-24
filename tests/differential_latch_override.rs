// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 1 (`docs/phase1-porting-graph.md`):
//! `LatchRestore.h` (`common::latch_restore`) and `Overridable.h`/`Override.h`
//! (`common::overridable`), upstream commit 0a05454d.
//!
//! References: the unchanged headers, instantiated by
//! `differential/wrap_latch_override.cpp` (see build.rs). `LatchRestore<T>`
//! is run for `T = int` and for a two-int struct; the overrides are chains of
//! a derived template class with an id and a value.
//!
//! **LatchRestore.** Random scripts of open / assign / read / close on one
//! variable, with latches nested to depth 40. Each open is a scope in the
//! Rust (a recursive call under the guard, so the restore runs on the
//! scope's exit, including its `return`); each close is the scope's end. The
//! logs of reads and of the final value must be identical. 20,000 scripts
//! per type, with the value being latched and assigned freely.
//!
//! **Overridable / Override.** Random forests of override chains, with
//! random `markAsOverride` flags and random cuts (`setNextOverride(NULL)`,
//! which makes the displaced node a root). For every node the test compares
//! `getNextOverride`, `getFinalOverride`, the `OVERRIDE<T>` reads (`->`,
//! `*`, the conversion and `getNonOverloadedPointer`) and the result of
//! `deleteOverrides()`, and, after all roots are processed, which nodes the
//! C++ has deleted (the destructor of each derived node records it). The
//! outcome of every root is compared, so a node deleted by an earlier root
//! is a mismatch in the liveness flags.
//!
//! **Planted bugs**, each run against the same inputs and required to be
//! caught: a latch whose open does not restore on scope exit, and an
//! `OVERRIDE` that reads the head of its chain instead of the final override.
//!
//! **Results** (2026-10-18, release, `cargo test --release --features
//! differential -- --nocapture`), 0 mismatches in every gated comparison:
//!
//! - `LatchRestore<int>`: 20,000 scripts, 0 mismatches; planted bug caught in 18,419.
//! - `LatchRestore<struct{int,int}>`: 20,000 scripts, 0 mismatches; planted bug caught in 18,419.
//! - `Overridable`/`OVERRIDE` chains: 20,000 forests (1 to 8 nodes, with
//!   marks and cuts), 0 mismatches; planted bug caught in 14,728.
//!
//! Interpretation: the Rust follows the unchanged C++ for the latch
//! lifetime and the override chain, including deletion by `deleteOverrides`
//! and the ownership of displaced chains; the planted slips are visible at
//! these inputs, so the agreement is not an artefact of weak inputs.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::latch_restore::LatchRestore;
use rts_lockstep_engine::common::overridable::{Override, Overridable};
use rts_lockstep_engine::differential as cpp;
use std::cell::RefCell;
use std::rc::Rc;
use support::{Check, Gen};

const SCRIPTS: usize = 20_000;
const OUT_CAP: usize = 1 << 16;

// ---------------------------------------------------------------- LatchRestore

#[derive(Clone, Copy, Debug)]
enum Op {
    Open(i32),
    Set(i32),
    Read,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pair {
    a: i32,
    b: i32,
}

fn mk_int(v: i32) -> i32 {
    v
}

fn mk_pair(v: i32) -> Pair {
    Pair { a: v, b: -v }
}

fn emit_int(v: &i32, out: &mut Vec<i32>) {
    out.push(*v);
}

fn emit_pair(v: &Pair, out: &mut Vec<i32>) {
    out.push(v.a);
    out.push(v.b);
}

/// Runs the ops from `*pos` until the close of the current scope (or the end).
/// An `Open` is a scope: the guard lives in this frame and its restore runs
/// when the frame returns, which is the scope's end or an early return.
/// `planted` replaces the guard with a plain assignment that never restores.
fn exec<T: Copy>(
    var: &mut T,
    ops: &[Op],
    pos: &mut usize,
    out: &mut Vec<i32>,
    mk: fn(i32) -> T,
    emit: fn(&T, &mut Vec<i32>),
    planted: bool,
) {
    while *pos < ops.len() {
        let op = ops[*pos];
        *pos += 1;
        match op {
            Op::Set(v) => *var = mk(v),
            Op::Read => emit(var, out),
            Op::Close => return,
            Op::Open(v) => {
                if planted {
                    *var = mk(v);
                    exec(var, ops, pos, out, mk, emit, planted);
                } else {
                    let mut latch = LatchRestore::new(&mut *var, mk(v));
                    exec(&mut *latch, ops, pos, out, mk, emit, planted);
                }
            }
        }
    }
}

fn rust_latch<T: Copy>(ops: &[Op], mk: fn(i32) -> T, emit: fn(&T, &mut Vec<i32>), planted: bool) -> Vec<i32> {
    let mut var = mk(0);
    let mut out = Vec::new();
    let mut pos = 0;
    exec(&mut var, ops, &mut pos, &mut out, mk, emit, planted);
    emit(&var, &mut out);
    out
}

/// A random script: depth-balanced opens and closes, reads and assignments.
fn gen_latch_script(g: &mut Gen) -> Vec<Op> {
    let len = 1 + g.below(40) as usize;
    let mut ops = Vec::with_capacity(len);
    let mut depth = 0usize;
    for _ in 0..len {
        let v = g.i32() % 50;
        let op = match g.below(20) {
            0..=5 => {
                depth += 1;
                Op::Open(v)
            }
            6..=9 => Op::Set(v),
            10..=13 => Op::Read,
            _ if depth > 0 => {
                depth -= 1;
                Op::Close
            }
            _ => Op::Read,
        };
        ops.push(op);
    }
    ops
}

/// The flat encoding `ref_latch_restore` reads: two ints per op.
fn encode_latch(ops: &[Op]) -> Vec<i32> {
    let mut flat = Vec::with_capacity(ops.len() * 2);
    for op in ops {
        let (code, v) = match *op {
            Op::Open(v) => (0, v),
            Op::Set(v) => (1, v),
            Op::Read => (2, 0),
            Op::Close => (3, 0),
        };
        flat.push(code);
        flat.push(v);
    }
    flat
}

fn cpp_latch(pair: bool, ops: &[Op]) -> Vec<i32> {
    let flat = encode_latch(ops);
    let mut out = vec![0i32; OUT_CAP];
    let n = unsafe { cpp::ref_latch_restore(pair as i32, flat.as_ptr(), ops.len() as i32, out.as_mut_ptr(), OUT_CAP as i32) };
    assert!(n >= 0, "latch log exceeds the buffer");
    out.truncate(n as usize);
    out
}

#[test]
fn latch_restore_matches() {
    let mut g = Gen::new(301);
    let mut int_ok = Check::new("LatchRestore<int>");
    let mut int_bug = Check::new("LatchRestore<int>");
    let mut pair_ok = Check::new("LatchRestore<struct{int,int}>");
    let mut pair_bug = Check::new("LatchRestore<struct{int,int}>");
    for _ in 0..SCRIPTS {
        let ops = gen_latch_script(&mut g);
        let input = || format!("{ops:?}");

        let cpp_int = cpp_latch(false, &ops);
        int_ok.eq(rust_latch(&ops, mk_int, emit_int, false), cpp_int.clone(), input);
        int_bug.eq(rust_latch(&ops, mk_int, emit_int, true), cpp_int, input);

        let cpp_pair = cpp_latch(true, &ops);
        pair_ok.eq(rust_latch(&ops, mk_pair, emit_pair, false), cpp_pair.clone(), input);
        pair_bug.eq(rust_latch(&ops, mk_pair, emit_pair, true), cpp_pair, input);
    }
    int_ok.finish();
    pair_ok.finish();
    int_bug.finish_planted();
    pair_bug.finish_planted();
}

// ----------------------------------------------------- Overridable / Override

/// The derived template class: an id (its index in the forest) and a value.
/// Its drop records that the node has been deleted, which is how the test
/// sees `deleteOverrides` and the chain destructors on the Rust side.
struct Payload {
    id: usize,
    value: i32,
    alive: Rc<RefCell<Vec<bool>>>,
}

impl Drop for Payload {
    fn drop(&mut self) {
        self.alive.borrow_mut()[self.id] = false;
    }
}

/// The forest: 3 ints per node: value, next (index greater than the node's
/// own, or -1), flags (bit 0 markAsOverride, bit 1 cut the link to next).
/// Each node has at most one predecessor, so each node is owned once.
fn gen_forest(g: &mut Gen) -> Vec<i32> {
    let n = 1 + g.below(8) as usize;
    let mut has_pred = vec![false; n];
    let mut spec = Vec::with_capacity(3 * n);
    for i in 0..n {
        let value = (g.i32() % 1000) - 500;
        let mut next = -1i32;
        if i + 1 < n && g.below(3) != 0 {
            let start = i + 1 + g.below((n - i - 1) as u32) as usize;
            for k in 0..(n - i - 1) {
                let j = i + 1 + (start - i - 1 + k) % (n - i - 1);
                if !has_pred[j] {
                    has_pred[j] = true;
                    next = j as i32;
                    break;
                }
            }
        }
        let mut flags = 0;
        if g.below(3) == 0 {
            flags |= 1;
        }
        if next >= 0 && g.below(4) == 0 {
            flags |= 2;
        }
        spec.push(value);
        spec.push(next);
        spec.push(flags);
    }
    spec
}

fn id_of(p: Option<&Payload>) -> i32 {
    p.map_or(-1, |p| p.id as i32)
}

/// The same walk and log as `ref_overridable_chains`. `planted` makes
/// `OVERRIDE`'s reads return the chain's head instead of its final override.
fn rust_chains(spec: &[i32], planted: bool) -> Vec<i32> {
    let n = spec.len() / 3;
    let alive = Rc::new(RefCell::new(vec![true; n]));
    let mut table: Vec<Option<Box<Overridable<Payload>>>> = (0..n).map(|_| None).collect();
    for i in (0..n).rev() {
        let (value, next, flags) = (spec[3 * i], spec[3 * i + 1], spec[3 * i + 2]);
        let mut node = Box::new(Overridable::new(Payload { id: i, value, alive: Rc::clone(&alive) }));
        if next >= 0 {
            let j = next as usize;
            let child = table[j].take().expect("a node has one predecessor");
            let displaced = node.set_next_override(Some(child));
            assert!(displaced.is_none());
            if flags & 2 != 0 {
                // setNextOverride(NULL): the displaced chain becomes a root.
                let cut = node.set_next_override(None).expect("the displaced child");
                table[j] = Some(cut);
            }
        }
        if flags & 1 != 0 {
            node.mark_as_override();
        }
        table[i] = Some(node);
    }

    let mut log = Vec::new();
    let mut kept = Vec::new();
    for slot in table.iter_mut() {
        let Some(root) = slot.take() else { continue };
        let mut cur: &Overridable<Payload> = &root;
        loop {
            let ov = Override::new(Some(cur));
            let read = if planted {
                ov.get_non_overloaded_pointer().map(|p| &p.data)
            } else {
                ov.get()
            };
            log.push(cur.data.id as i32);
            log.push(cur.data.value);
            log.push(id_of(cur.get_next_override().map(|n| &n.data)));
            log.push(cur.get_final_override().data.id as i32);
            // operator->, operator* and the conversion all read the same final override.
            log.push(id_of(read));
            log.push(id_of(read));
            log.push(id_of(read));
            log.push(ov.get_non_overloaded_pointer().map_or(-1, |p| p.data.id as i32));
            match cur.get_next_override() {
                Some(next) => cur = next,
                None => break,
            }
        }
        let res = root.delete_overrides();
        log.push(res.as_ref().map_or(-1, |b| b.data.id as i32));
        kept.push(res);
    }
    for &flag in alive.borrow().iter() {
        log.push(flag as i32);
    }
    log
}

fn cpp_chains(spec: &[i32]) -> Vec<i32> {
    let n = spec.len() / 3;
    let mut out = vec![0i32; OUT_CAP];
    let len = unsafe { cpp::ref_overridable_chains(spec.as_ptr(), n as i32, out.as_mut_ptr(), OUT_CAP as i32) };
    assert!(len >= 0, "override log exceeds the buffer");
    out.truncate(len as usize);
    out
}

#[test]
fn overridable_chains_match() {
    let mut g = Gen::new(302);
    let mut ok = Check::new("Overridable / OVERRIDE (chains)");
    let mut bug = Check::new("Overridable / OVERRIDE (chains)");
    for _ in 0..SCRIPTS {
        let spec = gen_forest(&mut g);
        let input = || format!("spec (value, next, flags) = {spec:?}");
        let reference = cpp_chains(&spec);
        ok.eq(rust_chains(&spec, false), reference.clone(), input);
        bug.eq(rust_chains(&spec, true), reference, input);
    }
    ok.finish();
    bug.finish_planted();
}
