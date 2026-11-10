#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/UserPreferences.cpp (intAsStr, boolAsStr, realAsStr, UserPreferences::load,
//     write, getBool, getReal, getInt, getAsciiString, setBool, setReal, setInt, setAsciiString)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The user preference files (`client::user_preferences::UserPreferences`) against the C++, #93.
//!
//! # Method
//!
//! The C++ side is UserPreferences.h's class and UserPreferences.cpp's base class, cut verbatim by build.rs and
//! compiled by `differential/wrap_user_prefs.cpp` with the real `AsciiString` (as the AsciiString differential) and
//! the C library's own `fopen` / `fgets` / `fprintf`: the case's bytes are written to a real file in a folder of this
//! test's, `load("Prefs.ini")` reads it, `write()` writes it back. The Rust side reads the same bytes with
//! `read_lines` (glibc's `fopen(.., "r")` does no text-mode translation, so the bytes are compared before MSVC's
//! text mode, [`crt_text_read`] / [`crt_text_write`], which have their own unit test) and prints with `write_text`.
//! Both then run the case's getters and setters and write again; the dumps (the map after the load, the written file,
//! each getter's answer, the map and the file at the end) must be equal.
//!
//! A case: 0 to 40 lines of keys from a small pool (case variants, so duplicates are common), values with blanks
//! around and inside, `=` inside, empty keys and empty values, lines of blanks or of `=` only, a line without `=`
//! (one in 30), CR LF endings, a NUL now and then, a byte from 0x80 at an edge now and then, and one line in 40 longer
//! than 2,047 bytes; then up to 12 operations on the pool's keys (`getBool`, `getInt`, `getReal`, `getAsciiString`
//! with defaults, `setBool`, `setInt`, `setReal` of any bits, `setAsciiString`).
//!
//! **Where the two are not compared** (counted, never silently): a case where a line without anything after its key
//! read past the terminator (`line.str() + 1`, undefined: the C++ reads the byte after a static) is compared on the
//! map without the keys of those lines and nothing else; a `getReal` of a value glibc's `strtod` reads as hexadecimal,
//! infinity or NaN (which the translation, as MSVC's of the time, does not); `%g` is compared with glibc's two-digit
//! exponent (`format_g(.., 2)`), the translation printing MSVC's three.
//!
//! # Predictions (written before the first run)
//!
//! 0 mismatches. Planted bugs (`L9_PLANT=<id>`, each in its own run), all caught:
//!
//! | id | planted bug | predicted |
//! |---|---|---|
//! | 8941 | the first of duplicate keys kept | caught in most cases (the pool's duplicates) |
//! | 8942 | the value not trimmed | caught (blanks around values) |
//! | 8943 | written as `key=value` | caught in every case with a key |
//! | 8944 | `getBool` without `toLower` | caught (the pool's `YES`, `True`) |
//! | 8945 | `fgets` of `LINE_LEN` bytes, not `LINE_LEN - 1` | caught, rarely (the long lines) |
//!
//! # Results (2026-11-10, `USER_PREFS_CASES=20000`, the default)
//!
//! **0 mismatches on the first run**, as predicted: 20,000 cases, 403,076 lines (6,357 longer than 2,047 bytes),
//! 169,591 keys loaded, 33,024 getter answers compared. Not compared whole: 8,816 cases met the empty line's undefined
//! read (compared on their other keys only; 11,184 compared whole), 553 `getReal`s of a `strtod`-only form; counted:
//! 5,575 cases with `isspace` of a byte from 0x80 (compared: glibc's C locale reads them as the translation does), 177
//! `atoi`s out of range (compared: glibc's `strtol` and the translation's wrap agree below 2^63).
//!
//! Planted bugs: 5 of 5 caught: 8941 in 17,223 cases, 8942 in 18,981, 8943 in 11,116, 8944 in 354, 8945 in 4,894.
//! One miss in the prediction: 8945 was predicted caught "rarely"; the long lines (one in 60 of a case's lines) make it
//! common.

use rts_lockstep_engine::client::user_preferences::{atof, format_g, PrefsUndefined, UserPreferences};
use rts_lockstep_engine::differential as cpp;
use std::collections::BTreeSet;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const KEYS: [&str; 12] = ["Alpha", "alpha", "ALPHA", "Beta", "MusicVolume", "Gamma", "x", "Two Words", "Resolution", "k=k", "B", "UseShadowVolumes"];
const VALUES: [&str; 22] = [
    "1", "0", "yes", "YES", "True", "t", "ok", "No", "42", "-17", "  12abc", "3.5", "-0.25e2", "1e40", "0x1A", "inf", "nan", "1024 768", "a=b", "", "99999999999", " spaced  out ",
];

fn pick<'a>(r: &mut Rng, xs: &[&'a str]) -> &'a str {
    xs[r.below(xs.len() as u64) as usize]
}

fn blanks(r: &mut Rng) -> &'static str {
    ["", "", " ", "\t", "  ", " \t "][r.below(6) as usize]
}

/// A case's file.
fn file(r: &mut Rng) -> Vec<u8> {
    let mut f = Vec::new();
    for _ in 0..r.below(41) {
        let kind = r.below(60);
        let mut line: Vec<u8> = match kind {
            0 | 1 => pick(r, &KEYS).as_bytes().to_vec(),
            2 => b"===".to_vec(),
            3 => b"   ".to_vec(),
            4 => Vec::new(),
            5 => format!("{}=", pick(r, &KEYS)).into_bytes(),
            6 => format!("={}={}", pick(r, &KEYS), pick(r, &VALUES)).into_bytes(),
            7 => format!("{}=={}", pick(r, &KEYS), pick(r, &VALUES)).into_bytes(),
            8 => format!("{}=\u{0}{}", pick(r, &KEYS), pick(r, &VALUES)).into_bytes(),
            9 => {
                let mut l = format!("{}{}=", blanks(r), pick(r, &KEYS)).into_bytes();
                l.push(0x80 + r.below(0x80) as u8);
                l
            }
            10 => {
                let mut l = format!("{}=", pick(r, &KEYS)).into_bytes();
                l.extend(std::iter::repeat_n(b'v', 2030 + r.below(40) as usize));
                l.extend_from_slice(format!("{}={}", pick(r, &KEYS), pick(r, &VALUES)).as_bytes());
                l
            }
            _ => format!("{}{}{}={}{}{}", blanks(r), pick(r, &KEYS), blanks(r), blanks(r), pick(r, &VALUES), blanks(r)).into_bytes(),
        };
        if r.below(5) == 0 {
            line.push(b'\r');
        }
        line.push(b'\n');
        f.extend(line);
    }
    if r.below(4) == 0 {
        // a last line without its line break
        f.extend_from_slice(format!("{}={}", pick(r, &KEYS), pick(r, &VALUES)).as_bytes());
    }
    f
}

#[derive(Debug)]
enum Op {
    GetBool(String, bool),
    GetInt(String, i32),
    GetReal(String, f32),
    GetString(String, String),
    SetBool(String, bool),
    SetInt(String, i32),
    SetReal(String, f32),
    SetString(String, String),
}

fn ops(r: &mut Rng) -> Vec<Op> {
    (0..r.below(13))
        .map(|_| {
            let k = pick(r, &KEYS).to_string();
            match r.below(8) {
                0 => Op::GetBool(k, r.below(2) == 1),
                1 => Op::GetInt(k, r.next() as i32),
                2 => Op::GetReal(k, (r.below(2000) as f32 - 1000.0) / 8.0),
                3 => Op::GetString(k, pick(r, &VALUES).to_string()),
                4 => Op::SetBool(k, r.below(2) == 1),
                5 => Op::SetInt(k, r.next() as i32),
                6 => Op::SetReal(k, if r.below(2) == 0 { f32::from_bits(r.next() as u32) } else { (r.below(100_000) as f32 - 50_000.0) / 64.0 }),
                _ => Op::SetString(k, pick(r, &VALUES).to_string()),
            }
        })
        .collect()
}

fn s16(out: &mut Vec<u8>, s: &str) {
    out.extend((s.len() as u16).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn encode(file: &[u8], ops: &[Op]) -> Vec<u8> {
    let mut b = (file.len() as u32).to_le_bytes().to_vec();
    b.extend_from_slice(file);
    for op in ops {
        match op {
            Op::GetBool(k, d) => {
                b.push(1);
                s16(&mut b, k);
                b.push(u8::from(*d));
            }
            Op::GetInt(k, d) => {
                b.push(2);
                s16(&mut b, k);
                b.extend(d.to_le_bytes());
            }
            Op::GetReal(k, d) => {
                b.push(3);
                s16(&mut b, k);
                b.extend(d.to_bits().to_le_bytes());
            }
            Op::GetString(k, d) => {
                b.push(4);
                s16(&mut b, k);
                s16(&mut b, d);
            }
            Op::SetBool(k, v) => {
                b.push(5);
                s16(&mut b, k);
                b.push(u8::from(*v));
            }
            Op::SetInt(k, v) => {
                b.push(6);
                s16(&mut b, k);
                b.extend(v.to_le_bytes());
            }
            Op::SetReal(k, v) => {
                b.push(7);
                s16(&mut b, k);
                b.extend(v.to_bits().to_le_bytes());
            }
            Op::SetString(k, v) => {
                b.push(8);
                s16(&mut b, k);
                s16(&mut b, v);
            }
        }
    }
    b
}

struct Reader<'a>(&'a [u8], usize);

impl Reader<'_> {
    fn u8(&mut self) -> u8 {
        self.1 += 1;
        self.0[self.1 - 1]
    }
    fn u32(&mut self) -> u32 {
        self.1 += 4;
        u32::from_le_bytes(self.0[self.1 - 4..self.1].try_into().expect("4"))
    }
    fn bytes(&mut self) -> Vec<u8> {
        let n = self.u32() as usize;
        self.1 += n;
        self.0[self.1 - n..self.1].to_vec()
    }
    fn map(&mut self) -> Vec<(Vec<u8>, Vec<u8>)> {
        (0..self.u32()).map(|_| (self.bytes(), self.bytes())).collect()
    }
}

/// The Rust side's `%g` as glibc prints it (two exponent digits), for `setReal`'s values.
fn glibc_real(v: f32) -> Vec<u8> {
    format_g(f64::from(v), 2).into_bytes()
}

/// `setReal` as the Rust translation does it, with glibc's exponent (the translation itself prints MSVC's three).
fn set_real(p: &mut UserPreferences, k: &[u8], v: f32) {
    p.set_real(k, v);
    p.map.insert(k.to_vec(), glibc_real(v));
}

/// Whether glibc's `strtod` reads `v` as hexadecimal, infinity or NaN (a form the translation does not read).
fn strtod_special(v: &[u8]) -> bool {
    let t: Vec<u8> = v.iter().copied().skip_while(|c| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')).collect();
    let t = t.strip_prefix(b"-").or_else(|| t.strip_prefix(b"+")).unwrap_or(&t).to_ascii_lowercase();
    t.starts_with(b"0x") || t.starts_with(b"inf") || t.starts_with(b"nan")
}

#[test]
fn user_preferences_match_the_cpp() {
    let cases: u64 = std::env::var("USER_PREFS_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let dir = std::env::temp_dir().join(format!("rts_user_prefs_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the test's folder");
    let dir_c = std::ffi::CString::new(format!("{}/", dir.display())).expect("path");
    let mut r = Rng(0x5eed_8941_u64);
    let (mut mismatches, mut lines, mut keys, mut undefined_cases, mut special_reals, mut non_ascii, mut out_of_range, mut long_lines, mut compared_ops) = (0, 0, 0, 0, 0, 0, 0, 0, 0);
    let mut first = None;
    let mut out = vec![0u8; 1 << 20];
    for case in 0..cases {
        let f = file(&mut r);
        let os = ops(&mut r);
        lines += f.iter().filter(|&&c| c == b'\n').count();
        long_lines += f.split(|&c| c == b'\n').filter(|l| l.len() > 2047).count();
        let input = encode(&f, &os);
        let n = unsafe { cpp::ref_user_prefs_run(input.as_ptr(), input.len() as i32, dir_c.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
        assert!(n >= 0, "case {case}: the C++ side failed");
        let mut c = Reader(&out[..n as usize], 0);
        // the Rust side
        let mut p = UserPreferences::new();
        p.filename = b"Prefs.ini".to_vec();
        p.read_lines(&f);
        let undefined_keys: BTreeSet<Vec<u8>> = p.undefined.iter().filter_map(|u| if let PrefsUndefined::ReadPastEmptyLine(k) = u { (!k.is_empty()).then(|| k.clone()) } else { None }).collect();
        non_ascii += usize::from(p.undefined.contains(&PrefsUndefined::NonAsciiCharClass));
        keys += p.map.len();
        let mut mism = |what: String| {
            mismatches += 1;
            if first.is_none() {
                first = Some(format!("case {case}: {what}\nfile: {:?}", String::from_utf8_lossy(&f)));
            }
        };
        let loaded = c.u8();
        let cmap = c.map();
        if loaded != 1 {
            mism("load did not open".into());
            continue;
        }
        let rmap: Vec<(Vec<u8>, Vec<u8>)> = p.map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        if !undefined_keys.is_empty() {
            undefined_cases += 1;
            let a: Vec<_> = cmap.iter().filter(|(k, _)| !undefined_keys.contains(k)).collect();
            let b: Vec<_> = rmap.iter().filter(|(k, _)| !undefined_keys.contains(k)).collect();
            if a != b {
                mism(format!("map (undefined keys left out): C++ {a:?}\nRust {b:?}"));
            }
            continue;
        }
        if cmap != rmap {
            mism(format!("map after load: C++ {cmap:?}\nRust {rmap:?}"));
            continue;
        }
        let _ = c.u8();
        let cfile = c.bytes();
        if cfile != p.write_text() {
            mism(format!("file written: C++ {:?}\nRust {:?}", String::from_utf8_lossy(&cfile), String::from_utf8_lossy(&p.write_text())));
            continue;
        }
        let mut bad = None;
        for op in &os {
            match op {
                Op::GetBool(k, d) => {
                    let (cv, rv) = (c.u8(), u8::from(p.get_bool(k.as_bytes(), *d)));
                    compared_ops += 1;
                    if cv != rv {
                        bad = Some(format!("getBool({k}): C++ {cv} Rust {rv}"));
                    }
                }
                Op::GetInt(k, d) => {
                    let before = p.undefined.len();
                    let (cv, rv) = (c.u32() as i32, p.get_int(k.as_bytes(), *d));
                    if p.undefined[before..].contains(&PrefsUndefined::IntOutOfRange) {
                        out_of_range += 1;
                    }
                    compared_ops += 1;
                    if cv != rv {
                        bad = Some(format!("getInt({k}): C++ {cv} Rust {rv}"));
                    }
                }
                Op::GetReal(k, d) => {
                    let cv = c.u32();
                    let v = p.map.get(k.as_bytes()).cloned().unwrap_or_default();
                    if strtod_special(&v) {
                        special_reals += 1;
                        continue;
                    }
                    let rv = p.get_real(k.as_bytes(), *d).to_bits();
                    compared_ops += 1;
                    if cv != rv && !(f32::from_bits(cv).is_nan() && f32::from_bits(rv).is_nan()) {
                        bad = Some(format!("getReal({k} = {:?}): C++ {} Rust {} (atof {})", String::from_utf8_lossy(&v), f32::from_bits(cv), f32::from_bits(rv), atof(&v)));
                    }
                }
                Op::GetString(k, d) => {
                    let (cv, rv) = (c.bytes(), p.get_ascii_string(k.as_bytes(), d.as_bytes()));
                    compared_ops += 1;
                    if cv != rv {
                        bad = Some(format!("getAsciiString({k}): C++ {cv:?} Rust {rv:?}"));
                    }
                }
                Op::SetBool(k, v) => p.set_bool(k.as_bytes(), *v),
                Op::SetInt(k, v) => p.set_int(k.as_bytes(), *v),
                Op::SetReal(k, v) => set_real(&mut p, k.as_bytes(), *v),
                Op::SetString(k, v) => p.set_ascii_string(k.as_bytes(), v.as_bytes()),
            }
        }
        if let Some(b) = bad {
            mism(b);
            continue;
        }
        let cmap = c.map();
        let rmap: Vec<(Vec<u8>, Vec<u8>)> = p.map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let _ = c.u8();
        let cfile = c.bytes();
        if cmap != rmap || cfile != p.write_text() {
            mism(format!("after the operations: C++ {cmap:?}\nRust {rmap:?}"));
        }
    }
    let _ = std::fs::remove_dir(&dir);
    println!(
        "user preferences: {cases} cases, {lines} lines ({long_lines} longer than 2,047 bytes), {keys} keys loaded, {compared_ops} getter answers compared; {undefined_cases} cases with the empty line's undefined read (compared on the other keys), {non_ascii} cases with isspace of a byte from 0x80, {out_of_range} atoi out of range, {special_reals} getReal of a strtod-only form not compared; {mismatches} mismatches"
    );
    assert_eq!(mismatches, 0, "{}", first.unwrap_or_default());
}
