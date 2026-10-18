// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::bezier_segment` (`BezierSegment`
//! and `BezFwdIterator`) against upstream `Bezier/BezierSegment.cpp` and
//! `Bezier/BezFwdIterator.cpp` (commit 0a05454d), compiled unchanged in
//! `differential/wrap_bezier.cpp` (see build.rs). Phase 1 layer 4.
//!
//! # Methodology
//!
//! Random cubic segments (control points of ordinary magnitude, signs, zeros
//! and special values from `support::Gen`), each compared bit for bit with
//! upstream for:
//!
//! - `evaluateBezSegmentAtT(t)`, at random `t` (in and beyond `[0, 1]`);
//! - `splitSegmentAtT(t)`, both halves;
//! - `getApproximateLength(tol)`, where Rust returns
//!   `BezierError::SubdivisionLimit` (upstream would recurse until the stack
//!   overflows) the input is counted and not sent to the C++;
//! - `getSegmentPoints(n)` for `n` from -2 to 40, where a negative count is
//!   an error on both sides (upstream throws `std::length_error`);
//! - the forward iterator `BezFwdIterator(n, seg)`, `start`, `done`, `next`.
//!
//! The D3DX operations (`D3DXVec4Transform`, `D3DXVec4Dot`) are a transcription
//! of the D3DX8 inline formulas (`differential/shim_bezier/D3DX8Math.h`), so
//! this test checks the translation against that transcription, not against
//! the d3dx8 library the shipped game linked.
//!
//! NaN sign and payload are not compared (see `canon` in the geometry test).
//!
//! **Planted bugs.** Each comparison is run against a deliberately wrong
//! variant of the Rust, which must be caught (`Check::finish_planted`).
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_bezier -- --nocapture` (the `DIFF` and `PLANTED` lines).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::bezier_segment::{BezFwdIterator, BezierError, BezierSegment};
use rts_lockstep_engine::differential as cpp;
use std::collections::BTreeMap;
use support::{Check, Gen};

/// NaN canonicalised (see the geometry test's `canon`).
fn canon(f: f32) -> u32 {
    if f.is_nan() {
        0x7FC0_0000
    } else {
        f.to_bits()
    }
}

fn bits_of(p: &[Coord3D]) -> Vec<u32> {
    p.iter().flat_map(|c| [canon(c.x), canon(c.y), canon(c.z)]).collect()
}

fn floats_of(cp: &[f32; 12]) -> BezierSegment {
    BezierSegment::from_floats(cp)
}

fn gen_real(g: &mut Gen) -> f32 {
    if g.below(10) < 8 {
        g.mixed()
    } else {
        g.finite()
    }
}

fn gen_cp(g: &mut Gen) -> [f32; 12] {
    let mut cp = [0f32; 12];
    for v in cp.iter_mut() {
        *v = gen_real(g);
    }
    cp
}

/// Coordinates of ordinary size, for the length and the iterator (where a
/// huge value would only make the recursion and the sums uninteresting).
fn gen_cp_modest(g: &mut Gen) -> [f32; 12] {
    let mut cp = [0f32; 12];
    for v in cp.iter_mut() {
        *v = if g.below(10) < 9 { g.uniform(-100.0, 100.0) } else { g.uniform(-1.0, 1.0) };
    }
    cp
}

fn gen_t(g: &mut Gen) -> f32 {
    if g.below(10) < 8 {
        g.uniform(-0.25, 1.25)
    } else {
        g.special()
    }
}

#[test]
fn bezier_evaluate_matches() {
    const CASES: u32 = 50_000;
    let mut g = Gen::new(0xBE2_0001);
    let mut ok = Check::new("BezierSegment::evaluateBezSegmentAtT");
    let mut bug = Check::new("BezierSegment::evaluateBezSegmentAtT (planted)");
    for case in 0..CASES {
        let cp = gen_cp(&mut g);
        let t = gen_t(&mut g);
        let input = || format!("case {case}: t {t:e}, cp {cp:?}");
        let rust = floats_of(&cp).evaluate_at_t(t);
        let mut out = [0f32; 3];
        unsafe { cpp::ref_bez_eval(cp.as_ptr(), t, out.as_mut_ptr()) };
        ok.eq(bits_of(&[rust]), out.iter().map(|&f| canon(f)).collect(), input);

        // Planted: control points 1 and 2 swapped before evaluating.
        let mut swapped = cp;
        for k in 0..3 {
            swapped.swap(3 + k, 6 + k);
        }
        let planted = floats_of(&swapped).evaluate_at_t(t);
        bug.eq(bits_of(&[planted]), out.iter().map(|&f| canon(f)).collect(), input);
    }
    ok.finish();
    bug.finish_planted();
}

#[test]
fn bezier_split_matches() {
    const CASES: u32 = 20_000;
    let mut g = Gen::new(0xBE2_0002);
    let mut ok = Check::new("BezierSegment::splitSegmentAtT");
    let mut bug = Check::new("BezierSegment::splitSegmentAtT (planted)");
    for case in 0..CASES {
        let cp = gen_cp(&mut g);
        let t = gen_t(&mut g);
        let input = || format!("case {case}: t {t:e}, cp {cp:?}");
        let (a, b) = floats_of(&cp).split_segment_at_t(t);
        let mut out = [0f32; 24];
        unsafe { cpp::ref_bez_split(cp.as_ptr(), t, out.as_mut_ptr()) };
        let rust: Vec<u32> = bits_of(a.control_points()).into_iter().chain(bits_of(b.control_points())).collect();
        let cpp_bits: Vec<u32> = out.iter().map(|&f| canon(f)).collect();
        ok.eq(rust, cpp_bits.clone(), input);

        // Planted: the second half keeps its own first point instead of the
        // evaluated split point.
        let mut wrong = bits_of(a.control_points());
        wrong.extend(bits_of(&floats_of(&cp).control_points_after_split_planted(t)));
        bug.eq(wrong, cpp_bits, input);
    }
    ok.finish();
    bug.finish_planted();
}

#[test]
fn bezier_length_matches() {
    const CASES: u32 = 20_000;
    let mut g = Gen::new(0xBE2_0003);
    let mut ok = Check::new("BezierSegment::getApproximateLength");
    let mut bug = Check::new("BezierSegment::getApproximateLength (planted)");
    let mut limit = 0u64;
    let mut tols: BTreeMap<String, u64> = BTreeMap::new();
    for case in 0..CASES {
        let cp = gen_cp_modest(&mut g);
        let tol = match g.below(6) {
            0 => 1.0,
            1 => g.uniform(0.001, 0.1),
            2 => g.uniform(0.1, 10.0),
            3 => 0.0,
            4 => g.uniform(0.01, 1.0),
            _ => g.uniform(-1.0, 1.0),
        };
        let input = || format!("case {case}: tol {tol:e}, cp {cp:?}");
        let rust = floats_of(&cp).get_approximate_length(tol);
        match rust {
            Err(BezierError::SubdivisionLimit) => {
                limit += 1;
                *tols.entry(format!("tol {tol:+.3}")).or_default() += 1;
                continue;
            }
            Err(e) => panic!("unexpected error {e:?}"),
            Ok(len) => {
                let c = unsafe { cpp::ref_bez_length(cp.as_ptr(), tol) };
                ok.eq(canon(len), canon(c), input);
                // Planted: the chord alone, not the average of chord and polygon.
                let chord = Coord3D { x: cp[9] - cp[0], y: cp[10] - cp[1], z: cp[11] - cp[2] }.length();
                bug.eq(canon(chord), canon(c), input);
            }
        }
    }
    println!(
        "length: {CASES} segments; {limit} left out (Rust reports SubdivisionLimit: upstream recurses without end; tolerances {:?})",
        tols.keys().take(4).collect::<Vec<_>>()
    );
    ok.finish();
    bug.finish_planted();
}

#[test]
fn bezier_points_and_iterator_match() {
    const CASES: u32 = 10_000;
    let mut g = Gen::new(0xBE2_0004);
    let mut ok = Check::new("BezierSegment::getSegmentPoints + BezFwdIterator");
    let mut bug = Check::new("BezierSegment::getSegmentPoints (planted)");
    let mut errors = 0u64;
    for case in 0..CASES {
        let cp = gen_cp_modest(&mut g);
        let n = g.below(43) as i32 - 2;
        let input = || format!("case {case}: n {n}, cp {cp:?}");
        let seg = floats_of(&cp);

        let mut pts = Vec::new();
        let rust = seg.get_segment_points(n, &mut pts);
        let mut cbuf = vec![0f32; 3 * (n.max(0) as usize) + 3];
        let mut count = 0i32;
        let status = unsafe { cpp::ref_bez_points(cp.as_ptr(), n, cbuf.as_mut_ptr(), cbuf.len() as i32, &mut count) };
        match rust {
            Err(BezierError::NegativeSegmentCount) => {
                errors += 1;
                ok.eq(true, status == 1, input);
            }
            Err(e) => panic!("unexpected error {e:?}"),
            Ok(()) => {
                ok.eq(status, 0, input);
                let cpp_pts: Vec<u32> = cbuf[..3 * count as usize].iter().map(|&f| canon(f)).collect();
                ok.eq(bits_of(&pts), cpp_pts.clone(), input);
                // Planted: one point too many (the iterator's `<=` for `<`).
                let mut more = bits_of(&pts);
                more.extend([0, 0, 0]);
                bug.eq(more, cpp_pts, input);
            }
        }

        // The iterator, by hand, against the same C++ points.
        let mut it = BezFwdIterator::new_with(n, &seg);
        it.start();
        let mut iter_pts = Vec::new();
        while !it.done() {
            iter_pts.push(*it.get_current());
            it.next();
        }
        let mut ibuf = vec![0f32; 3 * (n.max(0) as usize) + 3];
        unsafe { cpp::ref_bez_iter(cp.as_ptr(), n.max(0), ibuf.as_mut_ptr()) };
        let expected: Vec<u32> = ibuf[..3 * iter_pts.len()].iter().map(|&f| canon(f)).collect();
        ok.eq(bits_of(&iter_pts), expected, input);
    }
    println!("points: {CASES} (count, segment) cases; {errors} with a negative count (an error on both sides)");
    ok.finish();
    bug.finish_planted();
}

/// The Rust BezierSegment's `Coord3D` helper used by the planted split test.
trait PlantedSplit {
    fn control_points_after_split_planted(&self, t: f32) -> Vec<Coord3D>;
}

impl PlantedSplit for BezierSegment {
    /// The second half, with its first point taken from the original curve
    /// at `t = 0` (a slip in the split's end point).
    fn control_points_after_split_planted(&self, t: f32) -> Vec<Coord3D> {
        let (_, b) = self.split_segment_at_t(t);
        let mut v: Vec<Coord3D> = b.control_points().to_vec();
        v[0] = self.control_points()[0];
        v
    }
}

#[test]
fn bezier_length_rejects_nothing_on_a_straight_segment() {
    // A straight segment (collinear, evenly spaced control points) needs no
    // subdivision at any tolerance: its length is exact at the first level.
    let seg = BezierSegment::from_floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 3.0, 0.0, 0.0]);
    let len = seg.get_approximate_length(1.0e-6).expect("no subdivision needed");
    assert_eq!(len.to_bits(), 3.0f32.to_bits());
}
