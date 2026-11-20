#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/widestring.h, widestring.cpp (with trim.cpp's wcstrim, wwstring.cpp's Copy_Wide)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::wwlib::widestring::WideStringClass` against the upstream C++ (#215, group wwlib3).
//!
//! # Method
//!
//! `differential/wrap_draft_widestring.cpp` includes the upstream `widestring.h` and `widestring.cpp` unchanged (copied by
//! build.rs so that their includes find the stand-ins), `-fshort-wchar` (16-bit `wchar_t`, as MSVC), with `wcstrim` and
//! `StringClass::Copy_Wide` cut verbatim; its header lists the stand-ins (the 16-bit wide C runtime, `_vsnwprintf` as a copy of a
//! directive-free format, `MultiByteToWideChar`/`WideCharToMultiByte` over the Windows-1252 chart). A program of 1 to 40 random ops
//! (assign, append, `operator+`, the six comparisons, `Compare`, `Compare_No_Case`, `operator[]`, `Erase`, `Trim`, `Format`,
//! `Convert_From`, `Convert_To`, `Is_ANSI`, copy, `Get_Buffer`, the constructors with and without the temporary-string hint) runs on
//! one string on both sides; after every op the output is `Get_Length()`, the C-string length and its units, and the op's result.
//! 20,000 programs from a fixed seed.
//!
//! Not generated (undefined or unestablished upstream): `Compare`/`Compare_No_Case` return values are compared by SIGN (the C
//! standard defines only the sign; MSVC's UCRT returns -1/0/1, the stand-in the difference); `Compare_No_Case` over letters with case
//! outside ASCII (the stand-in folds `A`-`Z` only, the C locale: what MSVC's `towlower` does there is not established here, and the
//! draft folds every Unicode letter); `Erase` with a negative index or count; `operator[]` outside `0..Get_Length()`; `Format` with a
//! result of 512 units or more (`temp_buffer` has no terminator, upstream reads past it) and any `%` directive (the caller formats);
//! `Convert_To` of units outside Windows-1252 (the Windows best-fit mapping is not reproducible here); `Get_Buffer(n)` writes: only `n - 1`
//! non-zero units then the terminator (upstream leaves the buffer uninitialised); `operator= (WCHAR 0)` (the string then holds one NUL with
//! stored length 1, and a later `+=` that reallocates copies only the C string with `wcscpy` and resets the stored length: the draft, with no
//! allocation to model, cannot follow that, and upstream's own `Get_Length` and C string disagree there). `Release_Resources` is empty.
//!
//! # Predictions and results
//!
//! Written before the first run: the draft's `Trim` trims both ends fully while upstream's `wcstrim` tests `*source` (a pointer into the
//! already shifted buffer) before clipping each trailing blank, so after leading blanks it stops early: predicted mismatch in `Trim`
//! only when the string had leading blanks. `Compare` with an embedded NUL in the argument (the draft does not stop at it): predicted
//! mismatch. Everything else 0.

//! **Results.** First run (the draft as translated): 1,162 of 20,000 programs differed (prediction: `Trim` and `Compare` with an embedded NUL;
//! missed: also `Is_Empty` (a string holding one NUL), `Erase` (the stored length is `wcslen` afterwards), `Convert_To`, `Is_ANSI` and `Format`
//! (all stop at the first NUL upstream; the draft used the whole buffer), and the generator's own over-long `Format` text). Draft defects
//! corrected in `widestring.rs`: `Trim` now reproduces `wcstrim`'s `*source` test (a real upstream quirk: `"  a  "` becomes `"a "`),
//! every pointer argument is cut at its first NUL, `Is_Empty` is `m_Buffer[0] == 0`, `Erase` re-reads the C string length. After: 0 of 20,000
//! mismatching programs (about 400,000 ops, each compared whole). Planted bugs (mismatching programs of 20,000): 17361 `operator+` swapped:
//! 8,861; 17362 `operator >` NULL false: 2,398; 17363 `+= (WCHAR 0)` stores the NUL: 3,038; 17364 `Erase` clip one short: 3,819; 17365 `Trim`
//! `< 32`: 644; 17366 `Trim` without the `*source` test: 45; 17367 `Format` returns -1 from 510 units: 114; 17368 `Compare_No_Case` unfolded:
//! 1,690; 17369 `Get_Buffer` one unit too long: 11,783; 17370 `Is_ANSI` `< 255`: 25. 10 of 10 caught. (Two first plants were equivalent
//! mutants and were replaced: 17364 `>=` for `>` in the clip condition, 17367 a 511-unit cut only reachable above 512.)
//! `operator+`'s use of `Get_Length` for the second operand is not distinguishable in these inputs (it differs only for `operator= (WCHAR 0)`).

mod support;

use rts_lockstep_engine::drafts::wwlib::widestring::WideStringClass;
use rts_lockstep_engine::logic::modules::plant;
use support::{Check, Gen};

extern "C" {
    fn ref_ws_run(prog: *const i32, nops: i32, out: *mut i32, cap: i32) -> i32;
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_WS_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

const ASCII_ALPHABET: &[u16] = &[b'a' as u16, b'b' as u16, b'Z' as u16, b'A' as u16, b'm' as u16, b'0' as u16, b'9' as u16, b'_' as u16, b'[' as u16, b'`' as u16, b'@' as u16, 32, 9, 10, 1, 31, 33, 127];
// non-ASCII units without case (or not letters at all): safe for the C-locale and the Unicode fold alike
const WIDE_ALPHABET: &[u16] = &[0x80, 0xA0, 0xA3, 0xD7, 0xF7, 0x20AC, 0x2022, 0x2122, 0xFFFF, 0xD800, 0x4E2D];

fn gen_unit(g: &mut Gen, nul: bool) -> u16 {
    match g.below(12) {
        0 if nul => 0,
        1 | 2 => WIDE_ALPHABET[g.below(WIDE_ALPHABET.len() as u32) as usize],
        _ => ASCII_ALPHABET[g.below(ASCII_ALPHABET.len() as u32) as usize],
    }
}

fn gen_string(g: &mut Gen, nul: bool) -> Vec<u16> {
    let len = match g.below(10) {
        0 => 0,
        1 => 255 + g.below(4) as usize,
        2 => 1,
        _ => g.below(12) as usize,
    };
    let mut v: Vec<u16> = (0..len).map(|_| gen_unit(g, nul)).collect();
    // leading and trailing blanks (the Trim cases), and an occasional repeat of an earlier string's prefix for equal prefixes
    if g.below(4) == 0 {
        for _ in 0..g.below(4) {
            v.insert(0, [32u16, 9, 10, 1][g.below(4) as usize]);
        }
    }
    if g.below(4) == 0 {
        for _ in 0..g.below(4) {
            v.push([32u16, 9, 10, 1][g.below(4) as usize]);
        }
    }
    v
}

/// One generated op as the int stream both sides read. `s` is the model of the string's content only to bound `Erase` and `operator[]`.
struct Prog {
    words: Vec<i32>,
    nops: i32,
    starts: Vec<usize>,
}

fn push_str(w: &mut Vec<i32>, s: Option<&[u16]>) {
    match s {
        None => w.push(-1),
        Some(s) => {
            w.push(s.len() as i32);
            w.extend(s.iter().map(|&u| u as i32));
        }
    }
}

fn gen_prog(g: &mut Gen) -> Prog {
    let nops = 1 + g.below(40) as i32;
    let mut w: Vec<i32> = Vec::new();
    // an upper bound on the string's current length (units), for Erase / operator[]: tracked by running the Rust draft alongside
    let mut shadow = WideStringClass::new();
    let mut starts = Vec::new();
    for _ in 0..nops {
        starts.push(w.len());
        let op = g.below(20) as i32;
        let before = shadow.get_length();
        match op {
            0 => {
                let s = if g.below(8) == 0 { None } else { Some(gen_string(g, true)) };
                w.push(0);
                push_str(&mut w, s.as_deref());
                shadow.assign_wide(s.as_deref());
            }
            1 => {
                let c = gen_unit(g, false); // never 0: see the header
                w.extend([1, c as i32]);
                shadow.assign_char(c);
            }
            2 => {
                let s = if g.below(8) == 0 { None } else { Some(gen_string(g, true)) };
                w.push(2);
                push_str(&mut w, s.as_deref());
                shadow.append_wide(s.as_deref());
            }
            3 => {
                let c = if g.below(6) == 0 { 0 } else { gen_unit(g, false) };
                w.extend([3, c as i32]);
                shadow.append_char(c);
            }
            4 => {
                let s = gen_string(g, true);
                w.push(4);
                push_str(&mut w, Some(&s));
                w.push(g.below(2) as i32);
            }
            5 | 6 | 7 | 11 => {
                // an argument that is often equal to, or a prefix or case variant of, the current string
                let cur = shadow.as_wide().to_vec();
                let s: Option<Vec<u16>> = match g.below(8) {
                    0 => None,
                    1 | 2 => Some(cur),
                    3 => Some(cur.iter().take(g.below(cur.len() as u32 + 1) as usize).copied().collect()),
                    4 => Some(cur.iter().map(|&c| if (b'a' as u16..=b'z' as u16).contains(&c) { c - 32 } else if (b'A' as u16..=b'Z' as u16).contains(&c) { c + 32 } else { c }).collect()),
                    5 => {
                        let mut v = cur;
                        v.push(gen_unit(g, false));
                        Some(v)
                    }
                    _ => Some(gen_string(g, op != 11)),
                };
                let s = if op == 11 {
                    // Format's text: no `%`, shorter than 512 units (see the header)
                    s.map(|mut v| {
                        v.retain(|&c| c != b'%' as u16);
                        if g.below(40) == 0 {
                            while v.len() < 508 + g.below(3) as usize {
                                v.push(b'x' as u16);
                            }
                        }
                        v
                    })
                } else {
                    s
                };
                // Compare_No_Case only over strings without cased non-ASCII letters (see the header); Format only below 512 units
                let cased = |v: &[u16]| v.iter().any(|&u| u > 127 && char::from_u32(u as u32).map_or(false, |c| c.is_uppercase() || c.is_lowercase()));
                let op = if op == 6 && (cased(shadow.as_wide()) || s.as_deref().map_or(false, cased)) { 5 } else { op };
                let s = if op == 11 { s.map(|mut v| { v.truncate(511); v }) } else { s };
                w.push(op);
                push_str(&mut w, s.as_deref());
                if op == 11 {
                    shadow.format(s.as_deref());
                }
            }
            8 => {
                if before <= 0 {
                    w.extend([17]);
                    continue;
                }
                let i = g.below(before as u32) as i32;
                w.extend([8, i]);
            }
            9 => {
                let i = g.below(before as u32 + 3) as i32;
                let n = g.below(before as u32 + 3) as i32;
                w.extend([9, i, n]);
                shadow.erase(i, n);
            }
            10 => {
                w.push(10);
                shadow.trim();
            }
            12 => {
                w.push(12);
                if g.below(8) == 0 {
                    w.push(-1);
                } else {
                    // bytes 1..=255 (a NUL ends the C string upstream)
                    let n = g.below(10) as usize;
                    let bytes: Vec<u8> = (0..n).map(|_| 1 + g.below(255) as u8).collect();
                    w.push(n as i32);
                    w.extend(bytes.iter().map(|&b| b as i32));
                    shadow.convert_from(Some(&bytes));
                }
            }
            13 => {
                // Convert_To only over units Windows-1252 has: constrain the string first to such units through an assign
                let s: Vec<u16> = gen_string(g, false).into_iter().filter(|u| *u < 0x80 || (0xA0..=0xFF).contains(u) || [0x20AC, 0x2022, 0x2122].contains(u)).collect();
                w.push(0);
                push_str(&mut w, Some(&s));
                shadow.assign_wide(Some(&s));
                starts.push(w.len());
                w.push(13);
            }
            14 => w.push(14),
            15 => {
                w.push(15);
                shadow = WideStringClass::from_wide(Some(shadow.as_wide()));
            }
            16 => {
                let n = 1 + g.below(12) as i32;
                let fill = 1 + g.below(120) as i32;
                w.extend([16, n, fill]);
                let buf = shadow.get_buffer(n);
                for u in buf.iter_mut() {
                    *u = fill as u16;
                }
            }
            17 => w.push(17),
            18 => {
                let kind = g.below(3) as i32;
                w.extend([18, kind]);
                let tmp = g.below(2) as i32;
                match kind {
                    0 => {
                        let s = if g.below(6) == 0 { None } else { Some(gen_string(g, true)) };
                        push_str(&mut w, s.as_deref());
                        w.push(tmp);
                        shadow = WideStringClass::from_wide(s.as_deref());
                    }
                    1 => {
                        let c = gen_unit(g, false);
                        w.extend([c as i32, tmp]);
                        shadow = WideStringClass::from_char(c);
                    }
                    _ => {
                        let n = g.below(10) as usize;
                        let bytes: Vec<u8> = (0..n).map(|_| 1 + g.below(255) as u8).collect();
                        w.push(n as i32);
                        w.extend(bytes.iter().map(|&b| b as i32));
                        w.push(tmp);
                        shadow = WideStringClass::from_ansi(Some(&bytes));
                    }
                }
            }
            _ => {
                // 19: operator[] assignment
                if before <= 0 {
                    w.extend([17]);
                    continue;
                }
                let i = g.below(before as u32) as i32;
                let v = gen_unit(g, false) as i32;
                w.extend([19, i, v]);
                *shadow.at_mut(i) = v as u16;
            }
        }
    }
    // the `continue`s above wrote one op (17) each: count the ops by decoding is overkill, so the count is returned separately
    Prog { words: w, nops: starts.len() as i32, starts }
}

fn dump(out: &mut Vec<i32>, s: &WideStringClass) {
    out.push(s.get_length());
    let c = s.as_wide();
    let l = c.iter().position(|&u| u == 0).unwrap_or(c.len());
    out.push(l as i32);
    out.extend(c[..l].iter().map(|&u| u as i32));
}

struct Reader<'a> {
    w: &'a [i32],
    i: usize,
}

impl<'a> Reader<'a> {
    fn next(&mut self) -> i32 {
        let v = self.w[self.i];
        self.i += 1;
        v
    }
    fn string(&mut self) -> Option<Vec<u16>> {
        let n = self.next();
        if n < 0 {
            return None;
        }
        Some((0..n).map(|_| self.next() as u16).collect())
    }
}

fn sign(x: i32) -> i32 {
    if x < 0 {
        -1
    } else if x > 0 {
        1
    } else {
        0
    }
}

fn run_rust(p: &Prog) -> Vec<i32> {
    let mut r = Reader { w: &p.words, i: 0 };
    let mut out = Vec::new();
    let mut a = WideStringClass::new();
    for _ in 0..p.nops {
        let op = r.next();
        match op {
            0 => {
                let s = r.string();
                a.assign_wide(s.as_deref());
            }
            1 => {
                let c = r.next() as u16;
                a.assign_char(c);
            }
            2 => {
                let s = r.string();
                a.append_wide(s.as_deref());
            }
            3 => {
                let c = r.next() as u16;
                a.append_char(c);
            }
            4 => {
                let s = r.string();
                let _tmp = r.next();
                let b = WideStringClass::from_wide(s.as_deref());
                let c = WideStringClass::concat(&a, &b);
                dump(&mut out, &c);
            }
            5 => {
                let s = r.string();
                out.push(sign(a.compare(s.as_deref())));
            }
            6 => {
                let s = r.string();
                out.push(sign(a.compare_no_case(s.as_deref())));
            }
            7 => {
                let s = r.string();
                let s = s.as_deref();
                out.extend([a.eq_wide(s) as i32, a.ne_wide(s) as i32, a.lt_wide(s) as i32, a.le_wide(s) as i32, a.gt_wide(s) as i32, a.ge_wide(s) as i32]);
            }
            8 => {
                let i = r.next();
                out.push(a.at(i) as i32);
            }
            9 => {
                let i = r.next();
                let n = r.next();
                a.erase(i, n);
            }
            10 => a.trim(),
            11 => {
                let s = r.string();
                out.push(a.format(s.as_deref()));
            }
            12 => {
                let n = r.next();
                if n < 0 {
                    out.push(a.convert_from(None) as i32);
                } else {
                    let bytes: Vec<u8> = (0..n).map(|_| r.next() as u8).collect();
                    out.push(a.convert_from(Some(&bytes)) as i32);
                }
            }
            13 => {
                let mut v = Vec::new();
                let ok = a.convert_to(&mut v);
                out.push(ok as i32);
                out.push(v.len() as i32);
                out.extend(v.iter().map(|&b| b as i32));
            }
            14 => out.push(a.is_ansi() as i32),
            15 => {
                let b = WideStringClass::from_wide(Some(a.as_wide()));
                dump(&mut out, &b);
                a = b;
            }
            16 => {
                let n = r.next();
                let fill = r.next();
                for u in a.get_buffer(n).iter_mut() {
                    *u = fill as u16;
                }
            }
            17 => out.push(a.is_empty() as i32),
            18 => {
                let kind = r.next();
                a = match kind {
                    0 => {
                        let s = r.string();
                        let _tmp = r.next();
                        WideStringClass::from_wide(s.as_deref())
                    }
                    1 => {
                        let c = r.next() as u16;
                        let _tmp = r.next();
                        WideStringClass::from_char(c)
                    }
                    _ => {
                        let n = r.next();
                        let bytes: Vec<u8> = (0..n).map(|_| r.next() as u8).collect();
                        let _tmp = r.next();
                        WideStringClass::from_ansi(Some(&bytes))
                    }
                };
            }
            19 => {
                let i = r.next();
                let v = r.next();
                *a.at_mut(i) = v as u16;
            }
            _ => unreachable!(),
        }
        dump(&mut out, &a);
    }
    out
}

fn run_cpp(p: &Prog) -> Vec<i32> {
    let mut out = vec![0i32; 200_000];
    let n = unsafe { ref_ws_run(p.words.as_ptr(), p.nops, out.as_mut_ptr(), out.len() as i32) } as usize;
    assert!(n <= out.len(), "the C++ output outgrew the buffer");
    out.truncate(n);
    out
}

#[test]
fn widestring_matches_upstream_over_random_programs() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_17361);
    let mut c = Check::new("WideStringClass programs");
    for i in 0..n {
        let p = gen_prog(&mut g);
        let cpp = run_cpp(&p);
        let rs = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_rust(&p))).ok();
        let bad = rs.as_ref() != Some(&cpp);
        c.eq(rs.clone(), Some(cpp.clone()), || format!("program {i}: {:?}", p.words));
        if bad && std::env::var("DRAFT_WS_SHOW").is_ok() && c.mismatches <= 12 {
            // the shortest failing prefix and its last op
            for k in 1..=p.nops {
                let q = Prog { words: p.words.clone(), nops: k, starts: Vec::new() };
                let a = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_rust(&q))).ok();
                if a.as_ref() != Some(&run_cpp(&q)) {
                    let at = p.starts[(k - 1) as usize];
                    println!("SHOW program {i}: first differs at op {k}: {:?}", &p.words[at..(at + 12).min(p.words.len())]);
                    println!("   prefix {:?}\n   rust {:?}\n   c++  {:?}", &p.words[..(at + 12).min(p.words.len())], a, run_cpp(&q));
                    break;
                }
            }
        }
    }
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}
