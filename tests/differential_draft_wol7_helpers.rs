// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group wol7) of the small helpers the big WOL menu drafts share: `next_token_space` (`AsciiString::nextToken(&tok, " ")`) of
//! `src/drafts/ui/wol_quick_match_menu.rs`, against upstream's `AsciiString::nextToken` and its `skipSeps` / `skipNonSeps` (AsciiString.cpp), cut verbatim by build.rs
//! (`drafts_wol7`, `differential/wrap_draft_wol7_small.cpp`); and the `atoi` of `wol_lobby_menu.rs`, `wol_game_setup_menu.rs` and `wol_quick_match_menu.rs` (identical copies)
//! against the C library's `atoi` (what upstream calls; MSVC's and glibc's agree on the defined inputs: leading `isspace`, one sign, digits).
//!
//! # Method
//!
//! `DRAFT_CASES` cases (default 20,000; fixed seed): a text of 0 to 14 bytes drawn from space, tab, line feed, vertical tab, form feed, carriage return, `+`, `-`, digits and letters,
//! with runs of separators at the front, between and at the end. Both sides print the return value, the token and the rest. Excluded: a NUL byte (a C string) and, for `atoi`,
//! more than 18 digits (`strtol` saturates; the draft wraps like MSVC's).
//!
//! # Prediction (before the first run)
//!
//! `next_token_space`: 0 mismatches (a transcription of the same loops). `atoi`: one defect from reading, Rust's `is_ascii_whitespace` has no vertical tab (0x0B), C's `isspace` has.
//!
//! # Planted bugs (`L9_PLANT=<id>`): 24279 `atoi` skips only spaces; 24280 `atoi` ignores `-`; 24281 `atoi` rejects `+`; 24282 `atoi` stops after 9 characters; 24283 an all-separator text
//! is not cleared; 24284 leading separators are not skipped; 24285 a tab is a separator too; 24286 the rest starts at the token.

mod support_wol6;

use rts_lockstep_engine::drafts::ui::{wol_game_setup_menu as gs, wol_lobby_menu as lob, wol_quick_match_menu as qm};
use support_wol6::Rng;

extern "C" {
    fn ref_wol7_atoi(s: *const std::os::raw::c_char) -> i32;
    fn ref_wol7_next_token(text: *const std::os::raw::c_char, tok: *mut u8, rest: *mut u8, cap: i32) -> i32;
}

const ALPHABET: &[u8] = b"  \t\n\x0B\x0C\r+-0123456789abc  ";

fn gen_text(r: &mut Rng) -> Vec<u8> {
    let n = r.below(15) as usize;
    (0..n).map(|_| ALPHABET[r.below(ALPHABET.len() as u64) as usize]).collect()
}

fn cstr(b: &[u8]) -> std::ffi::CString {
    std::ffi::CString::new(b.to_vec()).expect("no NUL")
}

fn out_str(b: &[u8]) -> Vec<u8> {
    b.iter().take_while(|&&c| c != 0).copied().collect()
}

#[test]
fn the_wol_helpers_match_upstream() {
    let cases: usize = std::env::var("DRAFT_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut r = Rng(0x215_77_0002);
    let (mut atoi_bad, mut token_bad, mut tokens) = (0usize, 0usize, 0usize);
    for n in 0..cases {
        let t = gen_text(&mut r);
        let digits = t.iter().filter(|c| c.is_ascii_digit()).count();
        if digits <= 18 {
            let c = unsafe { ref_wol7_atoi(cstr(&t).as_ptr()) };
            let (a, b, d) = (lob::atoi(&t), gs::atoi(&t), qm::atoi(&t));
            if a != c || b != c || d != c {
                atoi_bad += 1;
                if atoi_bad <= 3 {
                    println!("atoi case {n}: {:?} c {c} lobby {a} game setup {b} quick match {d}", String::from_utf8_lossy(&t));
                }
            }
        }
        let mut tok = vec![0u8; 64];
        let mut rest = vec![0u8; 64];
        let cr = unsafe { ref_wol7_next_token(cstr(&t).as_ptr(), tok.as_mut_ptr(), rest.as_mut_ptr(), 64) };
        let mut data = t.clone();
        let rr = qm::next_token_space(&mut data);
        let rust = (rr.is_some(), rr.clone().unwrap_or_default(), data.clone());
        let cpp = (cr == 1, out_str(&tok), out_str(&rest));
        tokens += usize::from(cr == 1);
        if rust != cpp {
            token_bad += 1;
            if token_bad <= 3 {
                println!("token case {n}: {:?} rust {rust:?} c++ {cpp:?}", String::from_utf8_lossy(&t));
            }
        }
    }
    let mismatches = atoi_bad + token_bad;
    println!("DIFF WOL helpers compared {cases} mismatches {mismatches} (atoi {atoi_bad}, nextToken {token_bad}; {tokens} tokens found)");
    if planted {
        println!("MEASURED mismatches {mismatches}");
    }
    assert_eq!(mismatches, 0);
}
