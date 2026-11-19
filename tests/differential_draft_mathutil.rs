#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the drafts src/drafts/wwlib/math_util.rs and misc_util.rs against (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/mathutil.h, mathutil.cpp, miscutil.h, miscutil.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with the additional terms in NOTICE.md.

//! `cMathUtil` and the pure part of `cMiscUtil` against the C++ (#215, group wwlib2).
//!
//! # Method
//!
//! `differential/wrap_draft_mathutil.cpp` `#include`s mathutil.cpp UNCHANGED and compiles miscutil.cpp's pure functions as
//! VERBATIM cuts (build.rs `drafts_wwlib2`). Stand-ins: `LPCSTR`, `stricmp` (glibc `strcasecmp`), a forward declaration of
//! `StringClass`, and `::rand()` / `RAND_MAX` (the MSVC runtime's 0..32767 generator is not reproducible: the C++ reads the
//! next word of a sequence this test supplies, the Rust draft's `rand` closure reads the same words, and the NUMBER of
//! `rand()` calls is compared as well). `sinf`, `cosf`, `atanf` and `sqrt` are the test binary's interposers.
//!
//! Every output is compared bit for bit (doubles by their bits): `Angle_To_Vector` (dx, dy), `Vector_To_Angle`,
//! `Simple_Distance`, `Round`, `Rotate_Vector`, the six probability-density functions and their `rand()` counts,
//! `Seconds_To_Hms`, `Is_String_Same` / `Is_String_Different`, the four character classes and `Trim_Trailing_Whitespace`.
//! Inputs are not only generic: multiples of 90 degrees and their neighbours (the quadrant boundaries), zero and signed-zero
//! vectors, huge and tiny ratios, rounding half-points and the `MISCUTIL_EPSILON` boundary, `rand` words 0 and 32767.
//!
//! # Excluded inputs
//!
//! `WWASSERT` is compiled out upstream (and the draft's `debug_assert!` is off in release), so nothing is excluded for an
//! assertion. Excluded for undefined behaviour or a representation limit: NaN arguments; `Seconds_To_Hms` of a negative,
//! NaN or above 2.1e9 seconds (`h * 3600` overflows `int`, and the `(int)` cast of the then huge remainder is undefined); `Get_Uniform_Pdf_Int` with `upper - lower + 1 <= 0` (`%` by zero or
//! a negative); strings for `Is_String_*` and `Trim_Trailing_Whitespace` are ASCII without NUL (the draft takes `&str`).
//! `Is_Alphabetic` and friends take every byte 0..=255 (the C `char` is signed; the draft's `char` is the same byte value, and
//! the results agree: false above 0x7F).
//!
//! # Predictions (written before the first run, 2026-11-19)
//!
//! 0 mismatches. The risk I see is `Angle_To_Vector` and `Rotate_Vector`, where upstream passes `double` to `WWMath::Sin(float)`
//! (a narrowing the draft reproduces by hand), and `Round`, where `arg + 0.5f` is a `double` sum.
//!
//! # Results and planted bugs
//!
//! See the `DIFF` lines of the run; the table of planted defects is at the end of this comment block's test
//! (`L9_PLANT=<id>` runs one; ids 14861 to 14879).

mod support;

use rts_lockstep_engine::drafts::wwlib::{math_util, misc_util};
use support::{Check, Gen};

extern "C" {
    fn ref_mu_angle_to_vector(angle: f64, out: *mut f64);
    fn ref_mu_vector_to_angle(dx: f64, dy: f64) -> f64;
    fn ref_mu_simple_distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64;
    fn ref_mu_round(arg: f64) -> i32;
    fn ref_mu_rotate(vx: f64, vy: f64, angle: f64, out: *mut f64);
    fn ref_mu_pdf(op: i32, lower: f64, upper: f64, ilower: i32, iupper: i32, seq: *const i32, nseq: i32, used: *mut i32) -> f64;
    fn ref_mu_seconds_to_hms(seconds: f32, out: *mut i32);
    fn ref_mu_string_cmp(a: *const u8, b: *const u8) -> i32;
    fn ref_mu_char_class(c: i8) -> i32;
    fn ref_mu_trim(text: *mut u8) -> i32;
}

const CASES: usize = 60_000;

fn cases(default: usize) -> usize {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(default)
}

fn plant_id() -> u32 {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
}

fn unit(g: &mut Gen) -> f64 {
    (g.u64() >> 11) as f64 / (1u64 << 53) as f64
}

fn uni(g: &mut Gen, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * unit(g)
}

fn angle(g: &mut Gen) -> f64 {
    match g.below(10) {
        0..=4 => uni(g, 0.0, 360.0),
        5 | 6 => {
            let k = g.below(5) as f64 * 90.0;
            let d = [0.0, 1e-12, -1e-12, 1e-9, -1e-9, 1e-6, -1e-6, 1e-3, -1e-3][g.below(9) as usize];
            k + d
        }
        7 => [-0.00005, -0.00001, 360.00005, 359.9999999999, 89.99999999, 90.00000001, 179.999999999, 269.9999999][g.below(8) as usize],
        _ => uni(g, 0.0, 360.0).floor(),
    }
}

fn comp(g: &mut Gen) -> f64 {
    match g.below(12) {
        0 => 0.0,
        1 => -0.0,
        2 => uni(g, -1.0, 1.0),
        3 => 1e-30 * uni(g, -1.0, 1.0),
        4 => 1e30 * uni(g, -1.0, 1.0),
        5 => (g.below(11) as f64 - 5.0),
        6 => 1e300 * uni(g, -1.0, 1.0),
        7 => 1e-300 * uni(g, -1.0, 1.0),
        _ => uni(g, -100.0, 100.0),
    }
}

fn run_checks(name: &str, c: Check) {
    if plant_id() != 0 {
        let bad = c.report();
        println!("PLANTED {} {name}: {bad} mismatching outputs", plant_id());
        // the assertion that the plant was caught is made once, over all the checks (see `finish_all`)
        CAUGHT.with(|c| c.set(c.get() + bad));
    } else {
        c.finish();
    }
}

thread_local! {
    static CAUGHT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[test]
fn math_util_matches_the_cpp() {
    let mut g = Gen::new(0x215_A101);
    let n = cases(CASES);

    let mut c = Check::new("Angle_To_Vector (dx, dy)");
    for _ in 0..n {
        let a = angle(&mut g);
        let (dx, dy) = math_util::angle_to_vector(a);
        let mut o = [0.0f64; 2];
        unsafe { ref_mu_angle_to_vector(a, o.as_mut_ptr()) };
        c.f64(dx, o[0], || format!("angle {a:e} dx"));
        c.f64(dy, o[1], || format!("angle {a:e} dy"));
    }
    run_checks("angle_to_vector", c);

    let mut c = Check::new("Vector_To_Angle");
    for _ in 0..n {
        let (dx, dy) = match g.below(8) {
            0 => (0.0, comp(&mut g)),
            1 => (comp(&mut g), 0.0),
            2 => {
                let v = comp(&mut g);
                (v, if g.below(2) == 0 { v } else { -v })
            }
            3 => ([0.0, -0.0][g.below(2) as usize], [0.0, -0.0][g.below(2) as usize]),
            _ => (comp(&mut g), comp(&mut g)),
        };
        let r = math_util::vector_to_angle(dx, dy);
        let cpp = unsafe { ref_mu_vector_to_angle(dx, dy) };
        c.f64(r, cpp, || format!("dx {dx:e} dy {dy:e}"));
    }
    run_checks("vector_to_angle", c);

    let mut c = Check::new("Simple_Distance");
    for _ in 0..n {
        let (x1, y1, x2, y2) = (comp(&mut g), comp(&mut g), comp(&mut g), comp(&mut g));
        let r = math_util::simple_distance(x1, y1, x2, y2);
        let cpp = unsafe { ref_mu_simple_distance(x1, y1, x2, y2) };
        c.f64(r, cpp, || format!("{x1:e} {y1:e} {x2:e} {y2:e}"));
    }
    run_checks("simple_distance", c);

    let mut c = Check::new("Round");
    let eps = 0.0001f32 as f64;
    for _ in 0..n {
        let a = match g.below(10) {
            0 => (g.below(2001) as f64 - 1000.0) * 0.5,
            1 => [eps, -eps, eps * 1.0000001, -eps * 1.0000001, eps * 0.9999999, 0.0, -0.0, 0.5, -0.5, 0.49999999999, -0.49999999999][g.below(11) as usize],
            2 => uni(&mut g, -1.0, 1.0),
            3 => uni(&mut g, -1e9, 1e9),
            _ => uni(&mut g, -1000.0, 1000.0),
        };
        let r = math_util::round(a);
        let cpp = unsafe { ref_mu_round(a) };
        c.eq(r, cpp, || format!("arg {a:e}"));
    }
    run_checks("round", c);

    let mut c = Check::new("Rotate_Vector (vx, vy)");
    for _ in 0..n {
        let (vx, vy) = (uni(&mut g, -100.0, 100.0), uni(&mut g, -100.0, 100.0));
        let a = match g.below(4) {
            0 => g.below(9) as f64 * 90.0 - 360.0,
            1 => uni(&mut g, -720.0, 720.0),
            _ => uni(&mut g, 0.0, 360.0),
        };
        let (mut x, mut y) = (vx, vy);
        math_util::rotate_vector(&mut x, &mut y, a);
        let mut o = [0.0f64; 2];
        unsafe { ref_mu_rotate(vx, vy, a, o.as_mut_ptr()) };
        c.f64(x, o[0], || format!("v ({vx:e}, {vy:e}) angle {a:e} x"));
        c.f64(y, o[1], || format!("v ({vx:e}, {vy:e}) angle {a:e} y"));
    }
    run_checks("rotate_vector", c);

    // the six probability-density functions, with the words ::rand() returns supplied by the test
    let names = ["Get_Uniform_Pdf_Double", "Get_Normalized_Uniform_Pdf_Double", "Get_Uniform_Pdf_Int", "Get_Hat_Pdf_Double", "Get_Normalized_Hat_Pdf_Double", "Get_Hat_Pdf_Int"];
    for op in 0..6 {
        let mut c = Check::new(names[op as usize]);
        let mut calls = Check::new(&format!("{} rand() calls", names[op as usize]));
        for _ in 0..n {
            let seq: Vec<i32> = (0..8)
                .map(|_| match g.below(8) {
                    0 => 0,
                    1 => 32767,
                    2 => 16383 + g.below(3) as i32,
                    _ => g.below(32768) as i32,
                })
                .collect();
            let (lo, hi) = match g.below(8) {
                0 => {
                    let v = uni(&mut g, -50.0, 50.0);
                    (v, v)
                }
                1 => {
                    let v = uni(&mut g, -50.0, 50.0);
                    (v, v + 0.00005)
                }
                2 => {
                    let v = uni(&mut g, -50.0, 50.0);
                    (v, v + 0.0001)
                }
                _ => {
                    let a = uni(&mut g, -1000.0, 1000.0);
                    (a, a + uni(&mut g, 0.0, 2000.0))
                }
            };
            let (ilo, ihi) = match g.below(6) {
                0 => {
                    let v = g.below(100) as i32 - 50;
                    (v, v)
                }
                1 => (g.below(2_000_000_000) as i32 - 1_000_000_000, 1_000_000_000),
                _ => {
                    let a = g.below(2001) as i32 - 1000;
                    (a, a + g.below(300) as i32)
                }
            };
            let (lo, hi) = if op == 2 || op == 5 { (ilo as f64, ihi as f64) } else { (lo, hi) };
            let mut used = 0usize;
            let mut rand = || {
                let w = seq.get(used).copied().unwrap_or(0);
                used += 1;
                w
            };
            let r = match op {
                0 => math_util::get_uniform_pdf_double(lo, hi, &mut rand),
                1 => math_util::get_normalized_uniform_pdf_double(&mut rand),
                2 => math_util::get_uniform_pdf_int(ilo, ihi, &mut rand) as f64,
                3 => math_util::get_hat_pdf_double(lo, hi, &mut rand),
                4 => math_util::get_normalized_hat_pdf_double(&mut rand),
                _ => math_util::get_hat_pdf_int(ilo, ihi, &mut rand) as f64,
            };
            let mut cused = 0i32;
            let cpp = unsafe { ref_mu_pdf(op, lo, hi, ilo, ihi, seq.as_ptr(), seq.len() as i32, &mut cused) };
            c.f64(r, cpp, || format!("lo {lo:e} hi {hi:e} ints ({ilo}, {ihi}) seq {seq:?}"));
            calls.eq(used as i32, cused, || format!("lo {lo:e} hi {hi:e} ints ({ilo}, {ihi}) seq {seq:?}"));
        }
        run_checks(names[op as usize], c);
        run_checks("rand calls", calls);
    }
    finish_all(14861, 14874);
}

fn finish_all(lo: u32, hi: u32) {
    if plant_id() >= lo && plant_id() <= hi {
        assert!(CAUGHT.with(|c| c.get()) > 0, "plant {} not caught", plant_id());
    }
}

fn ascii_string(g: &mut Gen) -> Vec<u8> {
    let len = g.below(8) as usize;
    (0..len)
        .map(|_| match g.below(6) {
            0 => b' ',
            1 => b'\t',
            2 => b'a' + g.below(26) as u8,
            3 => b'A' + g.below(26) as u8,
            4 => b'0' + g.below(10) as u8,
            _ => 1 + g.below(127) as u8,
        })
        .collect()
}

#[test]
fn misc_util_matches_the_cpp() {
    let mut g = Gen::new(0x215_A102);
    let n = cases(CASES);

    let mut c = Check::new("Seconds_To_Hms (h, m, s)");
    for _ in 0..n {
        let secs = match g.below(8) {
            0 => g.below(100_000) as f32,
            1 => [0.0, 59.999, 60.0, 3599.9998, 3600.0, 3599.0, 86399.0, 1.0e-30, 0.5][g.below(9) as usize],
            2 => g.uniform(0.0, 2.1e9),
            3 => g.uniform(0.0, 1.0e6),
            _ => g.uniform(0.0, 20_000.0),
        };
        let (h, m, s) = misc_util::seconds_to_hms(secs);
        let mut o = [0i32; 3];
        unsafe { ref_mu_seconds_to_hms(secs, o.as_mut_ptr()) };
        c.eq((h, m, s), (o[0], o[1], o[2]), || format!("seconds {secs:e}"));
    }
    run_checks("seconds_to_hms", c);

    let mut c = Check::new("Is_String_Same / Is_String_Different");
    for _ in 0..n {
        let a = ascii_string(&mut g);
        let b = match g.below(4) {
            0 => a.clone(),
            1 => a.iter().map(|&x| if g.below(2) == 0 { x.to_ascii_uppercase() } else { x.to_ascii_lowercase() }).collect(),
            2 => {
                let mut v = a.clone();
                v.push(b'x');
                v
            }
            _ => ascii_string(&mut g),
        };
        let (sa, sb) = (String::from_utf8(a.clone()).unwrap(), String::from_utf8(b.clone()).unwrap());
        let (mut ca, mut cb) = (a.clone(), b.clone());
        ca.push(0);
        cb.push(0);
        let cpp = unsafe { ref_mu_string_cmp(ca.as_ptr(), cb.as_ptr()) };
        let r = (misc_util::is_string_same(&sa, &sb) as i32) | ((misc_util::is_string_different(&sa, &sb) as i32) << 1);
        c.eq(r, cpp, || format!("{sa:?} {sb:?}"));
    }
    run_checks("string compare", c);

    let mut c = Check::new("Is_Alphabetic / Numeric / Alphanumeric / Whitespace");
    for round in 0..(n / 256).max(1) {
        for b in 0..=255u8 {
            let ch = b as char;
            let r = (misc_util::is_alphabetic(ch) as i32) | ((misc_util::is_numeric(ch) as i32) << 1) | ((misc_util::is_alphanumeric(ch) as i32) << 2) | ((misc_util::is_whitespace(ch) as i32) << 3);
            let cpp = unsafe { ref_mu_char_class(b as i8) };
            c.eq(r, cpp, || format!("byte {b:#04x} round {round}"));
        }
    }
    run_checks("char classes", c);

    let mut c = Check::new("Trim_Trailing_Whitespace");
    for _ in 0..n {
        let mut v = ascii_string(&mut g);
        for _ in 0..g.below(4) {
            v.push(if g.below(2) == 0 { b' ' } else { b'\t' });
        }
        let mut s = String::from_utf8(v.clone()).unwrap();
        misc_util::trim_trailing_whitespace(&mut s);
        let mut buf = v.clone();
        buf.push(0);
        let len = unsafe { ref_mu_trim(buf.as_mut_ptr()) } as usize;
        c.eq(s.as_bytes().to_vec(), buf[..len].to_vec(), || format!("{:?}", String::from_utf8_lossy(&v)));
    }
    run_checks("trim", c);
    finish_all(14875, 14879);
}
