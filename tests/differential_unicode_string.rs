// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::unicode_string` against upstream
//! `GameEngine/Source/Common/System/UnicodeString.cpp` and
//! `Include/Common/UnicodeString.h` (commit 0a05454d), compiled unchanged
//! (`differential/wrap_unicode_string.cpp`) with a 16-bit `WideChar` and
//! 16-bit stand-ins for the MSVC wide C runtime (`wcslen`, `wcscpy`,
//! `wcscat`, `wcscmp`, `_wcsicmp`, `wcsspn`, `wcscspn`, `iswspace`; each
//! documented in `differential/shim_unicode/unicode_prelude.h`).
//!
//! Method: 5,000 random scripts of up to 100 operations on four strings:
//! `set`, `concat` (string, character and `UnicodeString`), `trim`,
//! `removeLastChar`, `translate` (from an `AsciiString`), `nextToken` (the
//! default delimiters, a literal, and a copy of any of the four strings, which
//! may be the string itself or the token), `clear`, `operator=`,
//! `set(const UnicodeString&)`, the constructor, a `set` from the string's own
//! tail (the overlapping `wcscpy` that `trim` and `nextToken` also make), and
//! checks of everything observable: whether a buffer exists, `getLength`,
//! `isEmpty`, `str()`, `getCharAt` at every defined index, the signs of the
//! four `compare`/`compareNoCase` forms and the six comparison operators.
//! Inputs mix whitespace (including `\v` and `\f`), mixed case, `_` and
//! `` ` `` (between the upper- and lower-case letters, so folding to lower
//! case is told from folding to upper), delimiters, embedded 0 units (C
//! strings end there), units from 0x80 (including 0xffff, to check unsigned
//! ordering), and strings near the 16,380-unit limit where upstream throws
//! `ERROR_OUT_OF_MEMORY`, which the logs compare (also in the middle of
//! `translate`, which keeps the units appended so far). Operations upstream
//! leaves undefined or whose MSVC result is not established (`iswspace` of a
//! unit from 0x80 in `trim`, `getCharAt` past the terminator, a tail `set`
//! past the string) are not sent to the C++; the Rust must return an error
//! there, and they are counted. `concat` of a string onto itself is not
//! generated (undefined when the buffer has room; see the module docs). The
//! logs must be identical byte for byte.
//!
//! Planted bugs, each of which must be caught: `removeLastChar` releasing an
//! emptied buffer, a case-sensitive `compareNoCase`, `compareNoCase` folding
//! to upper case, and `translate` zero-extending instead of sign-extending.
//!
//! Results: see the `ok.finish()` lines of a run (`cargo test --release
//! --features differential --test differential_unicode_string -- --nocapture`).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::unicode_string::{
    UnicodeString, UnicodeStringError, THE_EMPTY_STRING,
};
use rts_lockstep_engine::differential as cpp;
use std::cmp::Ordering;
use support::{Check, Gen};

#[derive(Clone, Debug)]
enum Delim {
    Default,
    Literal(Vec<u16>),
    Slot(u8),
}

#[derive(Clone, Debug)]
enum Op {
    Set(u8, Vec<u16>),
    Concat(u8, Vec<u16>),
    ConcatChar(u8, u16),
    Trim(u8),
    RemoveLast(u8),
    Translate(u8, Vec<u8>),
    NextToken(u8, u8, Delim),
    Clear(u8),
    Assign(u8, u8),
    SetU(u8, u8),
    Dump(u8),
    CharAt(u8, i32),
    Compare(u8, u8, Vec<u16>),
    ConcatU(u8, u8),
    Construct(u8, Vec<u16>),
    SetTail(u8, i32),
}

fn encode(ops: &[Op]) -> Vec<u8> {
    let mut s = Vec::new();
    let lit = |s: &mut Vec<u8>, w: &[u16]| {
        s.extend_from_slice(&(w.len() as u16).to_le_bytes());
        for c in w {
            s.extend_from_slice(&c.to_le_bytes());
        }
    };
    for op in ops {
        match op {
            Op::Set(a, w) => {
                s.extend_from_slice(&[0, *a]);
                lit(&mut s, w);
            }
            Op::Concat(a, w) => {
                s.extend_from_slice(&[1, *a]);
                lit(&mut s, w);
            }
            Op::ConcatChar(a, c) => {
                s.extend_from_slice(&[2, *a]);
                s.extend_from_slice(&c.to_le_bytes());
            }
            Op::Trim(a) => s.extend_from_slice(&[3, *a]),
            Op::RemoveLast(a) => s.extend_from_slice(&[4, *a]),
            Op::Translate(a, b) => {
                s.extend_from_slice(&[5, *a]);
                s.extend_from_slice(&(b.len() as u16).to_le_bytes());
                s.extend_from_slice(b);
            }
            Op::NextToken(a, t, d) => {
                s.extend_from_slice(&[6, *a, *t]);
                match d {
                    Delim::Default => s.push(0),
                    Delim::Literal(w) => {
                        s.push(1);
                        lit(&mut s, w);
                    }
                    Delim::Slot(k) => s.extend_from_slice(&[2, *k]),
                }
            }
            Op::Clear(a) => s.extend_from_slice(&[7, *a]),
            Op::Assign(a, t) => s.extend_from_slice(&[8, *a, *t]),
            Op::SetU(a, t) => s.extend_from_slice(&[9, *a, *t]),
            Op::Dump(a) => s.extend_from_slice(&[10, *a]),
            Op::CharAt(a, i) => {
                s.extend_from_slice(&[11, *a]);
                s.extend_from_slice(&i.to_le_bytes());
            }
            Op::Compare(a, t, w) => {
                s.extend_from_slice(&[12, *a, *t]);
                lit(&mut s, w);
            }
            Op::ConcatU(a, t) => s.extend_from_slice(&[13, *a, *t]),
            Op::Construct(a, w) => {
                s.extend_from_slice(&[14, *a]);
                lit(&mut s, w);
            }
            Op::SetTail(a, k) => {
                s.extend_from_slice(&[15, *a]);
                s.extend_from_slice(&k.to_le_bytes());
            }
        }
    }
    s
}

/// Operations that threw `ERROR_OUT_OF_MEMORY` (counted on the Rust side).
static TOO_LONG: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq)]
enum Bug {
    None,
    RemoveLastReleases,
    CaseSensitiveNoCase,
    FoldsToUpper,
    TranslateZeroExtends,
}

/// `s.set(s.str() + k)`: defined only with a buffer and `0 <= k <= getLength()`
/// (without one, only `k == 0`); `k == 0` is the `s != peek()` no-op.
fn set_tail(s: &mut UnicodeString, k: i32) -> Result<(), UnicodeStringError> {
    if k < 0 || k as usize > s.str().len() || (k > 0 && !s.has_buffer()) {
        return Err(UnicodeStringError::IndexOutOfRange);
    }
    if k == 0 {
        return Ok(());
    }
    let tail = s.str()[k as usize..].to_vec();
    s.set(&tail)
}

fn fold_upper(c: u16) -> u16 {
    if (b'a' as u16..=b'z' as u16).contains(&c) {
        c - 32
    } else {
        c
    }
}

/// One operation of the Rust run, appending to the wrapper's log format;
/// `None` if it is undefined upstream (the Rust returned the error).
fn step(slot: &mut [UnicodeString; 4], op: &Op, bug: Bug, lg: &mut Vec<u8>) -> Option<()> {
    let ord = |o: Ordering| o as i8 as u8;
    let at = lg.len();
    lg.push(0);
    let r: Result<(), UnicodeStringError> = (|| {
        match op {
            Op::Set(a, w) => slot[*a as usize].set(w)?,
            Op::Concat(a, w) => slot[*a as usize].concat(w)?,
            Op::ConcatChar(a, c) => slot[*a as usize].concat_char(*c)?,
            Op::Trim(a) => slot[*a as usize].trim()?,
            Op::RemoveLast(a) => {
                let s = &mut slot[*a as usize];
                s.remove_last_char()?;
                if bug == Bug::RemoveLastReleases && s.is_empty() {
                    s.clear();
                }
            }
            Op::Translate(a, b) => {
                let src = AsciiString::try_from_bytes(b).expect("test strings fit an AsciiString");
                if bug == Bug::TranslateZeroExtends {
                    slot[*a as usize].clear();
                    for &c in src.str() {
                        slot[*a as usize].concat_char(c as u16)?;
                    }
                } else {
                    slot[*a as usize].translate(&src)?;
                }
            }
            Op::NextToken(a, t, d) => {
                let delims = match d {
                    Delim::Default => THE_EMPTY_STRING.clone(),
                    Delim::Literal(w) => UnicodeString::try_from_chars(w)?,
                    Delim::Slot(k) => slot[*k as usize].clone(),
                };
                let r = if a == t {
                    // `tok == this`: upstream returns false, changing nothing
                    false
                } else {
                    let mut tok = std::mem::take(&mut slot[*t as usize]);
                    let r = slot[*a as usize].next_token(&mut tok, &delims);
                    slot[*t as usize] = tok;
                    r?
                };
                lg.push(u8::from(r));
            }
            Op::Clear(a) => slot[*a as usize].clear(),
            Op::Assign(a, t) => slot[*a as usize] = slot[*t as usize].clone(),
            Op::SetU(a, t) => {
                let src = slot[*t as usize].clone();
                slot[*a as usize].set_unicode(&src);
            }
            Op::Dump(a) => {
                let s = &slot[*a as usize];
                lg.push(u8::from(s.has_buffer()));
                lg.extend_from_slice(&s.get_length().to_le_bytes());
                lg.push(u8::from(s.is_empty()));
                lg.extend_from_slice(&(s.str().len() as i32).to_le_bytes());
                for c in s.str() {
                    lg.extend_from_slice(&c.to_le_bytes());
                }
            }
            Op::CharAt(a, i) => {
                let c = slot[*a as usize].try_get_char_at(*i)?;
                lg.extend_from_slice(&c.to_le_bytes());
            }
            Op::Compare(a, t, p) => {
                let (s, b) = (&slot[*a as usize], &slot[*t as usize]);
                let nocase = |x: &UnicodeString, y: &[u16]| match bug {
                    Bug::CaseSensitiveNoCase => x.compare(y),
                    Bug::FoldsToUpper => {
                        let u = x.str().iter().copied().map(fold_upper);
                        let v = y.iter().copied().take_while(|&c| c != 0).map(fold_upper);
                        u.cmp(v)
                    }
                    _ => x.compare_no_case(y),
                };
                lg.extend_from_slice(&[
                    ord(s.compare(p)),
                    ord(nocase(s, p)),
                    ord(s.compare_unicode(b)),
                    ord(nocase(s, b.str())),
                ]);
                lg.extend_from_slice(&[
                    u8::from(s == b),
                    u8::from(s != b),
                    u8::from(s < b),
                    u8::from(s <= b),
                    u8::from(s > b),
                    u8::from(s >= b),
                ]);
            }
            Op::ConcatU(a, t) => {
                assert_ne!(a, t);
                let src = slot[*t as usize].clone();
                slot[*a as usize].concat_unicode(&src)?;
            }
            Op::Construct(a, w) => slot[*a as usize] = UnicodeString::try_from_chars(w)?,
            Op::SetTail(a, k) => set_tail(&mut slot[*a as usize], *k)?,
        }
        Ok(())
    })();
    match r {
        Ok(()) => Some(()),
        Err(UnicodeStringError::TooLong) => {
            lg.truncate(at + 1);
            lg[at] = 1;
            if bug == Bug::None {
                TOO_LONG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            Some(())
        }
        Err(_) => {
            lg.truncate(at);
            None
        }
    }
}

/// The Rust run of a whole script (only defined operations). A planted bug
/// can change the state so that a later operation becomes undefined: that
/// is logged as a 0xff byte (a difference, as it should be).
fn run_rust(ops: &[Op], bug: Bug) -> Vec<u8> {
    let mut slot: [UnicodeString; 4] = Default::default();
    let mut lg = Vec::new();
    for op in ops {
        if step(&mut slot, op, bug, &mut lg).is_none() {
            assert!(bug != Bug::None, "an operation kept as defined failed");
            lg.push(0xff);
        }
    }
    lg
}

fn run_cpp(ops: &[Op]) -> Vec<u8> {
    let script = encode(ops);
    let mut log = vec![0u8; 64 * 1024 * 1024];
    let n = unsafe {
        cpp::ref_unicode_run(
            script.as_ptr(),
            script.len() as i32,
            log.as_mut_ptr(),
            log.len() as i32,
        )
    };
    assert!(n >= 0, "C++ log overflow");
    log.truncate(n as usize);
    log
}

fn units(g: &mut Gen) -> Vec<u16> {
    let pick = |g: &mut Gen| -> u16 {
        match g.below(20) {
            0 => 0x20,
            1 => 0x09,
            2 => 0x0a,
            3 => 0x0b,
            4 => 0x0d,
            5 => 0x0c,
            6 => b',' as u16,
            7 => 0,
            8 => [0x80, 0xa0, 0xe9, 0x100, 0x2003, 0x7fff, 0x8000, 0xffff][g.below(8) as usize],
            9 => b'_' as u16,
            10 => b'`' as u16,
            11..=13 => b'A' as u16 + g.below(26) as u16,
            _ => b'a' as u16 + g.below(26) as u16,
        }
    };
    let w = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    // (the two near-limit shapes are rare: they make a script 100x slower)
    match g.below(40) {
        0 => w("None"),
        1 => w("  Alpha Beta \t"),
        2 => Vec::new(),
        3 => vec![b'x' as u16; 16379 + g.below(4) as usize],
        4 => {
            // a long string with a space or tab in it: tokens near the limit
            let mut v = vec![b'z' as u16; 16377 + g.below(5) as usize];
            let at = g.below(v.len() as u32) as usize;
            v[at] = 0x20;
            v
        }
        5 => format!("  Alpha{}Beta  ", ["", " ", ",", "\t"][g.below(4) as usize])
            .encode_utf16()
            .collect(),
        6 => w(["a b\tc\nd\re", "AbC", "abc", "ABC", "a_c", "a`c"][g.below(6) as usize]),
        _ => {
            let n = g.below(14) as usize;
            let mut v: Vec<u16> = (0..n).map(|_| pick(g)).collect();
            if g.below(2) == 0 {
                v.retain(|&c| c < 0x80);
            }
            v
        }
    }
}

fn ascii_bytes(g: &mut Gen) -> Vec<u8> {
    // (a long one is rare: the C++ reallocates and copies the whole string
    // for every character translate appends, quadratic)
    match g.below(60) {
        0 => vec![b'q'; 16379 + g.below(5) as usize],
        1..=7 => Vec::new(),
        8..=14 => vec![0xe9, 0x80, b'A', 0xff],
        _ => (0..g.below(12))
            .map(|_| [b'a', b'Z', b' ', 0, 0x80, 0xe9, 0xff, b'_'][g.below(8) as usize])
            .collect(),
    }
}

#[test]
fn unicode_string_matches_cpp() {
    let mut g = Gen::new(601);
    let mut ok = Check::new("UnicodeString script logs");
    let mut planted = [
        (
            Bug::RemoveLastReleases,
            Check::new("UnicodeString (removeLastChar releases)"),
        ),
        (
            Bug::CaseSensitiveNoCase,
            Check::new("UnicodeString (case-sensitive compareNoCase)"),
        ),
        (
            Bug::FoldsToUpper,
            Check::new("UnicodeString (compareNoCase folds to upper case)"),
        ),
        (
            Bug::TranslateZeroExtends,
            Check::new("UnicodeString (translate zero-extends)"),
        ),
    ];
    let mut undefined = 0u32;
    let mut ops_total = 0usize;
    for script in 0..5000 {
        let mut ops = Vec::new();
        for _ in 0..g.below(100) {
            let a = g.below(4) as u8;
            let t = g.below(4) as u8;
            let op = match g.below(100) {
                0..=14 => Op::Set(a, units(&mut g)),
                15..=22 => Op::Concat(a, units(&mut g)),
                23..=26 => Op::ConcatChar(
                    a,
                    [b'a' as u16, 0x20, 0, 0xe9, 0xffe9, b'N' as u16][g.below(6) as usize],
                ),
                27..=36 => Op::Trim(a),
                37..=43 => Op::RemoveLast(a),
                44..=47 => Op::Translate(a, ascii_bytes(&mut g)),
                48..=60 => Op::NextToken(
                    a,
                    t,
                    match g.below(6) {
                        0 | 1 => Delim::Literal(vec![b',' as u16, 0x20, 0x09, 0])
                            .into_literal(g.below(3)),
                        2 => Delim::Slot(g.below(4) as u8),
                        _ => Delim::Default,
                    },
                ),
                61..=62 => Op::Clear(a),
                63..=65 => Op::Assign(a, t),
                66..=67 => Op::SetU(a, t),
                68..=77 => Op::Dump(a),
                78..=83 => Op::CharAt(a, g.below(16) as i32 - 2),
                84..=89 => Op::Compare(a, t, units(&mut g)),
                90..=92 => Op::ConcatU(a, (a + 1 + g.below(3) as u8) % 4),
                93..=94 => Op::Construct(a, units(&mut g)),
                _ => Op::SetTail(a, g.below(10) as i32 - 1),
            };
            ops.push(op);
        }
        ops_total += ops.len();
        // Keep only the defined operations: each is tried on a copy of the
        // state, and dropped (counted) if the Rust reports it undefined.
        let mut defined: Vec<Op> = Vec::new();
        let mut state: [UnicodeString; 4] = Default::default();
        let mut scratch = Vec::new();
        for op in ops {
            let mut trial = state.clone();
            match step(&mut trial, &op, Bug::None, &mut scratch) {
                Some(()) => {
                    state = trial;
                    defined.push(op);
                }
                None => undefined += 1,
            }
        }
        let rust = run_rust(&defined, Bug::None);
        let c = run_cpp(&defined);
        if rust != c && std::env::var("UNICODE_DEBUG").is_ok() {
            for k in 1..=defined.len() {
                let (r, cc) = (run_rust(&defined[..k], Bug::None), run_cpp(&defined[..k]));
                if r != cc {
                    let lo = k.saturating_sub(8);
                    for (j, o) in defined[lo..k].iter().enumerate() {
                        let d = format!("{o:?}");
                        println!("op {}: {}", lo + j, &d[..d.len().min(200)]);
                    }
                    let n = r.len().min(cc.len());
                    let first = (0..n).find(|&i| r[i] != cc[i]).unwrap_or(n);
                    println!(
                        "first diff at byte {first}: rust {:?} c++ {:?}",
                        &r[first..(first + 40).min(r.len())],
                        &cc[first..(first + 40).min(cc.len())]
                    );
                    break;
                }
            }
        }
        ok.eq(&rust, &c, || {
            format!("script {script} ({} ops)", defined.len())
        });
        for (bug, check) in planted.iter_mut() {
            check.eq(run_rust(&defined, *bug), c.clone(), || {
                format!("script {script}")
            });
        }
    }
    println!(
        "     {ops_total} operations; {} threw ERROR_OUT_OF_MEMORY (compared); {undefined} left out (undefined or not established upstream; Rust returned Err)",
        TOO_LONG.load(std::sync::atomic::Ordering::Relaxed) / 2
    );
    ok.finish();
    for (_, check) in planted {
        check.finish_planted();
    }
}

impl Delim {
    /// A literal from the first `n + 1`... of the given units: lengths 1 to 4.
    fn into_literal(self, n: u32) -> Delim {
        match self {
            Delim::Literal(mut v) => {
                v.truncate(n as usize + 1);
                Delim::Literal(v)
            }
            d => d,
        }
    }
}
