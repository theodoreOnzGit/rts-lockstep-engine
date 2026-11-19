// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui2) of the draft `src/drafts/sweep_common_client/game_text.rs`
//! (`remove_leading_and_trailing`) against upstream `GameText.cpp`'s `GameTextManager::removeLeadingAndTrailing`, cut
//! verbatim by build.rs (`drafts_ui2`; `differential/wrap_draft_game_text.cpp` lists the cut and the stand-in).
//!
//! # Method
//!
//! 20,000 strings (fixed seed, `DRAFT_CASES` overrides), each 0 to 24 characters drawn from white space (space, `\t`, `\n`, `\v`,
//! `\f`, `\r`), letters, a quote, a digit, DEL (0x7f) and sometimes an embedded NUL; runs of leading and trailing
//! white space, white-space-only strings, empty and one-character strings are generated on purpose. Each string is
//! trimmed by the C++ (a `char` buffer, `iswspace`) and by the Rust (`Vec<u16>`); the resulting strings are compared.
//!
//! **Excluded input (stated, not hidden):** characters 0x80 to 0xFF. Upstream's `Char` is signed, and `iswspace` on a
//! negative `char` is undefined in C; the draft's `iswspace` is the C locale's (0x09..0x0d and 0x20) on `u16`.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches over 20,000 strings: the draft is a line-by-line reading of the C++ and the only subtlety (the
//! first character is never stripped from the end; a space-only string empties in the leading pass) is handled.
//! Plants 15461 to 15465 all caught; 15464 (the trailing walk stops one early) the fewest, needing a text of 2 or more
//! characters with trailing white space.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`:
//!
//! | id | planted bug |
//! |---|---|
//! | 15461 | the leading strip skips only the space character |
//! | 15462 | the trailing strip skips only the space character |
//! | 15463 | `iswspace` misses `\r` (0x0d) |
//! | 15464 | the trailing walk stops one character early (`end > 2`) |
//! | 15465 | the leading strip stops one white-space character before the first non-space (`first + 1 < len`) |
//!
//! # Results (2026-11-19)
//!
//! 20,000 strings: **0 mismatches** on the first run, as predicted. Planted bugs (mismatching strings of 20,000): 15461 8,854; 15462 3,838; 15463 4,993; 15464 556; 15465 5,578. **5 of 5 caught**; 15464 the fewest, as predicted.
//! Mismatch before fixes: none to fix; no draft changed behaviour. The `DIFF` line is printed by the run.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::game_text::remove_leading_and_trailing;
use support::{Check, Gen};

extern "C" {
    fn ref_u2_gt_trim(input: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
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
fn remove_leading_and_trailing_matches_upstream() {
    let mut g = Gen::new(0x215_2_01);
    let mut chk = Check::new("GameTextManager::removeLeadingAndTrailing");
    const SPACES: [i32; 6] = [0x20, 0x09, 0x0a, 0x0b, 0x0c, 0x0d];
    const OTHERS: [i32; 8] = [b'a' as i32, b'Z' as i32, b'"' as i32, b'7' as i32, 0x7f, b'-' as i32, 0x01, b'x' as i32];
    for _ in 0..cases() {
        let mode = g.below(5);
        let len = match g.below(6) {
            0 => 0,
            1 => 1,
            _ => g.below(25) as usize,
        };
        let mut s: Vec<i32> = Vec::new();
        for i in 0..len {
            let c = match mode {
                0 => SPACES[g.below(6) as usize], // white space only
                1 => {
                    // spaces at the ends, text inside
                    if i < 3 || i + 3 >= len {
                        SPACES[g.below(6) as usize]
                    } else {
                        OTHERS[g.below(8) as usize]
                    }
                }
                2 => {
                    // mostly plain spaces
                    if g.below(3) == 0 { OTHERS[g.below(8) as usize] } else { 0x20 }
                }
                _ => {
                    if g.below(2) == 0 { SPACES[g.below(6) as usize] } else { OTHERS[g.below(8) as usize] }
                }
            };
            s.push(c);
        }
        if g.below(12) == 0 && !s.is_empty() {
            let at = g.below(s.len() as u32) as usize;
            s[at] = 0;
        }
        let mut out = vec![0i32; 64];
        let n = unsafe { ref_u2_gt_trim(s.as_ptr(), s.len() as i32, out.as_mut_ptr(), 64) } as usize;
        let cpp: Vec<u16> = out[..n].iter().map(|&c| c as u16).collect();
        let mut buf: Vec<u16> = s.iter().map(|&c| c as u16).collect();
        remove_leading_and_trailing(&mut buf);
        chk.eq(buf, cpp, || format!("{s:?}"));
    }
    finish(chk);
}
