// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 2 (`docs/phase1-porting-graph.md`):
//! the `AsciiString` comparison functors of `Common/STLTypedefs.h`,
//! `rts::less_than_nocase<AsciiString>` and `rts::equal_to<AsciiString>`
//! (`common::stl_typedefs`), upstream commit 0a05454d.
//!
//! References: the unchanged `STLTypedefs.h`, instantiated by
//! `differential/wrap_stl_typedefs.cpp`, with the real `AsciiString.h` (built
//! as in the AsciiString build, see build.rs).
//!
//! Inputs: 50,000 pairs of random ASCII strings (0 to 6 characters from a
//! set with both cases, digits and punctuation), each made three ways: no
//! buffer, from the text, and empty but keeping its buffer (`removeLastChar`).
//! A third of the pairs are the same text in random case, so that
//! case-only differences are tested, not left to chance. Outputs: the
//! two functors in both directions, and upstream's `operator==`.
//!
//! What is measured and not gated: upstream's `operator==` compares the
//! characters, so a string with no buffer equals an empty string that keeps
//! one. The derived `PartialEq` of `AsciiString` does not, which is recorded
//! here (the count of disagreements) and in `common::stl_typedefs`. The
//! functors are gated; `NoCaseAsciiKey`, the map key, is gated against the
//! `less_than_nocase` functor (order and std::map equivalence).
//!
//! **Planted bugs**, which must be caught: a case-sensitive order, and an
//! `equal_to` that ignores case.
//!
//! The `UnicodeString` comparators (`less_than_nocase<UnicodeString>` and
//! `equal_to<UnicodeString>`, `common::stl_typedefs::*_unicode`) are the same
//! test on the same reference header, built as its own C++ wrapper
//! (`differential/wrap_stl_typedefs_unicode.cpp`, with the real
//! `UnicodeString.cpp`). Inputs: 50,000 pairs of 0 to 6 units from the ASCII
//! set above, plus non-ASCII units (0x80 to 0xffff), each made three ways as
//! for `AsciiString`. Excluded as undefined upstream: **0 pairs**. The
//! comparators call only `_wcsicmp` and `wcscmp`, and the module doc of
//! `common::unicode_string` does not list either as undefined for any unit
//! (its undefined cases are `iswspace` above 0x80, `getCharAt` past the end,
//! and the length limit, none of which these comparators reach). Planted:
//! a case-sensitive order and a case-blind equality, on units.
//!
//! **Results** (2026-10-18, release, `--features differential -- --nocapture`):
//! - `less_than_nocase`: 100,000 comparisons (both directions of 50,000
//!   pairs), 0 mismatches; the case-sensitive planted order caught in 2,039.
//! - `equal_to`: 50,000 comparisons, 0 mismatches; the case-blind planted
//!   equality caught in 3,210.
//! - `NoCaseAsciiKey` (order and equivalence): 100,000 comparisons, 0 mismatches.
//! - Measured, not gated: upstream `operator==` against the `PartialEq` of
//!   `AsciiString`. Before commit 28ce3e9 they disagreed on 12,388 of 50,000
//!   pairs (a string with no buffer against an empty string with one). Since
//!   that commit, 0 of 50,000 (2026-10-18 re-run). This note was stale until
//!   this change; the measurement line is still printed by the test.
//! - `UnicodeString` (`stl_unicode_comparators_match`, 2026-10-18): `less_than_nocase`
//!   100,000 comparisons, 0 mismatches, the case-sensitive planted order caught
//!   in 1,482; `equal_to` (operator==) 50,000 comparisons, 0 mismatches, the
//!   case-blind planted equality caught in 2,547; 0 of 50,000 pairs excluded.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::stl_typedefs::{
    equal_to, equal_to_unicode, less_than_nocase, less_than_nocase_unicode, NoCaseAsciiKey,
};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::differential as cpp;
use std::cmp::Ordering;
use std::ffi::CString;
use support::{Check, Gen};

const PAIRS: usize = 50_000;
const ALPHABET: &[u8] = b"aAbBcCxyzXYZ09 _-~[{";

/// How a string is made: 0 no buffer, 1 from its text, 2 empty with a buffer.
#[derive(Clone, Debug)]
struct Sample {
    text: Vec<u8>,
    kind: i32,
}

fn random_sample(g: &mut Gen) -> Sample {
    let len = g.below(7) as usize;
    let text = (0..len).map(|_| ALPHABET[g.below(ALPHABET.len() as u32) as usize]).collect();
    Sample { text, kind: g.below(3) as i32 }
}

fn make(s: &Sample) -> AsciiString {
    match s.kind {
        0 => AsciiString::new(),
        1 => AsciiString::try_from_bytes(&s.text).expect("short ASCII text"),
        _ => {
            // As the C++ wrapper: AsciiString("x"), then removeLastChar().
            let mut a = AsciiString::try_from_bytes(b"x").expect("short text");
            a.remove_last_char().expect("one character");
            a
        }
    }
}

/// The four results of `ref_stl_compare`: less(a,b), equal_to(a,b),
/// operator==(a,b), less(b,a).
fn cpp_compare(a: &Sample, b: &Sample) -> [i32; 4] {
    // The C++ side takes a C string; the samples are NUL-free by construction.
    let ca = CString::new(a.text.clone()).expect("no NUL");
    let cb = CString::new(b.text.clone()).expect("no NUL");
    let mut out = [0i32; 4];
    unsafe { cpp::ref_stl_compare(ca.as_ptr(), a.kind, cb.as_ptr(), b.kind, out.as_mut_ptr()) };
    out
}

/// The planted order: case-sensitive, which the reference would reject.
fn case_sensitive_less(a: &AsciiString, b: &AsciiString) -> bool {
    a.str().cmp(b.str()) == Ordering::Less
}

/// The planted equality: ignores case, which the reference would reject.
fn case_blind_equal(a: &AsciiString, b: &AsciiString) -> bool {
    a.str().eq_ignore_ascii_case(b.str())
}

#[test]
fn stl_comparators_match() {
    let mut g = Gen::new(303);
    let mut less = Check::new("rts::less_than_nocase<AsciiString>");
    let mut equal = Check::new("rts::equal_to<AsciiString>");
    let mut key = Check::new("NoCaseAsciiKey (std::map equivalence)");
    let mut less_bug = Check::new("rts::less_than_nocase<AsciiString>");
    let mut equal_bug = Check::new("rts::equal_to<AsciiString>");
    let mut operator_eq = Check::new("AsciiString operator== (measured only)");
    for _ in 0..PAIRS {
        let a = random_sample(&mut g);
        // A third of the pairs are the same text in other case (or equal),
        // so that case-only differences are common enough to be tested.
        let b = if a.kind == 1 && g.below(3) == 0 {
            let text = a
                .text
                .iter()
                .map(|&c| if g.below(2) == 0 { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() })
                .collect();
            Sample { text, kind: 1 }
        } else {
            random_sample(&mut g)
        };
        let input = || format!("a = {:?} (kind {}), b = {:?} (kind {})", String::from_utf8_lossy(&a.text), a.kind, String::from_utf8_lossy(&b.text), b.kind);
        let r = cpp_compare(&a, &b);
        let (sa, sb) = (make(&a), make(&b));

        less.eq(less_than_nocase(&sa, &sb), r[0] == 1, input);
        less.eq(less_than_nocase(&sb, &sa), r[3] == 1, input);
        equal.eq(equal_to(&sa, &sb), r[1] == 1, input);

        // std::map equivalence: neither key is less than the other.
        let (ka, kb) = (NoCaseAsciiKey(sa.clone()), NoCaseAsciiKey(sb.clone()));
        key.eq(ka < kb, r[0] == 1, input);
        key.eq(ka == kb, r[0] == 0 && r[3] == 0, input);

        less_bug.eq(case_sensitive_less(&sa, &sb), r[0] == 1, input);
        equal_bug.eq(case_blind_equal(&sa, &sb), r[1] == 1, input);

        // Measured, not gated: the derived PartialEq sees no-buffer and empty-with-buffer as different.
        operator_eq.eq(sa == sb, r[2] == 1, input);
    }
    less.finish();
    equal.finish();
    key.finish();
    less_bug.finish_planted();
    equal_bug.finish_planted();
    operator_eq.report();
}

// ---------------------------------------------------------------------------
// UnicodeString
// ---------------------------------------------------------------------------

/// The units of `ALPHABET` as `u16`, and units from 0x80 up: `_wcsicmp`
/// folds `A`..`Z` only (C locale), and `wcscmp` orders units as unsigned.
const UNICODE_ALPHABET: &[u16] = &[
    0x61, 0x41, 0x62, 0x42, 0x63, 0x43, 0x78, 0x79, 0x7a, 0x58, 0x59, 0x5a, 0x30, 0x39, 0x20, 0x5f, 0x2d, 0x7e, 0x5b, 0x7b,
    0x80, 0xc0, 0xe9, 0x130, 0x2000, 0xff21, 0xffff,
];

fn random_unicode_sample(g: &mut Gen) -> (Vec<u16>, i32) {
    let len = g.below(7) as usize;
    let units = (0..len).map(|_| UNICODE_ALPHABET[g.below(UNICODE_ALPHABET.len() as u32) as usize]).collect();
    (units, g.below(3) as i32)
}

fn make_unicode(units: &[u16], kind: i32) -> UnicodeString {
    match kind {
        0 => UnicodeString::new(),
        1 => UnicodeString::try_from_chars(units).expect("short string"),
        _ => {
            // As the C++ wrapper: UnicodeString(L"x"), then removeLastChar().
            let mut u = UnicodeString::try_from_chars(&[0x78]).expect("short text");
            u.remove_last_char().expect("one unit");
            u
        }
    }
}

/// `_wcsicmp`'s fold: `A`..`Z` to lower case, nothing else (C locale).
fn fold_unit(c: u16) -> u16 {
    if (0x41..=0x5a).contains(&c) {
        c + 0x20
    } else {
        c
    }
}

/// The other case of an ASCII letter; any other unit unchanged.
fn swap_ascii_case(c: u16) -> u16 {
    match c {
        0x41..=0x5a => c + 0x20,
        0x61..=0x7a => c - 0x20,
        _ => c,
    }
}

/// The planted order: case-sensitive, which the reference would reject.
fn case_sensitive_less_unicode(a: &UnicodeString, b: &UnicodeString) -> bool {
    a.str().cmp(b.str()) == Ordering::Less
}

/// The planted equality: ignores case, which the reference would reject.
fn case_blind_equal_unicode(a: &UnicodeString, b: &UnicodeString) -> bool {
    a.str().iter().map(|&c| fold_unit(c)).eq(b.str().iter().map(|&c| fold_unit(c)))
}

#[test]
fn stl_unicode_comparators_match() {
    let mut g = Gen::new(304);
    let mut less = Check::new("rts::less_than_nocase<UnicodeString>");
    let mut equal = Check::new("rts::equal_to<UnicodeString> (operator==)");
    let mut less_bug = Check::new("rts::less_than_nocase<UnicodeString>");
    let mut equal_bug = Check::new("rts::equal_to<UnicodeString>");
    for _ in 0..PAIRS {
        let (ua, ka) = random_unicode_sample(&mut g);
        // A third of the pairs with text are the same text in other case, so
        // that case-only differences are tested, as for AsciiString.
        let (ub, kb) = if ka == 1 && g.below(3) == 0 {
            let units = ua.iter().map(|&c| if g.below(2) == 0 { swap_ascii_case(c) } else { c }).collect();
            (units, 1)
        } else {
            random_unicode_sample(&mut g)
        };
        let input = || format!("a = {:?} (kind {ka}), b = {:?} (kind {kb})", ua, ub);
        let mut out = [0i32; 3];
        let (na, nb) = (ua.len() as i32, ub.len() as i32);
        unsafe {
            cpp::ref_stl_compare_unicode(ua.as_ptr(), na, ka, ub.as_ptr(), nb, kb, out.as_mut_ptr());
        }
        let (sa, sb) = (make_unicode(&ua, ka), make_unicode(&ub, kb));

        less.eq(less_than_nocase_unicode(&sa, &sb), out[0] == 1, input);
        less.eq(less_than_nocase_unicode(&sb, &sa), out[2] == 1, input);
        equal.eq(equal_to_unicode(&sa, &sb), out[1] == 1, input);

        less_bug.eq(case_sensitive_less_unicode(&sa, &sb), out[0] == 1, input);
        equal_bug.eq(case_blind_equal_unicode(&sa, &sb), out[1] == 1, input);
    }
    less.finish();
    equal.finish();
    less_bug.finish_planted();
    equal_bug.finish_planted();
    println!("excluded (undefined upstream): 0 of {PAIRS} pairs (no input is undefined for these comparators)");
}
