// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui2) of the draft `src/drafts/sweep_common_client/credits.rs`
//! (`CreditsManagerLines::add_blank`, `parse_blank`, `CREDIT_STYLE_BLANK`) against upstream `Credits.cpp`'s
//! `CreditsManager::addBlank` and `parseBlank` and `Credits.h`'s style enum, cut verbatim by build.rs (`drafts_ui2`;
//! `differential/wrap_draft_credits.cpp`).
//!
//! # Method
//!
//! 5,000 sequences (fixed seed, `DRAFT_CASES` overrides) of 0 to 12 steps, each `addBlank()` or `parseBlank()`; after every step the
//! list's length, at the end every line's style, are compared with the C++'s. The draft's constant is also compared with the
//! C++ enum's `CREDIT_STYLE_BLANK` (through the style of a line added).
//!
//! The unit is two one-line functions, so there are only four plants (15474 to 15477), not five to ten.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches; all four plants caught in nearly every sequence that adds a line (15474, 15476, 15477 in all sequences
//! with a step; 15475 only in sequences of 4 or more steps).
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 15474 | `addBlank` gives the style `CREDIT_STYLE_COLUMN` (3) |
//! | 15475 | the fourth and later lines are put at the front with the style one past BLANK |
//! | 15476 | `parseBlank` does nothing |
//! | 15477 | `parseBlank` adds two lines |
//!
//! # Results (2026-11-19)
//!
//! 5,000 sequences: **0 mismatches** on the first run. Planted bugs (of 5,000): 15474 4,599; 15475 3,438; 15476 4,225; 15477 4,225. **4 of 4 caught** (15476 and 15477 need a `parseBlank` step, not just any step, as the prediction had it).
//! Mismatch before fixes: none to fix; no draft changed behaviour. The `DIFF` line is printed by the run.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::credits::*;
use support::{Check, Gen};

extern "C" {
    fn ref_u2_cr_run(input: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(5000)
}

fn finish(chk: Check) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    println!("DIFF {} compared {} mismatches {} (sequences)", chk.name, chk.compared, chk.mismatches);
    if planted {
        assert!(chk.mismatches > 0, "plant not caught");
    } else {
        chk.finish();
    }
}

#[test]
fn credits_blank_lines_match_upstream() {
    let mut g = Gen::new(0x215_2_03);
    let mut chk = Check::new("CreditsManager::addBlank / parseBlank");
    for _ in 0..cases() {
        let n = g.below(13) as usize;
        let steps: Vec<i32> = (0..n).map(|_| g.below(2) as i32).collect();
        let mut m = CreditsManagerLines::default();
        let mut rust = Vec::new();
        for &s in &steps {
            if s == 0 { m.add_blank() } else { m.parse_blank() }
            rust.push(m.credit_line_list.len() as i32);
        }
        rust.extend(m.credit_line_list.iter().map(|l| l.style));
        let mut out = vec![0i32; 64];
        let k = unsafe { ref_u2_cr_run(steps.as_ptr(), n as i32, out.as_mut_ptr(), 64) } as usize;
        out.truncate(k);
        chk.eq(rust, out, || format!("{steps:?}"));
    }
    finish(chk);
}
