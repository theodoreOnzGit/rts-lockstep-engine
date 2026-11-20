#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/ini.cpp (INIClass members, below), trim.cpp (strtrim)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The typed `INIClass` accessors of `drafts::sweep_wwlib::ini` against the upstream C++ (#215, group wwlib3).
//!
//! # Method
//!
//! `differential/wrap_draft_ini.cpp` holds the upstream members cut verbatim (list in its header) over a stand-in `INIClass` (a vector of
//! sections of `(entry, value)` pairs; `Put_String` removes an existing entry and appends a non-empty one; the test's own store below is the
//! same). A program of 1 to 40 random ops on one database: `Put_String`, `Put_Int` (the three formats), `Put_Rect`, `Put_Hex`, `Put_Float`,
//! `Put_Double`, `Put_Bool`, the three `Put_Point`s, `Put_TextBlock`, and the reads `Get_TextBlock`, `Get_Int`, `Get_Hex`, `Get_Float`,
//! `Get_Double`, `Get_Rect`, `Get_Bool`, the four `Get_Point`s, `Get_List_Index`, `Get_Int_Bitfield`, `Get_Alloc_Int_Array`,
//! `Get_Alloc_String`, `Get_String` (the buffer form) and `Enumerate_Entries`. Values are drawn from number-like, hex, boolean, list and
//! garbage strings; floats and doubles are compared by bits. After each op the result and the whole database are compared. 20,000 programs,
//! fixed seed.
//!
//! Not generated: `Get_Point` on a value that does not hold enough numbers (upstream leaves the `int`/`float` locals uninitialised), so the
//! point entries `q2` and `q3` only receive two and three numbers; `Put_Float` of NaN or infinity (C prints `nan`/`-nan`/`inf`; the draft
//! says it differs); `Put_Point(TPoint3D<float>)` with three floats of 1e5 or more in magnitude (its 54-byte buffer overflows); `Put_Double` of 1e50 or more (its `%f` overflows the 64-byte buffer); `Get_Double` (upstream scans `%lf` into a 4-byte `float`: the run aborts with stack smashing, so no defined C++ result exists; the draft returns the intended value); `Get_Alloc_Int_Array` with a `listend` that a token equals. Not covered: `Put_UUBlock`, `Get_UUBlock` (both forms), `Get_Wide_String`,
//! `Put_Wide_String` (streams, Base64 pipes and the wide string class), and `INIClass`'s own `Put_String`/`Find_Entry` (the verified
//! `ww::ini` port covers `Get_String`'s lookup).
//!
//! # Predictions and results
//!
//! Predicted before the first run: mismatches in `Get_Float`/`Get_Double` on long digit strings (the draft parses a double and narrows it, C
//! `sscanf("%f")` rounds once) and in `Get_Hex`/`Get_Int` on values beyond `i32` (`sscanf %x` and `atoi` overflow differently); all else 0.

//! **Results.** Prediction: the double rounding in `Get_Float` (confirmed, below); the integer overflow cases (not confirmed: excluded from the
//! generator instead, see above). First runs were dominated by the test's own stand-ins, recorded here because they are what a wrapper over C library
//! calls runs into: glibc's vectorised `strcpy` garbled `strtrim`'s overlapping copy (replaced by the forward byte copy MSVC's has, as the unicode
//! wrapper does), `Put_Double`/`Put_Point` overflowed their `%f` buffers (the 64- and 54-byte arrays), `Get_Point` on a non-numeric value read
//! uninitialised locals, and `atoi`/`%x` of an out-of-range number differs between libraries. After excluding those inputs (4,616, then 71, then 22
//! of 20,000 programs mismatching, none a draft defect) the only draft defect was `Get_Float`'s double rounding: 16 of 20,000 programs on the
//! halfway decimal texts. Corrected: 0 of 20,000 programs mismatching (about 400,000 ops). Planted bugs (mismatching programs of 20,000): 17411 `Put_Int` format 1
//! lower case: 3,254; 17412 format 2 without `$`: 3,257; 17413 `Get_Int` `h` suffix case-sensitive: 3; 17414 `Put_Rect` width and height swapped: 9,708;
//! 17415 `Get_Rect` skips the third number: 9,266; 17416 `Put_Hex` lower case: 9,517; 17417 `Get_Bool` `N`/`F`/`0` true: 201; 17418 `Get_Float` no `%`/100: 10;
//! 17419 `Get_TextBlock` stops at `len < 1`: 1,816; 17420 `Get_Alloc_Int_Array` keeps empty tokens: 1; 17421 `Get_Float` double rounding (the old behaviour): 8;
//! 17422 `Put_TextBlock` `count > 75`: 4,967; 17423 `Put_Point` float five decimals: 9,664. 13 of 13 caught. (17412 was first `{:X}` of the signed number, an
//! equivalent mutant, and was replaced; 17418 was first missed, no generated value held a `%`, which the generator now does.)

mod support;

use rts_lockstep_engine::drafts::sweep_wwlib::ini::*;
use support::{Check, Gen};

extern "C" {
    fn ref_ini_run(prog: *const i32, nops: i32, out: *mut i32, cap: i32) -> i32;
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_INI_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// The test's database: the stand-in of the C++ wrapper.
#[derive(Default)]
struct Store {
    secs: Vec<(Vec<u8>, Vec<(Vec<u8>, Vec<u8>)>)>,
}

impl IniRead for Store {
    fn find_value(&self, section: &[u8], entry: &[u8]) -> Option<Vec<u8>> {
        let s = self.secs.iter().find(|s| s.0 == section)?;
        s.1.iter().find(|e| e.0 == entry).map(|e| e.1.clone())
    }
    fn entry_count(&self, section: &[u8]) -> i32 {
        self.secs.iter().find(|s| s.0 == section).map_or(0, |s| s.1.len() as i32)
    }
    fn get_entry(&self, section: &[u8], index: i32) -> Option<Vec<u8>> {
        let s = self.secs.iter().find(|s| s.0 == section)?;
        if index < 0 {
            return None;
        }
        s.1.get(index as usize).map(|e| e.0.clone())
    }
}

impl IniWrite for Store {
    fn put_string(&mut self, section: &[u8], entry: &[u8], string: &[u8]) -> bool {
        if section.is_empty() || entry.is_empty() {
            return false;
        }
        let i = match self.secs.iter().position(|s| s.0 == section) {
            Some(i) => i,
            None => {
                self.secs.push((section.to_vec(), Vec::new()));
                self.secs.len() - 1
            }
        };
        let s = &mut self.secs[i].1;
        if let Some(j) = s.iter().position(|e| e.0 == entry) {
            s.remove(j);
        }
        if !string.is_empty() {
            s.push((entry.to_vec(), string.to_vec()));
        }
        true
    }
    fn clear_section(&mut self, section: &[u8]) -> bool {
        if let Some(i) = self.secs.iter().position(|s| s.0 == section) {
            self.secs.remove(i);
        }
        true
    }
}

fn dump(out: &mut Vec<i32>, st: &Store) {
    out.push(st.secs.len() as i32);
    for (name, entries) in &st.secs {
        out.push(name.len() as i32);
        out.extend(name.iter().map(|&b| b as i32));
        out.push(entries.len() as i32);
        for (e, v) in entries {
            out.push(e.len() as i32);
            out.extend(e.iter().map(|&b| b as i32));
            out.push(v.len() as i32);
            out.extend(v.iter().map(|&b| b as i32));
        }
    }
}

const NUMS: &[&str] = &[
    "12", "-7", "+5", "0", "1", "2147483647", "-2147483648", "1Fh", "FFh", "ffH", "$1F", "$ff", "$", "h", "1.5", "50%", "-3 %", "12.5%", "-2.25e3", "1e40",
    ".5", "3.14159265358979", "0.1", "16777217", "16777217.0", "1.00000005960464477539062500", "1.00000005960464477539062501", "0.99999997019767761230468749", "1e-50", "abc", "  42  ", "7 8 9", "1 2  3", "5 x 6", "yes", "No", "TRUE", "false", "t", "y", "n", "f",
    "1,2,3,4", "10, 20 ,30,40", "1,2", "-5,6,7", "a|b+c", "B|X|", "+|a", "red", "Green", "blue", "x y z",
];
const HEXPFX: &[&str] = &["0x1F", "0X1f", "0x", "0x7fffffff", "0xFFFFFFFF", "x1F"];
const POINT2: &[&str] = &["1,2", "-3,4", "10, 20", "0,0", "5,6,7"];
const POINT3: &[&str] = &["1,2,3", "-3,4,5", "0,0,0", "7, 8, 9"];
const POINT3F: &[&str] = &["1.5,2.5,3.5", "-0.25, 1e3,7", "1,2,3", "0,0,0"];
const ENTRIES: &[&str] = &["n0", "n1", "n2", "r0", "l0", "e0", "e1", "e2", "e4", "x0", "x1"];
const SECS: &[&str] = &["s1", "s2"];
const LIST: &[&str] = &["red", "GREEN", "blue", "a", "b", "c", "X"];

struct Prog {
    words: Vec<i32>,
    nops: i32,
    starts: Vec<usize>,
}

fn pstr(w: &mut Vec<i32>, s: &str) {
    w.push(s.len() as i32);
    w.extend(s.bytes().map(|b| b as i32));
}

fn pick<'a>(g: &mut Gen, v: &'a [&'a str]) -> &'a str {
    v[g.below(v.len() as u32) as usize]
}

fn gen_text(g: &mut Gen) -> String {
    let n = match g.below(6) {
        0 => 0,
        1 => 70 + g.below(30),
        2 => 150 + g.below(80),
        _ => g.below(60),
    };
    let words = ["alpha", "beta", "gamma", "delta", "epsilon", "a", "bb", "ccc", "supercalifragilisticexpialidocious", "x"];
    let mut s = String::new();
    while (s.len() as u32) < n {
        if g.below(10) == 0 {
            s.push_str("  ");
        }
        s.push_str(pick(g, &words));
        s.push(if g.below(8) == 0 { '\t' } else { ' ' });
    }
    if g.below(5) == 0 {
        // a 75-character run with no break (the Put_TextBlock `count == 0` exit)
        s = "z".repeat(70 + g.below(20) as usize);
    }
    s
}

fn put_float_bits(w: &mut Vec<i32>, f: f32) {
    w.push(f.to_bits() as i32);
}
fn put_double_bits(w: &mut Vec<i32>, d: f64) {
    let b = d.to_bits();
    w.push((b >> 32) as i32);
    w.push((b & 0xffff_ffff) as u32 as i32);
}

fn gen_f32(g: &mut Gen) -> f32 {
    loop {
        let f = match g.below(4) {
            0 => g.uniform(-100.0, 100.0),
            1 => g.uniform(-1.0e7, 1.0e7),
            2 => f32::from_bits(g.u32()),
            _ => [0.0f32, -0.0, 1.0, 0.5, 1.0e-7, 123456.789][g.below(6) as usize],
        };
        if f.is_finite() {
            return f;
        }
    }
}

/// A float for the point ops: `Put_Point(TPoint3D<float>)` prints three `%f` into a 54-byte buffer, which three numbers of 1e10 or more overflow.
fn gen_pf32(g: &mut Gen) -> f32 {
    match g.below(4) {
        0 => g.uniform(-99999.0, 99999.0),
        1 => g.uniform(-1.0, 1.0),
        2 => [0.0f32, -0.0, 1.0, 0.5, 1.0e-7, 123.456][g.below(6) as usize],
        _ => g.uniform(-1000.0, 1000.0),
    }
}

fn gen_f64(g: &mut Gen) -> f64 {
    loop {
        let d = match g.below(3) {
            0 => g.uniform(-100.0, 100.0) as f64 * 1.000_000_1,
            1 => f64::from_bits(g.u64()),
            _ => [0.0f64, -0.0, 1.0, 0.1, 1.0e-300, 123456.789012][g.below(6) as usize],
        };
        // below 1e50: `Put_Double` prints `%f` into a 64-byte buffer, which a larger magnitude overflows (upstream defect, aborts here)
        if d.is_finite() && d.abs() < 1.0e50 {
            return d;
        }
    }
}

fn gen_prog(g: &mut Gen) -> Prog {
    let nops = 1 + g.below(40) as i32;
    let mut w = Vec::new();
    let mut starts = Vec::new();
    for _ in 0..nops {
        starts.push(w.len());
        let sec = pick(g, SECS);
        let mut ent = pick(g, ENTRIES);
        let op = g.below(28) as i32;
        if matches!(op, 12 | 13 | 22 | 23 | 24) && ent.starts_with('x') {
            ent = "n0"; // integer reads never look at the `x` entries
        }
        if matches!(op, 12 | 13) && g.below(4) == 0 {
            ent = "h0"; // the only entry with `0x` text: read as an integer only (`%f` would take it as a hexadecimal float)
        }
        let head = |w: &mut Vec<i32>, op: i32, ent: &str| {
            w.push(op);
            pstr(w, sec);
            pstr(w, ent);
        };
        match op {
            0 => {
                // a raw value, by entry family
                let v = match ent {
                    "r0" => pick(g, &["1,2,3,4", "10, 20 ,30,40", "1,2", "-5", "x", "7,8,9,10,11"]).to_string(),
                    "l0" => pick(g, &["red", "Green", "BLUE", "a|b+c", "B|X|", "+|a", "zzz", "x y"]).to_string(),
                    _ => pick(g, NUMS).to_string(),
                };
                if g.below(3) == 0 {
                    ent = ["q2", "q3", "f3", "h0"][g.below(4) as usize];
                }
                let v = match ent {
                    "q2" => pick(g, POINT2).to_string(),
                    "q3" => pick(g, POINT3).to_string(),
                    "f3" => pick(g, POINT3F).to_string(),
                    "h0" => pick(g, HEXPFX).to_string(),
                    _ => v,
                };
                let v = if g.below(12) == 0 { String::new() } else { v };
                head(&mut w, 0, ent);
                pstr(&mut w, &v);
            }
            1 => {
                head(&mut w, 1, ent);
                w.push(g.i32());
                w.push(g.below(4) as i32);
            }
            2 => {
                head(&mut w, 2, "r0");
                for _ in 0..4 {
                    w.push((g.i32() >> g.below(28)) as i32);
                }
            }
            3 => {
                head(&mut w, 3, ent);
                w.push(g.i32());
            }
            4 => {
                // a float of any size goes to an `x` entry, which no integer read sees; to the others only below 1e9 (see the header)
                let big = g.below(2) == 0;
                head(&mut w, 4, if big { ["x0", "x1"][g.below(2) as usize] } else { ent });
                let f = if big { gen_f32(g) } else { g.uniform(-1.0e9, 1.0e9) };
                put_float_bits(&mut w, f);
            }
            5 => {
                let big = g.below(2) == 0;
                head(&mut w, 5, if big { ["x0", "x1"][g.below(2) as usize] } else { ent });
                let d = if big { gen_f64(g) } else { g.uniform(-1.0e9, 1.0e9) as f64 * 1.000_000_3 };
                put_double_bits(&mut w, d);
            }
            6 => {
                head(&mut w, 6, ent);
                w.push(g.below(2) as i32);
            }
            7 => {
                head(&mut w, 7, "q2");
                w.extend([g.i32() >> g.below(28), g.i32() >> g.below(28)]);
            }
            8 => {
                head(&mut w, 8, "q3");
                w.extend([g.i32() >> g.below(28), g.i32() >> g.below(28), g.i32() >> g.below(28)]);
            }
            9 => {
                head(&mut w, 9, "f3");
                for _ in 0..3 {
                    put_float_bits(&mut w, gen_pf32(g));
                }
            }
            10 => {
                let t = gen_text(g);
                head(&mut w, 10, "-");
                pstr(&mut w, &t);
            }
            11 => {
                head(&mut w, 11, "-");
                w.push(match g.below(5) {
                    0 => -1,
                    1 => 1,
                    2 => 2,
                    _ => g.below(300) as i32,
                });
            }
            12 | 13 => {
                head(&mut w, op, ent);
                w.push(g.i32() >> g.below(30));
            }
            14 => {
                head(&mut w, 14, ent);
                put_float_bits(&mut w, gen_f32(g));
            }
            15 => {
                // Get_Double is not run: upstream scans `%lf` into a `float` (8 bytes over 4, stack smashing; see the header)
                head(&mut w, 14, ent);
                put_float_bits(&mut w, gen_f32(g));
            }
            16 => {
                head(&mut w, 16, "r0");
                w.extend([g.i32() >> 20, g.i32() >> 20, g.i32() >> 20, g.i32() >> 20]);
            }
            17 => {
                head(&mut w, 17, ent);
                w.push(g.below(2) as i32);
            }
            18 => {
                head(&mut w, 18, "q2");
                w.extend([g.i32() >> 20, g.i32() >> 20]);
            }
            19 => {
                head(&mut w, 19, "q3");
                w.extend([g.i32() >> 20, g.i32() >> 20, g.i32() >> 20]);
            }
            20 => {
                head(&mut w, 20, if g.below(2) == 0 { "q3" } else { "f3" });
                for _ in 0..3 {
                    put_float_bits(&mut w, gen_pf32(g));
                }
            }
            21 => {
                head(&mut w, 21, ["q2", "q3", "f3"][g.below(3) as usize]);
                for _ in 0..2 {
                    put_float_bits(&mut w, gen_pf32(g));
                }
            }
            22 | 23 => {
                head(&mut w, op, if g.below(3) == 0 { "l0" } else { ent });
                w.push(g.below(10) as i32 - 3);
                let nl = g.below(5) as usize;
                w.push(nl as i32);
                for _ in 0..nl {
                    pstr(&mut w, pick(g, LIST));
                }
            }
            24 => {
                head(&mut w, 24, ent);
                w.push(-77777);
            }
            25 => {
                head(&mut w, 25, ent);
                let hasdef = g.below(2) as i32;
                w.push(hasdef);
                pstr(&mut w, if hasdef == 1 { pick(g, NUMS) } else { "" });
            }
            26 => {
                head(&mut w, 26, ent);
                w.push(match g.below(5) {
                    0 => -1,
                    1 => 0,
                    2 => 1,
                    3 => 2,
                    _ => 3 + g.below(70) as i32,
                });
                pstr(&mut w, pick(g, &["", "dflt", "  pad  ", "a long default value that is cut by the size"]));
            }
            _ => {
                head(&mut w, 27, "-");
                pstr(&mut w, "e");
                w.push(g.below(3) as i32);
                w.push(g.below(8) as i32);
            }
        }
    }
    // `head` writes the entry name; the C++ reads (section, entry) for every op, so the count is the loop's
    Prog { words: w, nops, starts }
}

struct Rd<'a> {
    w: &'a [i32],
    i: usize,
}

impl<'a> Rd<'a> {
    fn n(&mut self) -> i32 {
        let v = self.w[self.i];
        self.i += 1;
        v
    }
    fn s(&mut self) -> Vec<u8> {
        let n = self.n();
        (0..n).map(|_| self.n() as u8).collect()
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.n() as u32)
    }
    fn d(&mut self) -> f64 {
        let hi = self.n() as u32 as u64;
        let lo = self.n() as u32 as u64;
        f64::from_bits((hi << 32) | lo)
    }
}

fn pushf(out: &mut Vec<i32>, f: f32) {
    out.push(f.to_bits() as i32);
}

fn run_rust(p: &Prog) -> Vec<i32> {
    let mut r = Rd { w: &p.words, i: 0 };
    let mut st = Store::default();
    let mut out = Vec::new();
    for _ in 0..p.nops {
        let op = r.n();
        let sec = r.s();
        let ent = r.s();
        let (s, e) = (&sec[..], &ent[..]);
        match op {
            0 => {
                let v = r.s();
                out.push(st.put_string(s, e, &v) as i32);
            }
            1 => {
                let n = r.n();
                let f = r.n();
                out.push(put_int(&mut st, s, e, n, f) as i32);
            }
            2 => {
                let v = Rect { x: r.n(), y: r.n(), width: r.n(), height: r.n() };
                out.push(put_rect(&mut st, s, e, v) as i32);
            }
            3 => {
                let n = r.n();
                out.push(put_hex(&mut st, s, e, n) as i32);
            }
            4 => {
                let f = r.f();
                out.push(put_float(&mut st, s, e, f) as i32);
            }
            5 => {
                let d = r.d();
                out.push(put_double(&mut st, s, e, d) as i32);
            }
            6 => {
                let b = r.n() != 0;
                out.push(put_bool(&mut st, s, e, b) as i32);
            }
            7 => {
                let v = TPoint2DInt { x: r.n(), y: r.n() };
                out.push(put_point_2d_int(&mut st, s, e, v) as i32);
            }
            8 => {
                let v = TPoint3DInt { x: r.n(), y: r.n(), z: r.n() };
                out.push(put_point_3d_int(&mut st, s, e, v) as i32);
            }
            9 => {
                let v = TPoint3DFloat { x: r.f(), y: r.f(), z: r.f() };
                out.push(put_point_3d_float(&mut st, s, e, v) as i32);
            }
            10 => {
                let t = r.s();
                out.push(put_text_block(&mut st, s, &t) as i32);
            }
            11 => {
                let len = r.n();
                let (buf, total) = get_text_block(&st, s, len);
                out.push(total);
                if len > 0 {
                    out.push(buf.len() as i32);
                    out.extend(buf.iter().map(|&b| b as i32));
                }
            }
            12 => {
                let d = r.n();
                out.push(get_int(&st, s, e, d));
            }
            13 => {
                let d = r.n();
                out.push(get_hex(&st, s, e, d));
            }
            14 => {
                let d = r.f();
                pushf(&mut out, get_float(&st, s, e, d));
            }
            15 => {
                let d = r.d();
                let v = get_double(&st, s, e, d).to_bits();
                out.push((v >> 32) as i32);
                out.push((v & 0xffff_ffff) as u32 as i32);
            }
            16 => {
                let d = Rect { x: r.n(), y: r.n(), width: r.n(), height: r.n() };
                let v = get_rect(&st, s, e, d);
                out.extend([v.x, v.y, v.width, v.height]);
            }
            17 => {
                let d = r.n() != 0;
                out.push(get_bool(&st, s, e, d) as i32);
            }
            18 => {
                let d = TPoint2DInt { x: r.n(), y: r.n() };
                let v = get_point_2d_int(&st, s, e, d);
                out.extend([v.x, v.y]);
            }
            19 => {
                let d = TPoint3DInt { x: r.n(), y: r.n(), z: r.n() };
                let v = get_point_3d_int(&st, s, e, d);
                out.extend([v.x, v.y, v.z]);
            }
            20 => {
                let d = TPoint3DFloat { x: r.f(), y: r.f(), z: r.f() };
                let v = get_point_3d_float(&st, s, e, d);
                for f in [v.x, v.y, v.z] {
                    pushf(&mut out, f);
                }
            }
            21 => {
                let d = TPoint2DFloat { x: r.f(), y: r.f() };
                let v = get_point_2d_float(&st, s, e, d);
                for f in [v.x, v.y] {
                    pushf(&mut out, f);
                }
            }
            22 | 23 => {
                let d = r.n();
                let nl = r.n();
                let toks: Vec<Vec<u8>> = (0..nl).map(|_| r.s()).collect();
                let list: Vec<&[u8]> = toks.iter().map(|t| &t[..]).collect();
                out.push(if op == 22 { get_list_index(&st, s, e, d, &list) } else { get_int_bitfield(&st, s, e, d, &list) });
            }
            24 => {
                let le = r.n();
                let a = get_alloc_int_array(&st, s, e, le);
                out.push(a.len() as i32 - 1);
                out.extend(a[..a.len() - 1].iter().copied());
            }
            25 => {
                let hasdef = r.n();
                let d = r.s();
                match get_alloc_string(&st, s, e, if hasdef != 0 { Some(&d[..]) } else { None }) {
                    None => out.push(-1),
                    Some(v) => {
                        out.push(v.len() as i32);
                        out.extend(v.iter().map(|&b| b as i32));
                    }
                }
            }
            26 => {
                let size = r.n();
                let d = r.s();
                let v = get_string_sized(&st, s, e, &d, size.max(0) as usize);
                out.push(v.len() as i32);
                if size >= 2 {
                    out.push(v.len() as i32);
                    out.extend(v.iter().map(|&b| b as i32));
                }
            }
            27 => {
                let pre = r.s();
                let a = r.n() as u32;
                let b = r.n() as u32;
                out.push(enumerate_entries(&st, s, &pre, a, b) as i32);
            }
            _ => unreachable!(),
        }
        dump(&mut out, &st);
    }
    out
}

fn run_cpp(p: &Prog) -> Vec<i32> {
    let mut out = vec![0i32; 400_000];
    let n = unsafe { ref_ini_run(p.words.as_ptr(), p.nops, out.as_mut_ptr(), out.len() as i32) } as usize;
    assert!(n <= out.len());
    out.truncate(n);
    out
}

#[test]
fn ini_accessors_match_upstream_over_random_programs() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_17411);
    let mut c = Check::new("INIClass accessor programs");
    for i in 0..n {
        let p = gen_prog(&mut g);
        let cpp = run_cpp(&p);
        let rs = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_rust(&p))).ok();
        let bad = rs.as_ref() != Some(&cpp);
        c.eq(rs, Some(cpp), || format!("program {i}: {:?}", p.words));
        if bad && std::env::var("DRAFT_INI_SHOW").is_ok() && c.mismatches <= 15 {
            for k in 1..=p.nops {
                let q = Prog { words: p.words.clone(), nops: k, starts: Vec::new() };
                let a = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_rust(&q))).ok();
                let b = run_cpp(&q);
                if a.as_ref() != Some(&b) {
                    let at = p.starts[(k - 1) as usize];
                    println!("SHOW program {i}: first differs at op {k}: {:?}", &p.words[at..(at + 24).min(p.words.len())]);
                    {
                        let q0 = Prog { words: p.words.clone(), nops: k - 1, starts: Vec::new() };
                        let before = run_cpp(&q0);
                        let t: String = before.iter().map(|&x| if (32..127).contains(&x) { x as u8 as char } else { '.' }).collect();
                        let from = t.len().saturating_sub(260);
                        println!("   db before the op (tail): {}", &t[from..]);
                    }
                    if let Some(a) = a {
                        let d = a.iter().zip(b.iter()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                        let txt = |v: &Vec<i32>| v.iter().map(|&x| if (32..127).contains(&x) { x as u8 as char } else { '.' }).collect::<String>();
                        println!("   rust text {}\n   c++  text {}", txt(&a), txt(&b));
                        println!("   first differing word {d}: rust {:?} c++ {:?} (lens {} {})", &a[d.saturating_sub(2)..(d + 8).min(a.len())], &b[d.saturating_sub(2)..(d + 8).min(b.len())], a.len(), b.len());
                    } else {
                        println!("   rust panicked");
                    }
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
