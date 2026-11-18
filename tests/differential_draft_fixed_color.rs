#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/fixed.h, fixed.cpp, hsv.h, hsv.cpp, RGB.H, rgb.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::wwlib::fixed` (class `fixed`), `hsv` and `rgb` against the upstream C++ (#215).
//!
//! # Method
//!
//! The C++ side is fixed.cpp, hsv.cpp and rgb.cpp `#include`d unchanged by `differential/wrap_draft_fixed.cpp` and
//! `wrap_draft_color.cpp`, with the inline members of fixed.h, hsv.h and RGB.H. Every operation of `fixed` is run on raw
//! words chosen to hit the edges (0, 1, 127, 128, 255, 256, 257, 0xFF00, 0xFFFF, random), integers (negative, large, the
//! multiples of 2^24), and unsigned values; the answer (the raw word, the int, or the bool) is compared. The colour
//! conversions are run on ALL 2^24 colours; Adjust and Difference on random colour pairs and ratios (any `int`).
//!
//! Excluded inputs (undefined in C++, which divides by zero or shifts by 32 or more): `fixed /= fixed` by a divisor whose
//! rounded whole number is 0 (raw 1 to 127); `fixed / int` by a multiple of 2^24 (the divisor `rvalue * 256` wraps to 0); shifts
//! by 32 or more; `fixed(char const *)` text with a number of more than 9 digits (`atoi` overflow is undefined) or none of
//! the unspecified bytes outside ASCII; `To_ASCII` with a `maxlen` below -1.
//!
//! # Predictions (written before the first run, 2026-11-18) and results
//!
//! Predicted: 0 mismatches for hsv and rgb (integer code, upstream's order) and for `fixed`. Results: HSV and RGB conversions on
//! all 16,777,216 colours each 0; Adjust and Difference 2,000,000 pairs each 0; `fixed` 65 operations x 40,000 cases, `fixed(char
//! const *)` 100,000 texts, `To_ASCII` and `As_ASCII` 100,000 values.
//!
//! **Prediction missed (fixed):** (1) first run 377,147 of 2,600,000 operation cases and 82,020 of 100,000 texts differed. Cause
//! of most: the wrapper, not the draft: glibc defines `BIG_ENDIAN`, so fixed.h's `Whole`/`Fraction` were laid out big-endian
//! (the wrapper now undefines it; listed there). (2) After that 32,013 of 40,000 `fixed /= fixed` cases differed: a **draft defect**,
//! corrected in `fixed.rs`: upstream's `((int)Raw * 256) / rvalue` selects the friend `operator /(int, fixed const &)`
//! (`((unsigned)(lvalue * 256) + 128) / rvalue.Raw`), not the conversion of `rvalue` to `unsigned` that the draft's doc comment
//! called "an upstream defect" (and which divided by zero for raw 1 to 127).
//! `mul` and `*=` of raw words above 46,340 overflow `int` (wrapped, as MSVC); with this profile's checks the plain `*` also passed
//! (plant 10771, the original plain `*`: 0 mismatches), so `wrapping_mul` is a safety change only.
//!
//! **Upstream defect (not in the draft, reported in #210/#211):** `fixed::To_ASCII` ends with `strncpy(buffer, tbuffer, maxlen)`, which
//! writes no terminating NUL when the text has `maxlen` or more characters (32,939 of 100,000 calls here), and returns -1 for `maxlen
//! == 0`. The draft returns the text cut to `maxlen - 1` characters, equal to the returned count; compared by the bytes the C++
//! returns the count of.
//!
//! **RGB to HSV before the fix (Euler, in the curves test)** is in that test. Planted bugs (cases of 2,600,000 unless noted): 10705
//! signed division in `fixed(int,int)`: 14,684; 10706 round `+127`: 950; 10707 `>> 8` in `*=`: 1,899; 10708 `!= 255` in `/`:
//! 2,080; 10709 `*int` unrounded: 25,291; 10710 `Round` `> 128`: 1,725; 10711 `Sub_Saturate` `>`: 231; 10712 `To_ASCII` `/255`: 9,300 of 83,242
//! (text 42,616); 10713 percent `*255`: 9,550 of 100,000 texts; 10714 `Round_Up` `+256`: 7,329. HSV (of 2,000,000 pairs or
//! 16,777,216 colours): 10715 Adjust mask 0x7F: 926,176; 10716 Difference without the square: 1,976,621; 10717 `hue % 256`: 7,256,886;
//! 10718 `values[5] = values[3]`: 7,933,395; 10719 sector `i > 5`: caught by an index panic at the first hit (not counted). RGB:
//! 10720 Adjust `>> 8`: 978,875; 10721 Difference weights: 1,989,445; 10722 saturation `*256`: 11,096,058; 10723 hue rounding:
//! 8,414,590; 10724 `5*256 + g1`: 2,756,375; 10725 green branch skipped: 5,559,680. 21 of 21 caught (10771 is an equivalent mutant).
//!
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose). See the tables at the end of each test's header.

mod support;

use rts_lockstep_engine::drafts::wwlib::fixed::*;
use rts_lockstep_engine::drafts::wwlib::hsv::HSVClass;
use rts_lockstep_engine::drafts::wwlib::rgb::RGBClass;
use support::{Check, Gen};

extern "C" {
    fn ref_fixed_op(op: i32, a: u32, b: u32, i: i32, u: u32) -> i32;
    fn ref_fixed_from_ascii(text: *const std::os::raw::c_char) -> i32;
    fn ref_fixed_to_ascii(raw: u32, maxlen: i32, buf: *mut u8) -> i32;
    fn ref_fixed_as_ascii(raw: u32, buf: *mut u8);
    fn ref_rgb_to_hsv(c: u32) -> u32;
    fn ref_hsv_to_rgb(c: u32) -> u32;
    fn ref_rgb_adjust(c: u32, ratio: i32, to: u32) -> u32;
    fn ref_hsv_adjust(c: u32, ratio: i32, to: u32) -> u32;
    fn ref_rgb_difference(a: u32, b: u32) -> i32;
    fn ref_hsv_difference(a: u32, b: u32) -> i32;
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_CODEC_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn rust<T>(f: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok()
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

fn raw_word(g: &mut Gen) -> u32 {
    const EDGE: [u32; 14] = [0, 1, 2, 127, 128, 129, 255, 256, 257, 511, 512, 0xFF00, 0xFFFF, 0x8000];
    match g.below(4) {
        0 => EDGE[g.below(EDGE.len() as u32) as usize],
        1 => g.below(1024),
        _ => g.below(65536),
    }
}

fn int_word(g: &mut Gen) -> i32 {
    match g.below(8) {
        0 => 0,
        1 => g.below(20) as i32 - 5,
        2 => g.below(2000) as i32 - 1000,
        3 => (g.below(40) as i32 - 20) << 24,
        4 => i32::MAX - g.below(3) as i32,
        5 => i32::MIN + g.below(3) as i32,
        _ => g.i32(),
    }
}

fn uns_word(g: &mut Gen) -> u32 {
    match g.below(5) {
        0 => g.below(40),
        1 => 255 + g.below(3),
        2 => 0xFFFF_FFFF - g.below(3),
        3 => 1 << (g.below(32)),
        _ => g.u32(),
    }
}

/// (the operation's name, number) as in the wrapper; the Rust side below.
const OPS: [&str; 65] = [
    "fixed(int,int)", "fixed(int)", "operator unsigned", "*= fixed", "*= int", "/= fixed", "/= int", "+= fixed", "-= fixed", "* fixed", "* int", "/ fixed", "/ int", "+ fixed", "+ int", "- fixed",
    "- int", ">>=", "<<=", "!", "< int", "> int", "<= int", ">= int", "== int", "!= int", "Round_Up", "Round_Down", "Round", "Saturate(unsigned)", "Saturate(fixed)", "Sub_Saturate(unsigned)",
    "Sub_Saturate(fixed)", "Inverse", "int / fixed", "int - fixed", "int * fixed", "int + fixed", "unsigned < fixed", "unsigned > fixed", "unsigned <= fixed", "unsigned >= fixed", "unsigned == fixed",
    "unsigned != fixed", "fixed < fixed", "fixed > fixed", "fixed <= fixed", "fixed >= fixed", "fixed == fixed", "fixed != fixed", "Round_Up(f)", "Round_Down(f)", "Round(f)", "Saturate(f, u)",
    "Sub_Saturate(f, u)", "Inverse(f)", "_1_2", "_1_3", "_1_4", "_3_4", "_2_3", ">>", "<<", "Saturate(f, f)", "Sub_Saturate(f, f)",
];

fn rust_op(op: i32, a: u32, b: u32, i: i32, u: u32) -> i32 {
    let x = Fixed { raw: a as u16 };
    let y = Fixed { raw: b as u16 };
    let mut t = x;
    let r = |f: Fixed| f.raw as i32;
    match op {
        0 => r(Fixed::from_ratio(i, u as i32)),
        1 => r(Fixed::from_int(i)),
        2 => x.to_unsigned() as i32,
        3 => {
            t.mul_assign(y);
            r(t)
        }
        4 => {
            t.mul_assign_int(i);
            r(t)
        }
        5 => {
            t.div_assign(y);
            r(t)
        }
        6 => {
            t.div_assign_int(i);
            r(t)
        }
        7 => {
            t.add_assign(y);
            r(t)
        }
        8 => {
            t.sub_assign(y);
            r(t)
        }
        9 => r(x.mul(y)),
        10 => x.mul_int(i),
        11 => r(x.div(y)),
        12 => x.div_int(i),
        13 => r(x.add(y)),
        14 => x.add_int(i),
        15 => r(x.sub(y)),
        16 => x.sub_int(i),
        17 => {
            t.shr_assign(u);
            r(t)
        }
        18 => {
            t.shl_assign(u);
            r(t)
        }
        19 => x.is_zero() as i32,
        20 => x.lt_int(i) as i32,
        21 => x.gt_int(i) as i32,
        22 => x.le_int(i) as i32,
        23 => x.ge_int(i) as i32,
        24 => x.eq_int(i) as i32,
        25 => x.ne_int(i) as i32,
        26 => {
            t.round_up();
            r(t)
        }
        27 => {
            t.round_down();
            r(t)
        }
        28 => {
            t.round();
            r(t)
        }
        29 => {
            t.saturate(u);
            r(t)
        }
        30 => {
            t.saturate_fixed(y);
            r(t)
        }
        31 => {
            t.sub_saturate(u);
            r(t)
        }
        32 => {
            t.sub_saturate_fixed(y);
            r(t)
        }
        33 => {
            t.inverse();
            r(t)
        }
        34 => int_div_fixed(i, y),
        35 => int_sub_fixed(i, y),
        36 => int_mul_fixed(i, y),
        37 => int_add_fixed(i, y),
        38 => unsigned_lt_fixed(u, y) as i32,
        39 => unsigned_gt_fixed(u, y) as i32,
        40 => unsigned_le_fixed(u, y) as i32,
        41 => unsigned_ge_fixed(u, y) as i32,
        42 => unsigned_eq_fixed(u, y) as i32,
        43 => unsigned_ne_fixed(u, y) as i32,
        44 => (x < y) as i32,
        45 => (x > y) as i32,
        46 => (x <= y) as i32,
        47 => (x >= y) as i32,
        48 => (x == y) as i32,
        49 => (x != y) as i32,
        50 => r(fixed_round_up(x)),
        51 => r(fixed_round_down(x)),
        52 => r(fixed_round(x)),
        53 => r(fixed_saturate(x, u)),
        54 => r(fixed_sub_saturate(x, u)),
        55 => r(fixed_inverse(x)),
        56 => r(Fixed::one_half()),
        57 => r(Fixed::one_third()),
        58 => r(Fixed::one_quarter()),
        59 => r(Fixed::three_quarters()),
        60 => r(Fixed::two_thirds()),
        61 => {
            t.shr_assign(u);
            r(t)
        }
        62 => {
            t.shl_assign(u);
            r(t)
        }
        63 => {
            t.saturate_fixed(y);
            r(t)
        }
        _ => {
            t.sub_saturate_fixed(y);
            r(t)
        }
    }
}

#[test]
fn fixed_operations_match_upstream() {
    let n = cases(40_000);
    let mut g = Gen::new(0x215_7001);
    let mut checks: Vec<Check> = OPS.iter().map(|name| Check::new(&format!("fixed {name}"))).collect();
    let mut panics = vec![0u64; OPS.len()];
    for k in 0..n {
        for op in 0..OPS.len() as i32 {
            let a = raw_word(&mut g);
            let mut b = raw_word(&mut g);
            let mut i = int_word(&mut g);
            let mut u = uns_word(&mut g);
            // excluded inputs
            if op == 5 && b != 0 && b != 256 && (b + 128) / 256 == 0 {
                b = 256 + g.below(5000);
            }
            if op == 12 && i != 0 && (i as u32).wrapping_mul(256) == 0 {
                i = 3;
            }
            if (op == 17 || op == 18 || op == 61 || op == 62) && u >= 32 {
                u %= 32;
            }
            if op == 0 && u == 0 && g.below(2) == 0 {
                u = 1 + g.below(300);
            }
            let cpp = unsafe { ref_fixed_op(op, a, b, i, if op == 0 { u } else { u }) };
            let rs = rust(|| rust_op(op, a, b, i, u));
            if rs.is_none() {
                panics[op as usize] += 1;
            }
            checks[op as usize].eq(rs, Some(cpp), || format!("{} a={a:#x} b={b:#x} i={i} u={u} (case {k})", OPS[op as usize]));
        }
    }
    let total: u64 = checks.iter().map(|c| c.mismatches).sum();
    let compared: u64 = checks.iter().map(|c| c.compared).sum();
    for c in &checks {
        if c.mismatches > 0 {
            println!("{}: {} of {} differ; first {:?}", c.name, c.mismatches, c.compared, c.examples.first());
        }
    }
    println!("{} operations, {n} cases each, {compared} comparisons, {total} mismatches; Rust panics (overflow or out of range) per operation: {:?}", OPS.len(), panics.iter().enumerate().filter(|(_, &p)| p > 0).collect::<Vec<_>>());
    let mut all = Check::new("fixed operations (all)");
    all.compared = compared;
    all.mismatches = total;
    all.examples = checks.iter().flat_map(|c| c.examples.iter().cloned()).take(8).collect();
    finish(all);
}

fn gen_text(g: &mut Gen) -> String {
    let mut s = String::new();
    for _ in 0..g.below(3) {
        s.push([' ', '\t', '\n', '\x0b', '\x0c', '\r'][g.below(6) as usize]);
    }
    if g.below(6) == 0 {
        s.push(if g.below(2) == 0 { '-' } else { '+' });
    }
    let digits = |g: &mut Gen, s: &mut String, max: u32| {
        for _ in 0..g.below(max + 1) {
            s.push((b'0' + g.below(10) as u8) as char);
        }
    };
    digits(g, &mut s, 9);
    match g.below(8) {
        0 | 1 | 2 => {
            s.push('.');
            digits(g, &mut s, 9);
        }
        3 => s.push('%'),
        4 => {
            s.push('.');
            digits(g, &mut s, 4);
            s.push('.');
            digits(g, &mut s, 3);
        }
        5 => {
            for _ in 0..1 + g.below(4) {
                s.push((0x20 + g.below(95) as u8) as char);
            }
        }
        _ => {}
    }
    if g.below(10) == 0 {
        s.push('%');
    }
    s
}

#[test]
fn fixed_from_ascii_matches_upstream() {
    let n = cases(100_000);
    let mut g = Gen::new(0x215_7002);
    let mut c = Check::new("fixed(char const *)");
    let mut percents = 0u64;
    for k in 0..n {
        let none = k % 997 == 0;
        let text = if none { String::new() } else { gen_text(&mut g) };
        let cs = std::ffi::CString::new(text.clone()).expect("no NUL");
        let cpp = unsafe { ref_fixed_from_ascii(if none { std::ptr::null() } else { cs.as_ptr() }) };
        let rs = rust(|| Fixed::from_ascii(if none { None } else { Some(&text) }).raw as i32);
        if text.contains('%') {
            percents += 1;
        }
        c.eq(rs, Some(cpp), || format!("text {text:?}"));
    }
    println!("{n} texts, {percents} with a percent sign");
    finish(c);
}

#[test]
fn fixed_to_ascii_matches_upstream() {
    let n = cases(100_000);
    let mut g = Gen::new(0x215_7003);
    let mut c = Check::new("fixed To_ASCII (text)");
    let mut c_len = Check::new("fixed To_ASCII (returned count)");
    let mut c_as = Check::new("fixed As_ASCII");
    let mut unterminated = 0u64;
    for k in 0..n {
        let raw = if g.below(4) == 0 { (g.below(300)) * 1 } else { g.below(65536) };
        let maxlen = match g.below(6) {
            0 => -1,
            1 => 0,
            _ => 1 + g.below(12) as i32,
        };
        let cap = if maxlen == -1 { 64 } else { maxlen.max(1) as usize };
        let mut buf = vec![0x7Eu8; cap];
        let ret = unsafe { ref_fixed_to_ascii(raw, maxlen, buf.as_mut_ptr()) };
        let want = buf[..ret.max(0) as usize].to_vec();
        let f = Fixed { raw: raw as u16 };
        let rs = rust(|| f.to_ascii(maxlen));
        c.eq(rs.clone().map(|s| s.into_bytes()), Some(want), || format!("raw {raw:#x} maxlen {maxlen}"));
        if maxlen != 0 {
            c_len.eq(rs.map(|s| s.len() as i32), Some(ret), || format!("raw {raw:#x} maxlen {maxlen}"));
        }
        // strncpy leaves the buffer unterminated when the text is maxlen characters or longer
        if maxlen > 0 && !buf.contains(&0) {
            unterminated += 1;
        }
        let mut sb = vec![0u8; 32];
        unsafe { ref_fixed_as_ascii(raw, sb.as_mut_ptr()) };
        let end = sb.iter().position(|&b| b == 0).unwrap_or(32);
        c_as.eq(rust(|| f.as_ascii().into_bytes()), Some(sb[..end].to_vec()), || format!("raw {raw:#x} (case {k})"));
    }
    println!("{n} values; UPSTREAM DEFECT (not in the draft): To_ASCII's strncpy leaves the buffer without a terminating NUL in {unterminated} of those calls (text of maxlen characters or more); the draft returns the text cut to maxlen - 1 characters, the count To_ASCII returns");
    c_len.finish_or_report();
    c_as.finish_or_report();
    finish(c);
}

trait FinishOrReport {
    fn finish_or_report(self);
}
impl FinishOrReport for Check {
    fn finish_or_report(self) {
        if planted() {
            self.report();
        } else {
            self.finish();
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// HSV and RGB
// ---------------------------------------------------------------------------------------------------------------------------

fn unpack(c: u32) -> (u8, u8, u8) {
    (c as u8, (c >> 8) as u8, (c >> 16) as u8)
}
fn pack(a: i32, b: i32, c: i32) -> u32 {
    (a as u32 & 255) | ((b as u32 & 255) << 8) | ((c as u32 & 255) << 16)
}

#[test]
fn rgb_to_hsv_and_hsv_to_rgb_match_upstream_on_every_colour() {
    let mut c1 = Check::new("RGBClass::operator HSVClass (all 2^24 colours)");
    let mut c2 = Check::new("HSVClass::operator RGBClass (all 2^24 colours)");
    let limit: u32 = std::env::var("DRAFT_COLOR_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(1 << 24);
    for col in 0..limit {
        let (r, g, b) = unpack(col);
        let cpp = unsafe { ref_rgb_to_hsv(col) };
        let h = RGBClass::new(r, g, b).to_hsv();
        c1.eq(pack(h.get_hue(), h.get_saturation(), h.get_value()), cpp, || format!("rgb ({r},{g},{b})"));
        let cpp = unsafe { ref_hsv_to_rgb(col) };
        let x = HSVClass::new(r, g, b).to_rgb();
        c2.eq(pack(x.get_red(), x.get_green(), x.get_blue()), cpp, || format!("hsv ({r},{g},{b})"));
    }
    c1.finish_or_report();
    finish(c2);
}

#[test]
fn color_adjust_and_difference_match_upstream() {
    let n = cases(2_000_000);
    let mut g = Gen::new(0x215_7004);
    let mut c_ra = Check::new("RGBClass::Adjust");
    let mut c_rd = Check::new("RGBClass::Difference");
    let mut c_ha = Check::new("HSVClass::Adjust");
    let mut c_hd = Check::new("HSVClass::Difference");
    for _ in 0..n {
        let a = g.u32() & 0xFF_FFFF;
        let b = g.u32() & 0xFF_FFFF;
        let ratio = match g.below(6) {
            0 => g.below(256) as i32,
            1 => g.below(520) as i32 - 260,
            2 => g.i32(),
            _ => g.below(300) as i32,
        };
        let (ar, ag, ab) = unpack(a);
        let (br, bg, bb) = unpack(b);
        let cpp = unsafe { ref_rgb_adjust(a, ratio, b) };
        let rs = rust(|| {
            let mut x = RGBClass::new(ar, ag, ab);
            x.adjust(ratio, &RGBClass::new(br, bg, bb));
            pack(x.get_red(), x.get_green(), x.get_blue())
        });
        c_ra.eq(rs, Some(cpp), || format!("({ar},{ag},{ab}) -> ({br},{bg},{bb}) ratio {ratio}"));
        let cpp = unsafe { ref_rgb_difference(a, b) };
        let rs = rust(|| RGBClass::new(ar, ag, ab).difference(&RGBClass::new(br, bg, bb)));
        c_rd.eq(rs, Some(cpp), || format!("({ar},{ag},{ab}) vs ({br},{bg},{bb})"));
        let cpp = unsafe { ref_hsv_adjust(a, ratio, b) };
        let rs = rust(|| {
            let mut x = HSVClass::new(ar, ag, ab);
            x.adjust(ratio, &HSVClass::new(br, bg, bb));
            pack(x.get_hue(), x.get_saturation(), x.get_value())
        });
        c_ha.eq(rs, Some(cpp), || format!("({ar},{ag},{ab}) -> ({br},{bg},{bb}) ratio {ratio}"));
        let cpp = unsafe { ref_hsv_difference(a, b) };
        let rs = rust(|| HSVClass::new(ar, ag, ab).difference(&HSVClass::new(br, bg, bb)));
        c_hd.eq(rs, Some(cpp), || format!("({ar},{ag},{ab}) vs ({br},{bg},{bb})"));
    }
    c_ra.finish_or_report();
    c_rd.finish_or_report();
    c_ha.finish_or_report();
    finish(c_hd);
}
