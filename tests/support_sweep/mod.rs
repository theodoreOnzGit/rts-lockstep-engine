// SPDX-License-Identifier: GPL-3.0-only
//! Helpers shared by the differential tests of group sweepww3d (#215): the planted-run summary and the word-wise comparison.
#![allow(dead_code)]

use super::support::Check;

/// Print the summary lines; fail on a mismatch in a normal run; in a planted run (`L9_PLANT` set) print how many comparisons
/// differed and fail if none did.
pub fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in &chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        bad += c.mismatches;
    }
    if planted {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    } else {
        for c in chk {
            c.finish();
        }
    }
}

/// Word-wise comparison where a pair of NaNs with different sign or payload bits is counted apart (tests/support/mod.rs rule).
pub fn feq(c: &mut Check, rust: Vec<u32>, cpp: Vec<u32>, what: impl FnOnce() -> String) {
    let nan = |w: u32| f32::from_bits(w).is_nan();
    let same = rust.len() == cpp.len() && rust.iter().zip(&cpp).all(|(a, b)| a == b || (nan(*a) && nan(*b)));
    if same && rust != cpp {
        c.compared += 1;
        c.nan_only += 1;
    } else {
        c.eq(rust, cpp, what);
    }
}

pub fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

pub fn b(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}
