#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/multilist.h, multilist.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_wwlib::multilist::MultiListRegistry` against the upstream C++ (#215, group wwlib3).
//!
//! # Method
//!
//! `differential/wrap_draft_multilist.cpp` includes upstream's `multilist.h` and `multilist.cpp` unchanged (stand-ins: `mempool.h`'s
//! `AutoPoolClass`, `wwmemlog.h`'s `WWMEMLOG`: allocation machinery and a log). Three lists and eight objects; a program of 1 to 60
//! random ops: `Internal_Add`, `Internal_Add_Tail`, `Internal_Add_After` (each with `onlyonce` true or false, so an object can be in a
//! list twice), `Internal_Remove`, `Internal_Remove_List_Head`, destroying an object (`~MultiListObjectClass`, which leaves every list it
//! is in), `Contains`. After every op both sides output the op's result and, for each list, `Count()`, `Is_Empty()` and the iterator's
//! objects in order. 20,000 programs from a fixed seed.
//!
//! Not generated: `Internal_Add_After` with an `existing_list_member` that is not in this list. Upstream loops
//! `while ((existing_node->List != this) && (existing_node))`, which dereferences the NULL at the end of the chain before it
//! tests it, so it crashes where the draft's guard returns `false` (documented in the draft). Dropping a list that still holds
//! objects (`~GenericMultiListClass` asserts empty) is not exercised.
//!
//! # Predictions and results
//!
//! Predicted before the first run: a mismatch where an object is in one list twice and one of its nodes is removed: the draft's
//! `Internal_Remove` removed the first match from the head of the list, upstream removes the node found first on the object's chain
//! (the newest), which is not always the first in the ring (after `Add_Tail`, `Add_After`). Everything else 0.

mod support;

use rts_lockstep_engine::drafts::sweep_wwlib::multilist::{MultiListId, MultiListObjectId, MultiListRegistry};
use support::{Check, Gen};

extern "C" {
    fn ref_ml_run(prog: *const i32, nops: i32, out: *mut i32, cap: i32) -> i32;
}

const NL: u32 = 3;
const NO: u32 = 8;

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_ML_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

struct Prog {
    words: Vec<i32>,
    nops: i32,
}

fn gen_prog(g: &mut Gen) -> Prog {
    let nops = 1 + g.below(60) as i32;
    let mut w = Vec::new();
    let mut shadow = MultiListRegistry::new();
    let lists: Vec<MultiListId> = (0..NL).map(|_| shadow.new_list()).collect();
    for _ in 0..nops {
        let l = g.below(NL);
        let o = g.below(NO);
        let once = g.below(3) == 0; // mostly false: duplicates are what the remove cases need
        let mut op = g.below(7) as i32;
        if op == 2 {
            // an existing member of this list
            let members = shadow.objects(lists[l as usize]);
            if members.is_empty() {
                op = 0;
            } else {
                let e = members[g.below(members.len() as u32) as usize].0;
                w.extend([2, l as i32, o as i32, e as i32, once as i32]);
                shadow.internal_add_after(lists[l as usize], MultiListObjectId(o), MultiListObjectId(e), once);
                continue;
            }
        }
        match op {
            0 => {
                w.extend([0, l as i32, o as i32, once as i32]);
                shadow.internal_add(lists[l as usize], MultiListObjectId(o), once);
            }
            1 => {
                w.extend([1, l as i32, o as i32, once as i32]);
                shadow.internal_add_tail(lists[l as usize], MultiListObjectId(o), once);
            }
            3 => {
                w.extend([3, l as i32, o as i32]);
                shadow.internal_remove(lists[l as usize], MultiListObjectId(o));
            }
            4 => {
                w.extend([4, l as i32, o as i32]);
                shadow.internal_remove_list_head(lists[l as usize]);
            }
            5 => {
                w.extend([5, l as i32, o as i32]);
                shadow.drop_object(MultiListObjectId(o));
            }
            6 => w.extend([6, l as i32, o as i32]),
            _ => {
                w.extend([0, l as i32, o as i32, 0]);
                shadow.internal_add(lists[l as usize], MultiListObjectId(o), false);
            }
        }
    }
    Prog { words: w, nops }
}

fn run_rust(p: &Prog) -> Vec<i32> {
    let mut reg = MultiListRegistry::new();
    let lists: Vec<MultiListId> = (0..NL).map(|_| reg.new_list()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut next = || {
        let v = p.words[i];
        i += 1;
        v
    };
    for _ in 0..p.nops {
        let op = next();
        let l = next() as usize;
        let o = MultiListObjectId(next() as u32);
        match op {
            0 => {
                let once = next() != 0;
                out.push(reg.internal_add(lists[l], o, once) as i32);
            }
            1 => {
                let once = next() != 0;
                out.push(reg.internal_add_tail(lists[l], o, once) as i32);
            }
            2 => {
                let e = MultiListObjectId(next() as u32);
                let once = next() != 0;
                out.push(reg.internal_add_after(lists[l], o, e, once) as i32);
            }
            3 => out.push(reg.internal_remove(lists[l], o) as i32),
            4 => out.push(reg.internal_remove_list_head(lists[l]).map_or(-1, |x| x.0 as i32)),
            5 => reg.drop_object(o),
            6 => out.push(reg.contains(lists[l], o) as i32),
            _ => unreachable!(),
        }
        for &li in &lists {
            out.push(reg.count(li));
            out.push(reg.is_empty(li) as i32);
            out.extend(reg.objects(li).iter().map(|x| x.0 as i32));
        }
    }
    out
}

fn run_cpp(p: &Prog) -> Vec<i32> {
    let mut out = vec![0i32; 4096];
    let n = unsafe { ref_ml_run(p.words.as_ptr(), p.nops, out.as_mut_ptr(), out.len() as i32) } as usize;
    assert!(n <= out.len());
    out.truncate(n);
    out
}

#[test]
fn multilist_matches_upstream_over_random_programs() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_17381);
    let mut c = Check::new("MultiListRegistry programs");
    for i in 0..n {
        let p = gen_prog(&mut g);
        let cpp = run_cpp(&p);
        let rs = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_rust(&p))).ok();
        c.eq(rs, Some(cpp), || format!("program {i}: {:?}", p.words));
    }
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}
