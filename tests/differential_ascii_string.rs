// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::ascii_string` against upstream
//! `GameEngine/Source/Common/System/AsciiString.cpp` and
//! `Include/Common/AsciiString.h` (commit 0a05454d), compiled unchanged
//! (`differential/wrap_ascii_string.cpp`).
//!
//! Method: 5,000 random scripts of up to 100 operations on four strings:
//! `set`, `concat` (string and char), `trim`, `toLower`, `removeLastChar`,
//! `translate`, `nextToken` (default and given separators), `clear`,
//! assignment, and checks of everything observable: whether a buffer exists,
//! `getLength`, `isEmpty`, `isNone`, `str()`, `getCharAt` at every defined
//! index, the four prefix/suffix tests, the signs of `compare` and
//! `compareNoCase`, `find` and `reverseFind`. Inputs mix whitespace, mixed
//! case, `None`/`none`, separators, embedded NULs (C strings end there),
//! bytes from 0x80, and strings near the 2,048-byte `toLower` buffer and the
//! 32,767-byte `MAX_LEN` (where upstream throws `ERROR_OUT_OF_MEMORY`, which
//! the logs compare). Operations upstream leaves undefined (`isspace`/
//! `tolower` of a byte from 0x80, the `toLower` buffer overflow, `getCharAt`
//! past the terminator) are not sent to the C++; the Rust must return an
//! error there. The logs must be identical byte for byte.
//!
//! Planted bugs, each of which must be caught: `removeLastChar` releasing an
//! emptied buffer, a case-sensitive `compareNoCase`, and `translate` keeping
//! characters whose low byte is 0.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ascii_string::{AsciiString, AsciiStringError};
use rts_lockstep_engine::differential as cpp;
use std::cmp::Ordering;
use support::{Check, Gen};

#[derive(Clone, Debug)]
enum Op {
    Set(u8, Vec<u8>),
    Concat(u8, Vec<u8>),
    ConcatChar(u8, u8),
    Trim(u8),
    ToLower(u8),
    RemoveLast(u8),
    Translate(u8, Vec<u16>),
    NextToken(u8, u8, Option<Vec<u8>>),
    Clear(u8),
    Assign(u8, u8),
    Dump(u8),
    CharAt(u8, i32),
    Compare(u8, Vec<u8>),
    Find(u8, u8),
}

fn encode(ops: &[Op]) -> Vec<u8> {
    let mut s = Vec::new();
    let bytes = |s: &mut Vec<u8>, b: &[u8]| {
        s.extend_from_slice(&(b.len() as u16).to_le_bytes());
        s.extend_from_slice(b);
    };
    for op in ops {
        match op {
            Op::Set(a, b) => {
                s.extend_from_slice(&[0, *a]);
                bytes(&mut s, b);
            }
            Op::Concat(a, b) => {
                s.extend_from_slice(&[1, *a]);
                bytes(&mut s, b);
            }
            Op::ConcatChar(a, c) => s.extend_from_slice(&[2, *a, *c]),
            Op::Trim(a) => s.extend_from_slice(&[3, *a]),
            Op::ToLower(a) => s.extend_from_slice(&[4, *a]),
            Op::RemoveLast(a) => s.extend_from_slice(&[5, *a]),
            Op::Translate(a, w) => {
                s.extend_from_slice(&[6, *a]);
                s.extend_from_slice(&(w.len() as u16).to_le_bytes());
                for c in w {
                    s.extend_from_slice(&c.to_le_bytes());
                }
            }
            Op::NextToken(a, t, seps) => {
                s.extend_from_slice(&[7, *a, *t]);
                match seps {
                    Some(p) => {
                        s.extend_from_slice(&(p.len() as i16).to_le_bytes());
                        s.extend_from_slice(p);
                    }
                    None => s.extend_from_slice(&(-1i16).to_le_bytes()),
                }
            }
            Op::Clear(a) => s.extend_from_slice(&[8, *a]),
            Op::Assign(a, t) => s.extend_from_slice(&[9, *a, *t]),
            Op::Dump(a) => s.extend_from_slice(&[10, *a]),
            Op::CharAt(a, i) => {
                s.extend_from_slice(&[11, *a]);
                s.extend_from_slice(&i.to_le_bytes());
            }
            Op::Compare(a, b) => {
                s.extend_from_slice(&[12, *a]);
                bytes(&mut s, b);
            }
            Op::Find(a, c) => s.extend_from_slice(&[13, *a, *c]),
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
    TranslateKeepsZeroLowByte,
}

/// One operation of the Rust run, appending to the wrapper's log format;
/// `None` if it is undefined upstream (the Rust returned the error).
fn step(slot: &mut [AsciiString; 4], op: &Op, bug: Bug, lg: &mut Vec<u8>) -> Option<()> {
    let ord = |o: Ordering| o as i8 as u8;
    let at = lg.len();
    lg.push(0);
    let r: Result<(), AsciiStringError> = (|| {
        match op {
            Op::Set(a, b) => slot[*a as usize].set(b)?,
            Op::Concat(a, b) => slot[*a as usize].concat(b)?,
            Op::ConcatChar(a, c) => slot[*a as usize].concat_char(*c)?,
            Op::Trim(a) => slot[*a as usize].trim()?,
            Op::ToLower(a) => slot[*a as usize].to_lower()?,
            Op::RemoveLast(a) => {
                let s = &mut slot[*a as usize];
                s.remove_last_char()?;
                if bug == Bug::RemoveLastReleases && s.is_empty() {
                    s.clear();
                }
            }
            Op::Translate(a, w) => {
                if bug == Bug::TranslateKeepsZeroLowByte {
                    let bytes: Vec<u8> = w.iter().map(|&c| if c as u8 == 0 { b'?' } else { c as u8 }).collect();
                    slot[*a as usize].clear();
                    slot[*a as usize].concat(&bytes)?;
                } else {
                    slot[*a as usize].translate(w)?;
                }
            }
            Op::NextToken(a, t, seps) => {
                let r = if a == t {
                    // `tok == this`: upstream returns false, changing nothing
                    false
                } else {
                    let mut tok = std::mem::take(&mut slot[*t as usize]);
                    let r = slot[*a as usize].next_token(&mut tok, seps.as_deref());
                    slot[*t as usize] = tok;
                    r?
                };
                lg.push(u8::from(r));
            }
            Op::Clear(a) => slot[*a as usize].clear(),
            Op::Assign(a, t) => slot[*a as usize] = slot[*t as usize].clone(),
            Op::Dump(a) => {
                let s = &slot[*a as usize];
                lg.push(u8::from(s.has_buffer()));
                lg.extend_from_slice(&s.get_length().to_le_bytes());
                lg.extend_from_slice(&[
                    u8::from(s.is_empty()),
                    u8::from(s.is_none()),
                    u8::from(s.is_not_empty()),
                    u8::from(s.is_not_none()),
                ]);
                lg.extend_from_slice(&(s.str().len() as i32).to_le_bytes());
                lg.extend_from_slice(s.str());
            }
            Op::CharAt(a, i) => {
                let c = slot[*a as usize].try_get_char_at(*i)?;
                lg.push(c);
            }
            Op::Compare(a, p) => {
                let s = &slot[*a as usize];
                lg.extend_from_slice(&[
                    u8::from(s.starts_with(p)),
                    u8::from(s.starts_with_no_case(p)),
                    u8::from(s.ends_with(p)),
                    u8::from(s.ends_with_no_case(p)),
                    ord(s.compare(p)),
                ]);
                let nc = if bug == Bug::CaseSensitiveNoCase { s.compare(p) } else { s.compare_no_case(p) };
                lg.push(ord(nc));
            }
            Op::Find(a, c) => {
                let s = &slot[*a as usize];
                let f = s.find(*c).map_or(-1, |v| v as i32);
                let r = s.reverse_find(*c).map_or(-1, |v| v as i32);
                lg.extend_from_slice(&f.to_le_bytes());
                lg.extend_from_slice(&r.to_le_bytes());
            }
        }
        Ok(())
    })();
    match r {
        Ok(()) => Some(()),
        Err(AsciiStringError::TooLong) => {
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
    let mut slot: [AsciiString; 4] = Default::default();
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
    let mut log = vec![0u8; 32 * 1024 * 1024];
    let n = unsafe { cpp::ref_ascii_run(script.as_ptr(), script.len() as i32, log.as_mut_ptr(), log.len() as i32) };
    assert!(n >= 0, "C++ log overflow");
    log.truncate(n as usize);
    log
}

fn text(g: &mut Gen) -> Vec<u8> {
    let pick = |g: &mut Gen| -> u8 {
        match g.below(16) {
            0 => b' ',
            1 => b'\t',
            2 => b'\n',
            3 => 0x0b,
            4 => b'\r',
            5 => b',',
            6 => 0,
            7 => 0x80 + g.below(0x80) as u8,
            8 => b'A' + g.below(26) as u8,
            _ => b'a' + g.below(26) as u8,
        }
    };
    match g.below(14) {
        0 => b"None".to_vec(),
        1 => b"nONe".to_vec(),
        2 => Vec::new(),
        3 => vec![b'x'; 2047 + g.below(3) as usize],
        4 => vec![b'y'; 32762 + g.below(3) as usize],
        5 => format!("  Alpha{}Beta  ", ["", " ", ",", "\t"][g.below(4) as usize]).into_bytes(),
        _ => {
            let n = g.below(14) as usize;
            let mut v: Vec<u8> = (0..n).map(|_| pick(g)).collect();
            if g.below(2) == 0 {
                v.retain(|&b| b < 0x80);
            }
            v
        }
    }
}

#[test]
fn ascii_string_matches_cpp() {
    let mut g = Gen::new(501);
    let mut ok = Check::new("AsciiString script logs");
    let mut planted = [
        (Bug::RemoveLastReleases, Check::new("AsciiString (removeLastChar releases)")),
        (Bug::CaseSensitiveNoCase, Check::new("AsciiString (case-sensitive compareNoCase)")),
        (Bug::TranslateKeepsZeroLowByte, Check::new("AsciiString (translate keeps 0 low bytes)")),
    ];
    let mut undefined = 0u32;
    let mut ops_total = 0usize;
    for script in 0..5000 {
        let mut ops = Vec::new();
        for _ in 0..g.below(100) {
            let a = g.below(4) as u8;
            let t = g.below(4) as u8;
            let op = match g.below(100) {
                0..=14 => Op::Set(a, text(&mut g)),
                15..=24 => Op::Concat(a, text(&mut g)),
                25..=29 => Op::ConcatChar(a, [b'a', b' ', 0, 0xe9, b'N'][g.below(5) as usize]),
                30..=36 => Op::Trim(a),
                37..=41 => Op::ToLower(a),
                42..=48 => Op::RemoveLast(a),
                49..=52 => Op::Translate(a, (0..g.below(10)).map(|_| [0x41, 0x100, 0x1ff, 0x20, 0x61, g.u32() as u16][g.below(6) as usize]).collect()),
                53..=62 => Op::NextToken(a, t, if g.below(3) == 0 { Some(b", ".to_vec()) } else { None }),
                63..=64 => Op::Clear(a),
                65..=67 => Op::Assign(a, t),
                68..=79 => Op::Dump(a),
                80..=86 => Op::CharAt(a, g.below(16) as i32 - 2),
                87..=94 => Op::Compare(a, text(&mut g)),
                _ => Op::Find(a, [b'a', b' ', 0, b'e', 0xe9][g.below(5) as usize]),
            };
            ops.push(op);
        }
        ops_total += ops.len();
        // Keep only the defined operations: each is tried on a copy of the
        // state, and dropped (counted) if the Rust reports it undefined.
        let mut defined: Vec<Op> = Vec::new();
        let mut state: [AsciiString; 4] = Default::default();
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
        if rust != c && std::env::var("ASCII_DEBUG").is_ok() {
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
                    println!("first diff at byte {first}: rust {:?} c++ {:?}", &r[first..(first + 40).min(r.len())], &cc[first..(first + 40).min(cc.len())]);
                    break;
                }
            }
        }
        ok.eq(&rust, &c, || format!("script {script} ({} ops)", defined.len()));
        for (bug, check) in planted.iter_mut() {
            check.eq(run_rust(&defined, *bug), c.clone(), || format!("script {script}"));
        }
    }
    println!(
        "     {ops_total} operations; {} threw ERROR_OUT_OF_MEMORY (compared); {undefined} left out (undefined upstream; Rust returned Err)",
        TOO_LONG.load(std::sync::atomic::Ordering::Relaxed) / 2
    );
    ok.finish();
    for (_, check) in planted {
        check.finish_planted();
    }
}
