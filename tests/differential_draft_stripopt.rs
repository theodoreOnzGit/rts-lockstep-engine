// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dpure) of the draft `src/drafts/ww3d/stripoptimizer.rs` against upstream
//! `WW3D2/stripoptimizer.cpp`, compiled WHOLE (a copy with three mechanical for-scope edits, build.rs `drafts_ww3dpure`,
//! differential/wrap_draft_stripopt.cpp). No stand-ins: the real hashtemplate.h, always.h and wwstring.h.
//!
//! # Method
//!
//! 20,000 cases per function from a fixed seed (`DRAFT_CASES` overrides). Everything is integer data, compared word for word.
//! - `Stripify`: triangle soups over a pool of 3..=24 vertices (random distinct-vertex triangles, duplicated triangles, and
//!   non-manifold edges, which share edges in either direction), plus grid meshes (each quad split one of two ways, random
//!   winding flips and rotations of each triangle), plus `N = 0` (NULL).
//! - `Combine_Strips`: 1..=8 strips of 1..=12 indices.
//! - `Optimize_Strip_Order`, `Get_Strip_Index_Count`: 1..=10 strips of 0..=10 indices over a small index pool (many shared).
//! - `Optimize_Triangle_Order`: 1..=30 triangles over a small pool, degenerate triangles included.
//! - `Insertion_Sort` and `Quick_Sort` (templates not called by anything else in the file): ints and `Tri` (ordered by its
//!   first member only, the other two carry the original position so that any difference in the order of ties shows), 0..=70
//!   elements, a small value range (many ties) and a wide one.
//!
//! # Excluded inputs
//!
//! - `Stripify`: triangles with a repeated vertex (upstream's header: "assumes degenerate triangles have been removed"; such
//!   a triangle can be its own neighbour), and negative indices.
//! - `Combine_Strips`: zero strips (a negative allocation) and strips of length 0 (reads the next strip's length as an index,
//!   and past the end for the last).
//! - `Combine_Strips` writes `*o++ = o[-1]`, unsequenced before C++17; the build is `-std=gnu++17`, which sequences the right
//!   operand first, so the copy of the last index written is what both sides produce.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches. The draft keeps upstream's two findings (the `weight[0]` quirk and the last-matching-neighbour search) on
//! purpose, so I do not expect a difference there either; a hash-order difference is impossible because the hash is only
//! queried, never iterated. The riskiest place is non-manifold input.
//!
//! # Results (2026-11-19)
//!
//! **0 mismatches on the first run**: 20,000 cases of each of the 9 checks (`Stripify`, `Combine_Strips`, `Optimize_Strip_Order`,
//! `Get_Strip_Index_Count`, `Optimize_Triangle_Order`, `Insertion_Sort` and `Quick_Sort` on `int` and on `Tri`), no difference.
//! **Prediction held.** One thing the test taught: because of the `weight[0]` quirk the node weights are +2 or -2 for every
//! neighbour alike, so they never change a decision and the per-vertex connectivity count is dead state upstream. A first plant that
//! skipped its decrement (14673) was therefore NOT caught (an equivalent mutant) and was replaced by one that skips a neighbour's
//! reinsertion; the weight quirk itself is caught by 14661 (`weight[i]` for `weight[0]`).
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching cases of 20,000, summed over the checks it affects)
//!   14661: 2,863; 14662: 12,063; 14663: 12,486; 14664: 17,506
//!   14665: 20,000; 14666: 556; 14667: 6,800; 14668: 1,020
//!   14669: 1,838; 14670: 19,600; 14671: 18,475; 14672: 2,258
//!   14673: 2,608; 14674: 5,323; 14675: 28,526; 14676: 8,349
//!   14677: 1,900; 14678: 29,199
//!
//! **18 of 18 caught.**

mod support;

use rts_lockstep_engine::drafts::ww3d::stripoptimizer as so;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dpw_stripify(tris: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_dpw_combine(strips: *const i32, count: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_dpw_opt_strips(strips: *mut i32, count: i32, total: i32);
    fn ref_dpw_opt_tris(tris: *mut i32, n: i32);
    fn ref_dpw_index_count(strips: *const i32, count: i32) -> i32;
    fn ref_dpw_insertion_sort(a: *mut i32, n: i32);
    fn ref_dpw_quick_sort(a: *mut i32, n: i32);
    fn ref_dpw_insertion_sort_tri(a: *mut i32, n: i32);
    fn ref_dpw_quick_sort_tri(a: *mut i32, n: i32);
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        bad += c.mismatches;
        if !planted {
            c.finish();
        }
    }
    if planted {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

/// A triangle soup: flat `i32` triples.
fn soup(g: &mut Gen) -> Vec<i32> {
    let mut t: Vec<i32> = Vec::new();
    match g.below(3) {
        0 => {
            // random distinct-vertex triangles over a small pool (edges shared in either direction, duplicates)
            let pool = 3 + g.below(22) as i32;
            let n = 1 + g.below(40);
            for _ in 0..n {
                let a = g.below(pool as u32) as i32;
                let mut b = g.below(pool as u32) as i32;
                while b == a {
                    b = g.below(pool as u32) as i32;
                }
                let mut c = g.below(pool as u32) as i32;
                while c == a || c == b {
                    c = g.below(pool as u32) as i32;
                }
                t.extend_from_slice(&[a, b, c]);
            }
        }
        1 => {
            // a grid: w x h quads, each split one of two ways
            let w = 1 + g.below(5) as i32;
            let h = 1 + g.below(5) as i32;
            for y in 0..h {
                for x in 0..w {
                    let v = |dx: i32, dy: i32| (y + dy) * (w + 1) + x + dx;
                    let tris = if g.below(2) == 0 {
                        [[v(0, 0), v(1, 0), v(1, 1)], [v(0, 0), v(1, 1), v(0, 1)]]
                    } else {
                        [[v(0, 0), v(1, 0), v(0, 1)], [v(1, 0), v(1, 1), v(0, 1)]]
                    };
                    for mut tri in tris {
                        if g.below(8) == 0 {
                            tri.swap(0, 1); // winding flip
                        }
                        let r = g.below(3) as usize;
                        tri.rotate_left(r);
                        t.extend_from_slice(&tri);
                    }
                }
            }
            // shuffle the triangles
            let n = t.len() / 3;
            for i in (1..n).rev() {
                let j = g.below(i as u32 + 1) as usize;
                for k in 0..3 {
                    t.swap(i * 3 + k, j * 3 + k);
                }
            }
        }
        _ => {
            // a fan / strip-like chain with a few extra random triangles
            let n = 2 + g.below(14) as i32;
            for i in 0..n {
                if g.below(2) == 0 {
                    t.extend_from_slice(&[0, i + 1, i + 2]);
                } else {
                    t.extend_from_slice(&[i, i + 1, i + 2]);
                }
            }
            for _ in 0..g.below(4) {
                let a = g.below(n as u32 + 2) as i32;
                let b = (a + 1 + g.below(n as u32 + 1) as i32) % (n + 2);
                let c = (b + 1 + g.below(n as u32) as i32) % (n + 2);
                if a != b && b != c && a != c {
                    t.extend_from_slice(&[a, b, c]);
                }
            }
        }
    }
    t
}

fn strips(g: &mut Gen, count_max: u32, len_min: u32, len_max: u32, pool: u32) -> (Vec<i32>, i32) {
    let count = 1 + g.below(count_max) as i32;
    let mut v = Vec::new();
    for _ in 0..count {
        let len = len_min + g.below(len_max - len_min + 1);
        v.push(len as i32);
        for _ in 0..len {
            v.push(g.below(pool) as i32);
        }
    }
    (v, count)
}

#[test]
fn stripoptimizer_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_14_01);
    let mut c_strip = Check::new("StripOptimizerClass::Stripify");
    let mut c_comb = Check::new("StripOptimizerClass::Combine_Strips");
    let mut c_ostr = Check::new("StripOptimizerClass::Optimize_Strip_Order");
    let mut c_otri = Check::new("StripOptimizerClass::Optimize_Triangle_Order");
    let mut c_cnt = Check::new("StripOptimizerClass::Get_Strip_Index_Count");
    let mut c_is = Check::new("Insertion_Sort<int>");
    let mut c_qs = Check::new("Quick_Sort<int>");
    let mut c_ist = Check::new("Insertion_Sort<Tri>");
    let mut c_qst = Check::new("Quick_Sort<Tri>");
    for case in 0..cases() {
        // Stripify
        {
            let t = if case % 50 == 0 { Vec::new() } else { soup(&mut g) };
            let n = (t.len() / 3) as i32;
            let mut out = vec![0i32; (n as usize) * 5 + 16];
            let r = unsafe { ref_dpw_stripify(t.as_ptr(), n, out.as_mut_ptr(), out.len() as i32) };
            let cpp = if r < 0 { None } else { Some(out[..r as usize].to_vec()) };
            assert!(r != -2, "output buffer too small");
            c_strip.eq(so::stripify(&t, n), cpp, || format!("tris {t:?}"));
        }
        // Combine_Strips
        {
            let (s, count) = strips(&mut g, 8, 1, 12, 30);
            let mut out = vec![0i32; s.len() + 3 * count as usize + 8];
            let r = unsafe { ref_dpw_combine(s.as_ptr(), count, out.as_mut_ptr(), out.len() as i32) };
            c_comb.eq(so::combine_strips(&s, count), out[..r as usize].to_vec(), || format!("strips {s:?} count {count}"));
        }
        // Optimize_Strip_Order, Get_Strip_Index_Count
        {
            let pool = 4 + g.below(12);
            let (s, count) = strips(&mut g, 10, 0, 10, pool);
            let mut a = s.clone();
            let mut b = s.clone();
            so::optimize_strip_order(&mut a, count);
            unsafe { ref_dpw_opt_strips(b.as_mut_ptr(), count, b.len() as i32) };
            c_ostr.eq(a, b, || format!("strips {s:?} count {count}"));
            let cpp = unsafe { ref_dpw_index_count(s.as_ptr(), count) };
            c_cnt.eq(so::get_strip_index_count(&s, count), cpp, || format!("strips {s:?} count {count}"));
        }
        // Optimize_Triangle_Order
        {
            let pool = 3 + g.below(10);
            let n = 1 + g.below(30) as i32;
            let t: Vec<i32> = (0..n * 3).map(|_| g.below(pool) as i32).collect();
            let mut a = t.clone();
            let mut b = t.clone();
            so::optimize_triangle_order(&mut a, n);
            unsafe { ref_dpw_opt_tris(b.as_mut_ptr(), n) };
            c_otri.eq(a, b, || format!("tris {t:?}"));
        }
        // sorts
        {
            let n = g.below(71) as usize;
            let range = if g.below(2) == 0 { 4 } else { 1000 };
            let v: Vec<i32> = (0..n).map(|_| g.below(range) as i32).collect();
            let mut a = v.clone();
            let mut b = v.clone();
            so::insertion_sort(&mut a, n);
            unsafe { ref_dpw_insertion_sort(b.as_mut_ptr(), n as i32) };
            c_is.eq(a, b, || format!("{v:?}"));
            let mut a = v.clone();
            let mut b = v.clone();
            so::quick_sort(&mut a, n as i32);
            unsafe { ref_dpw_quick_sort(b.as_mut_ptr(), n as i32) };
            c_qs.eq(a, b, || format!("{v:?}"));
            // Tri: ordered by the first member; the other two record the original position
            let tri: Vec<i32> = (0..n).flat_map(|i| [g.below(range / 2 + 1) as i32, i as i32, (i * 7) as i32]).collect();
            let key = |t: &[i32]| -> Vec<Tk> { t.chunks(3).map(|c| Tk(c[0], c[1], c[2])).collect() };
            let back = |t: Vec<Tk>| -> Vec<i32> { t.into_iter().flat_map(|k| [k.0, k.1, k.2]).collect() };
            let mut a = key(&tri);
            let mut b = tri.clone();
            so::insertion_sort(&mut a, n);
            unsafe { ref_dpw_insertion_sort_tri(b.as_mut_ptr(), n as i32) };
            c_ist.eq(back(a), b, || format!("{tri:?}"));
            let mut a = key(&tri);
            let mut b = tri.clone();
            so::quick_sort(&mut a, n as i32);
            unsafe { ref_dpw_quick_sort_tri(b.as_mut_ptr(), n as i32) };
            c_qst.eq(back(a), b, || format!("{tri:?}"));
        }
    }
    finish(vec![c_strip, c_comb, c_ostr, c_cnt, c_otri, c_is, c_qs, c_ist, c_qst]);
}

/// `struct Tri` of stripoptimizer.cpp as a Rust type: ordered by its first member only (`operator<` and `operator>`).
#[derive(Clone, Copy)]
struct Tk(i32, i32, i32);

impl PartialEq for Tk {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}

impl PartialOrd for Tk {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&o.0)
    }
}
