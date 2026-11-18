#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo.cpp, lzo1x_c.cpp, lzo1x_d.cpp (and lzo.h, lzo1x.h, lzoconf.h, lzo_conf.h)
//   WWLib/lcw.cpp, rle.cpp, base64.cpp, md5.cpp (and global.h, md5.h), sha.cpp (and sha.h)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The byte codecs of `drafts::wwlib` (LZO, LCW, RLE, Base64, MD5, SHA-1) against the upstream C++ (#215).
//!
//! # Method
//!
//! The C++ side is the upstream .cpp files `#include`d unchanged by `differential/wrap_draft_<codec>.cpp` (each lists its stand-ins:
//! a 32-bit `long` for sha.h and md5's `UINT4`, an empty `CriticalSectionClass`, forwarding headers for the case of file names),
//! compiled with the flags of the shipped build. Every test feeds the same input to both sides from a fixed seed and compares the
//! output bytes, the returned counts and the status, whole. Where the output of an encoder is the input of a decoder, the
//! decoders are also run across: the C++ encoder's bytes through the Rust decoder, the Rust encoder's through the C++ decoder.
//!
//! Inputs by codec: see each test. Excluded inputs (undefined in C, or not representable by the draft): LZO and LCW decoders are
//! run on valid streams only (malformed ones are undefined behaviour upstream and a panic here); LCW's 0xFE run of fewer than
//! 4 bytes (`count -= pad` underflows upstream when `count` is smaller than the distance to the next 4-byte address); RLE's
//! `length <= 0` (its `assert`); MD5 and SHA messages of 512 MiB and more (the 32-bit length words); SHA's engine state is not
//! reachable from outside, so its `Length` near 2^31 is not tested.
//!
//! # Predictions (written before the first run, 2026-11-18) and results
//!
//! Predicted: 0 mismatches everywhere (the drafts are integer code in upstream's order). For the LZO work buffer kept between
//! calls I predicted the output would differ from a fresh dictionary in a fraction of repeated calls (a stale entry inside the
//! new input is accepted).
//!
//! Results (default case counts, 2026-11-18): LZO compress 12,000 inputs (42,159,144 bytes in) 0 mismatches, both round trips
//! 0; LZO decompress 12,000 random valid streams (41.9 MB decoded, 1,510 with bytes after the end marker giving `LZO_E_ERROR`)
//! 0; LCW 20,000 streams 0; RLE compress 40,000 and decompress 40,000 0; Base64 encode 30,000 and decode 30,000 0; MD5 20,000
//! (10,095 wrapping `count[0]`) 0; SHA-1 20,000 sequences (79,977 digests) 0. **Prediction missed (first run):** Base64
//! differed in 17,057 of 30,000 encodes and every decode round trip. Cause: not the draft but the wrapper: glibc's `<endian.h>`
//! defines `BIG_ENDIAN`, so upstream's `#ifdef BIG_ENDIAN` took the big-endian union; MSVC on x86 does not define it. The wrapper
//! now undefines it after the system headers (stand-in, listed in the wrapper). After that 43 encodes differed: a non-NULL empty
//! source with `slen <= 0` writes the trailing NUL upstream; the draft's empty slice is its NULL (not representable; excluded).
//! LZO work buffer (not a gate): **prediction missed in part**: 0 of 3,000 outputs differ from a fresh dictionary's when
//! unrelated inputs follow each other (same buffer address, or a new allocation each time), and 0 of 1,000 when the same input
//! is compressed again; **30 of 1,000 differ when an input is compressed after a similar one (its first 20 bytes changed)**: a stale
//! entry is then a valid earlier match. Every output decompressed to its input. So `LZOCompressor::Compress` is not a pure function
//! of its input upstream (#211 item 7); the draft's fresh dictionary is the deterministic one.
//!
//! Planted bugs (mismatching cases; 10661-10670 LZO of 12,000 inputs or streams): 10661 hash shift 4 not 5: 587; 10662 `m_off <
//! M2_MAX_OFFSET`: 8; 10663 literal run `t < 18`: 764; 10664 the `lit == 3` path skipped: 361; 10665 `m_len < 33`: 589; 10666 M4 bit
//! shift 10: 71; 10667 decode M3 extended base 30: 8,321; 10668 decode M4 `(t & 8) << 10`: 231; 10669 trailing literals `& 1`:
//! 11,120; 10670 first-literal-run distance 0x7ff: 314. LCW (of 20,000) 10671 short copy +2: 14,256; 10672 high nibble shift 4:
//! 5,923; 10673 medium copy mask 0x1f: 15,573; 10674 run byte read from the wrong position: 17,440; 10675 medium copy from
//! dest +2: 14,222; 10676 long copy count and offset swapped: 15,462. RLE 10677 run limit 254: 4,964 of 40,000; 10678 `runcount <
//! length`: 2,016; 10679 line prefix without its 2: 15,021; 10680 zero-run count not charged to `length`: 37,305; 10681 zero run
//! one short: 36,189; 10682 `datalen - 1`: 20,000; 10683 run counted 1 byte: 34,822. Base64 (of 30,000) 10684 shift 17: 17,020;
//! 10685 `pad < 1`: 4,971; 10686 NUL `dlen > 1`: 13,869; 10687 `+` = 63: 2,503 (decode); 10688 `pcount > 1`: 4,402; 10689 `dlen >
//! PACKET_CHARS`: 891; 10690 `=` not ending the data: 12,037. MD5 (of 20,000) 10691 constant index: 20,000; 10692 round 2 word
//! order: 20,000; 10693 shifts: 20,000; 10694 carry `<=`: 9,913; 10695 index mask 0x3E: 15,752; 10696 pad 119: 2,316; 10697
//! encode shift 7: 20,000. SHA-1 (of 20,000) 10698 constant boundary 39: 20,000; 10699 majority function: 20,000; 10700 no rotate
//! (SHA-0): 20,000; 10701 padding `< 8`: 500; 10702 length without the partial block: 16,347; 10703 blocks `(length - 1) / 64`:
//! 6,321; 10704 digest byte order: 20,000. **All 44 of 44 caught.** The least caught (10662, 8 of 12,000) needs an offset
//! of exactly 0x800 with a pending three-literal run.
//!
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose). The table is at the end of this header, per codec.

mod support;

use rts_lockstep_engine::drafts::wwlib::lzo::{lzo1x_1_compress, lzo_buffer_size, lzo_compressor_decompress, LZO_E_ERROR, LZO_E_OK};
use support::{Check, Gen};

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_CODEC_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// A Rust call whose panic (an out-of-range index where C reads or writes out of range) is a mismatch, not an abort.
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

/// Structured and random byte strings of length `n`.
fn gen_data(g: &mut Gen, n: usize) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::with_capacity(n);
    let kind = g.below(9);
    match kind {
        0 => (0..n).for_each(|_| v.push(g.u32() as u8)),
        1 => {
            let k = 2 + g.below(3);
            let alpha: Vec<u8> = (0..k).map(|_| g.u32() as u8).collect();
            (0..n).for_each(|_| v.push(alpha[g.below(k) as usize]));
        }
        2 => {
            while v.len() < n {
                let b = if g.below(3) == 0 { 0 } else { g.u32() as u8 };
                let r = 1 + mix(g, 4, 600, 12) as usize;
                for _ in 0..r.min(n - v.len()) {
                    v.push(b);
                }
            }
        }
        3 => {
            let p = 1 + g.below(40) as usize;
            let pat: Vec<u8> = (0..p).map(|_| g.u32() as u8).collect();
            (0..n).for_each(|i| v.push(pat[i % p]));
        }
        4 => {
            let words: [&[u8]; 8] = [b"the ", b"unit ", b"armor ", b"general ", b"GLA ", b"Zero Hour ", b"\n", b"supply "];
            while v.len() < n {
                let w = words[g.below(8) as usize];
                for &b in w.iter().take(n - v.len()) {
                    v.push(b);
                }
            }
        }
        5 | 7 => {
            // LZ-style: literals interleaved with copies from earlier (near or far) positions
            while v.len() < n {
                if v.len() < 4 || g.below(3) == 0 {
                    for _ in 0..(1 + g.below(20) as usize).min(n - v.len()) {
                        v.push(g.u32() as u8);
                    }
                } else {
                    let far = kind == 7 && g.below(2) == 0;
                    let maxd = v.len().min(if far { 50_000 } else { 3000 });
                    let d = 1 + g.below(maxd as u32) as usize;
                    let l = (3 + mix(g, 5, 400, 12) as usize).min(n - v.len());
                    for _ in 0..l {
                        let b = v[v.len() - d];
                        v.push(b);
                    }
                }
            }
        }
        6 => v.resize(n, 0),
        _ => (0..n).for_each(|_| v.push(if g.below(20) == 0 { g.u32() as u8 } else { 0 })),
    }
    v.truncate(n);
    v
}

/// `g.below(big)` one time in `one_in`, else `g.below(small)`.
fn mix(g: &mut Gen, one_in: u32, big: u32, small: u32) -> u32 {
    if g.below(one_in) == 0 {
        g.below(big)
    } else {
        g.below(small)
    }
}

fn gen_len(g: &mut Gen) -> usize {
    match g.below(100) {
        0..=14 => g.below(32) as usize,
        15..=69 => g.below(300) as usize,
        70..=91 => g.below(5000) as usize,
        92..=97 => 16_000 + g.below(20_000) as usize,
        _ => 40_000 + g.below(40_000) as usize,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// LZO
// ---------------------------------------------------------------------------------------------------------------------------

extern "C" {
    fn ref_lzo_compress_fresh(inp: *const u8, n: u32, out: *mut u8, out_len: *mut u32) -> i32;
    fn ref_lzo_compressor_compress(inp: *const u8, n: u32, out: *mut u8, out_len: *mut u32) -> i32;
    fn ref_lzo_work_buffer_clear();
    fn ref_lzo_decompress(inp: *const u8, n: u32, out: *mut u8, out_len: *mut u32) -> i32;
}

/// The C++ calls share `LZOCompressor::WorkBuffer`: one at a time.
static LZO_CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_lzo_compress_fresh(inp: &[u8]) -> (i32, Vec<u8>) {
    let _one = LZO_CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0xEEu8; lzo_buffer_size(inp.len()) + 64];
    let mut n = 0u32;
    let r = unsafe { ref_lzo_compress_fresh(inp.as_ptr(), inp.len() as u32, out.as_mut_ptr(), &mut n) };
    out.truncate(n as usize);
    (r, out)
}

fn cpp_lzo_compressor(inp: &[u8]) -> (i32, Vec<u8>) {
    let _one = LZO_CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0xEEu8; lzo_buffer_size(inp.len()) + 64];
    let mut n = 0u32;
    let r = unsafe { ref_lzo_compressor_compress(inp.as_ptr(), inp.len() as u32, out.as_mut_ptr(), &mut n) };
    out.truncate(n as usize);
    (r, out)
}

fn cpp_lzo_decompress(inp: &[u8], cap: usize) -> (i32, Vec<u8>) {
    let mut out = vec![0xEEu8; cap + 64];
    let mut n = 0u32;
    let r = unsafe { ref_lzo_decompress(inp.as_ptr(), inp.len() as u32, out.as_mut_ptr(), &mut n) };
    out.truncate(n as usize);
    (r, out)
}

fn rust_lzo_compress(inp: &[u8]) -> Option<(i32, Vec<u8>)> {
    rust(|| {
        let mut out = vec![0u8; lzo_buffer_size(inp.len())];
        let (r, n) = lzo1x_1_compress(inp, &mut out);
        out.truncate(n);
        (r, out)
    })
}

fn rust_lzo_decompress(inp: &[u8], cap: usize) -> Option<(i32, Vec<u8>)> {
    rust(|| {
        let mut out = vec![0u8; cap];
        let (r, n) = lzo_compressor_decompress(inp, &mut out);
        out.truncate(n);
        (r, out)
    })
}

#[test]
fn lzo_compress_matches_upstream_with_a_fresh_work_buffer() {
    let n = cases(12_000);
    let mut g = Gen::new(0x215_1001);
    let mut c = Check::new("lzo1x_1_compress (fresh work buffer)");
    let mut c_rt1 = Check::new("lzo round trip: C++ compress, Rust decompress");
    let mut c_rt2 = Check::new("lzo round trip: Rust compress, C++ decompress");
    let (mut bytes_in, mut bytes_out) = (0u64, 0u64);
    for i in 0..n {
        let len = gen_len(&mut g);
        let data = gen_data(&mut g, len);
        let cp = cpp_lzo_compress_fresh(&data);
        let rs = rust_lzo_compress(&data);
        bytes_in += len as u64;
        bytes_out += cp.1.len() as u64;
        c.eq(rs.clone(), Some(cp.clone()), || format!("case {i}, {len} bytes"));
        // C++ compress, Rust decompress
        let back = rust_lzo_decompress(&cp.1, len + 8);
        c_rt1.eq(back, Some((LZO_E_OK, data.clone())), || format!("case {i}, {len} bytes"));
        // Rust compress, C++ decompress
        // (not in a planted run: a wrong encoder's bytes are not a valid stream, and the C++ decoder would read out of range)
        if let (Some((_, rbytes)), false) = (&rs, planted()) {
            let back = cpp_lzo_decompress(rbytes, len + 8);
            c_rt2.eq(back, (LZO_E_OK, data.clone()), || format!("case {i}, {len} bytes"));
        }
    }
    println!("{n} inputs, {bytes_in} bytes in, {bytes_out} bytes out");
    c_rt1.finish_or_report();
    c_rt2.finish_or_report();
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

/// A random valid LZO1X stream built from the decoder's own grammar (lzo1x_d.cpp), so the branches the encoder never emits
/// (M1 matches, the three-byte copy after a first literal run) are reached. Returns the stream and the decoded length.
fn gen_lzo_stream(g: &mut Gen) -> (Vec<u8>, usize) {
    #[derive(Clone, Copy, PartialEq)]
    enum St {
        Outer,
        AfterLit,
        Inner,
    }
    let mut s: Vec<u8> = Vec::new();
    let mut op = 0usize;
    let big = g.below(10) == 0;
    let budget = if big { 90_000 } else { 3_000 };
    let zmax: u32 = if big { 60 } else { 3 };
    let lit = |g: &mut Gen, s: &mut Vec<u8>, op: &mut usize, n: usize| {
        for _ in 0..n {
            s.push(g.u32() as u8);
        }
        *op += n;
    };
    let mut state;
    if g.below(2) == 0 {
        let l = 1 + g.below(238) as usize;
        s.push((17 + l) as u8);
        lit(g, &mut s, &mut op, l);
        state = St::AfterLit;
    } else {
        state = St::Outer;
    }
    let mut steps = 0;
    loop {
        steps += 1;
        let stop = op > budget || steps > 400 || g.below(40) == 0;
        if stop && op > 0 || (stop && state != St::Outer && state != St::Inner && false) {
            break;
        }
        // which instruction: a literal run (Outer only), a short match (AfterLit, Inner), or a counted match
        let can_m1 = state == St::Inner && op >= 1;
        let can_far = state == St::AfterLit && op >= 0x801 + 3 + 1020;
        let choice = g.below(10);
        if state == St::Outer && (op == 0 || choice < 3) {
            // a literal run instruction: t in 1..=15 gives 3 + t literals; t = 0 is extended
            if g.below(4) == 0 {
                let z = g.below(3) as usize;
                s.extend(std::iter::repeat(0u8).take(1 + z)); // t = 0 byte and z zero bytes
                let last = 1 + g.below(255) as u8;
                s.push(last);
                let total = 3 + 15 + 255 * z + last as usize;
                lit(g, &mut s, &mut op, total);
            } else {
                let t = 1 + g.below(15) as usize;
                s.push(t as u8);
                lit(g, &mut s, &mut op, 3 + t);
            }
            state = St::AfterLit;
            continue;
        }
        if can_far && choice < 3 {
            // t < 16 after a literal run: m_pos = op - 1 - 0x800 - (t >> 2) - (b << 2); 3 bytes; trailing t & 3
            let t = g.below(16) as usize;
            let maxb = ((op - 1 - 0x800 - (t >> 2)) / 4).min(255);
            let b = g.below(maxb as u32 + 1) as usize;
            s.push(t as u8);
            s.push(b as u8);
            op += 3;
            let tr = t & 3;
            lit(g, &mut s, &mut op, tr);
            state = if tr > 0 { St::Inner } else { St::Outer };
            continue;
        }
        if can_m1 && choice < 4 {
            // a M1 match: m_pos = op - 1 - (t >> 2) - (b << 2); 2 bytes; trailing t & 3
            let t = g.below(16) as usize;
            let room = op - 1;
            if room < (t >> 2) {
                continue;
            }
            let maxb = ((room - (t >> 2)) / 4).min(255);
            let b = g.below(maxb as u32 + 1) as usize;
            s.push(t as u8);
            s.push(b as u8);
            op += 2;
            let tr = t & 3;
            lit(g, &mut s, &mut op, tr);
            state = if tr > 0 { St::Inner } else { St::Outer };
            continue;
        }
        if op == 0 {
            continue;
        }
        // a counted match, with 0 to 3 trailing literals
        let tr = g.below(4) as usize;
        let kind = g.below(3);
        let pick = |g: &mut Gen, max: usize| -> usize {
            // a distance in 1..=max, biased to the ends
            match g.below(6) {
                0 => 1,
                1 => max,
                _ => 1 + g.below(max as u32) as usize,
            }
        };
        match kind {
            0 => {
                // M2: copies 2 + k, k in 1..=6; distance 1 + ((t >> 2) & 7) + (b << 3)
                let d = pick(g, op.min(8 + 255 * 8));
                let k = 1 + g.below(6) as usize;
                let t = ((k + 1) << 5) | (((d - 1) & 7) << 2) | tr;
                s.push(t as u8);
                s.push(((d - 1) >> 3) as u8);
                op += 2 + k;
            }
            1 => {
                // M3: 2 + t, t in 1..=31 by the code, or extended; distance 1 + (b1 >> 2) + (b2 << 6)
                let d = pick(g, op.min(16384));
                let (code, ext): (usize, Option<(usize, u8)>) = if g.below(4) == 0 { (0, Some((g.below(zmax) as usize, 1 + g.below(255) as u8))) } else { (1 + g.below(31) as usize, None) };
                s.push((32 | code) as u8);
                let t = match ext {
                    Some((z, last)) => {
                        s.extend(std::iter::repeat(0u8).take(z));
                        s.push(last);
                        31 + 255 * z + last as usize
                    }
                    None => code,
                };
                s.push(((((d - 1) & 63) << 2) | tr) as u8);
                s.push(((d - 1) >> 6) as u8);
                op += 2 + t;
            }
            _ => {
                // M4: distance 0x4000 + ((t & 8) << 11) + (b1 >> 2) + (b2 << 6), at most 0xbfff; not 0x4000 + 0 (the end marker)
                if op < 0x4001 {
                    continue;
                }
                let d = pick(g, op.min(0xbfff) - 0x4000) + 0x4000;
                let e = d - 0x4000; // (t & 8) << 11 + (b1 >> 2) + (b2 << 6), 1..=0x7fff
                let hi = e >= 0x4000;
                let rest = e - if hi { 0x4000 } else { 0 };
                if e == 0 || rest > 0x3fff {
                    continue;
                }
                let (code, ext): (usize, Option<(usize, u8)>) = if g.below(4) == 0 { (0, Some((g.below(zmax) as usize, 1 + g.below(255) as u8))) } else { (1 + g.below(7) as usize, None) };
                s.push((16 | if hi { 8 } else { 0 } | code) as u8);
                let t = match ext {
                    Some((z, last)) => {
                        s.extend(std::iter::repeat(0u8).take(z));
                        s.push(last);
                        7 + 255 * z + last as usize
                    }
                    None => code,
                };
                s.push((((rest & 63) << 2) | tr) as u8);
                s.push((rest >> 6) as u8);
                op += 2 + t;
            }
        }
        lit(g, &mut s, &mut op, tr);
        state = if tr > 0 { St::Inner } else { St::Outer };
    }
    // the end-of-stream marker
    s.extend([17, 0, 0]);
    (s, op)
}

#[test]
fn lzo_decompress_matches_upstream_on_valid_streams() {
    let n = cases(12_000);
    let mut g = Gen::new(0x215_1002);
    let mut c = Check::new("lzo1x_decompress (valid streams)");
    let (mut total_out, mut errs) = (0u64, 0u64);
    for i in 0..n {
        let (mut s, want) = gen_lzo_stream(&mut g);
        // a share carry bytes after the end marker: LZO_E_ERROR with the same output length
        let trailing = g.below(8) == 0;
        if trailing {
            for _ in 0..1 + g.below(4) {
                s.push(g.u32() as u8);
            }
        }
        let cp = cpp_lzo_decompress(&s, want);
        let rs = rust_lzo_decompress(&s, want + 8);
        total_out += cp.1.len() as u64;
        if cp.0 == LZO_E_ERROR {
            errs += 1;
        }
        assert!(trailing || cp.0 == LZO_E_OK, "the generator produced a stream the C++ rejects at case {i}");
        c.eq(rs, Some(cp), || format!("case {i}, stream of {} bytes, {want} decoded", s.len()));
    }
    println!("{n} streams, {total_out} bytes decoded, {errs} with bytes after the end marker (LZO_E_ERROR)");
    finish(c);
}

#[test]
fn lzo_work_buffer_kept_between_calls_vs_a_fresh_one() {
    // FINDING (#211 item 7): upstream's LZOCompressor keeps its work buffer between calls, the draft uses a fresh one. Report
    // how often the C++ output of a repeated call differs from a fresh dictionary's, under three protocols. Not a gate: the
    // output is address-dependent upstream (a stale entry is accepted when it lies inside the current input); a stale-accepted
    // match is still a valid match, so every output must decompress to its input (that IS asserted).
    let n = cases(3000) as usize;
    let mut g = Gen::new(0x215_1003);
    let mut buf = vec![0u8; 100_000];
    let (mut same_diff, mut same_total, mut fresh_diff, mut fresh_total, mut first_diff) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut invalid = 0u64;
    let mut size_delta: i64 = 0;
    for i in 0..n {
        let len = 14 + (gen_len(&mut g) % 6000);
        let data = gen_data(&mut g, len);
        // (C) first call after a clear equals the fresh one
        if i % 50 == 0 {
            unsafe { ref_lzo_work_buffer_clear() };
            let a = cpp_lzo_compressor(&data);
            let f = cpp_lzo_compress_fresh(&data);
            if a != f {
                first_diff += 1;
            }
        }
        // (A) the same buffer address every call
        buf[..len].copy_from_slice(&data);
        let a = cpp_lzo_compressor(&buf[..len]);
        let f = cpp_lzo_compress_fresh(&data);
        same_total += 1;
        if a != f {
            same_diff += 1;
            size_delta += a.1.len() as i64 - f.1.len() as i64;
        }
        if cpp_lzo_decompress(&a.1, len + 8) != (LZO_E_OK, data.clone()) {
            invalid += 1;
        }
        // (B) a new allocation every call (the allocator decides the address)
        let owned = data.clone();
        let a = cpp_lzo_compressor(&owned);
        fresh_total += 1;
        if a != f {
            fresh_diff += 1;
        }
        if cpp_lzo_decompress(&a.1, len + 8) != (LZO_E_OK, data.clone()) {
            invalid += 1;
        }
    }
    // (D) the same input again, and a structured input again with its head changed: the cases where an entry left by the earlier
    // call points at bytes that match the new input
    let (mut again_diff, mut again_total, mut edit_diff, mut edit_total) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..n / 3 {
        let len = 200 + (gen_len(&mut g) % 6000);
        let data = gen_data(&mut g, len);
        buf[..len].copy_from_slice(&data);
        let first = cpp_lzo_compressor(&buf[..len]);
        let f = cpp_lzo_compress_fresh(&data);
        let second = cpp_lzo_compressor(&buf[..len]);
        again_total += 1;
        if second != f {
            again_diff += 1;
        }
        if cpp_lzo_decompress(&second.1, len + 8) != (LZO_E_OK, data.clone()) || first.0 != LZO_E_OK {
            invalid += 1;
        }
        // the first 20 bytes replaced, the rest the same
        let mut data2 = data.clone();
        for b in data2.iter_mut().take(20) {
            *b = g.u32() as u8;
        }
        buf[..len].copy_from_slice(&data2);
        let third = cpp_lzo_compressor(&buf[..len]);
        let f2 = cpp_lzo_compress_fresh(&data2);
        edit_total += 1;
        if third != f2 {
            edit_diff += 1;
        }
        if cpp_lzo_decompress(&third.1, len + 8) != (LZO_E_OK, data2.clone()) {
            invalid += 1;
        }
        let _ = i;
    }
    println!("LZO work buffer, same input compressed again in the same buffer: {again_diff} of {again_total} differ from a fresh dictionary's; the input with its first 20 bytes changed, compressed after the original: {edit_diff} of {edit_total}");
    println!("LZO work buffer kept between calls: same input buffer address every call: {same_diff} of {same_total} outputs differ from a fresh dictionary's (total size change {size_delta} bytes); a new Vec per call: {fresh_diff} of {fresh_total}; first call after a clear: {first_diff} differ; outputs that do not decompress to their input: {invalid}");
    assert_eq!(first_diff, 0, "after a clear the static buffer must equal a fresh one");
    assert_eq!(invalid, 0, "every output, stale dictionary or not, must decompress to its input");
}

// ---------------------------------------------------------------------------------------------------------------------------
// LCW
// ---------------------------------------------------------------------------------------------------------------------------

use rts_lockstep_engine::drafts::wwlib::base64::{base64_decode, base64_encode};
use rts_lockstep_engine::drafts::wwlib::lcw::lcw_uncomp;
use rts_lockstep_engine::drafts::wwlib::md5::{md5_final, md5_update, MD5Ctx};
use rts_lockstep_engine::drafts::wwlib::rle::{rle_compress, rle_decompress, rle_line_compress, rle_line_decompress};
use rts_lockstep_engine::drafts::wwlib::sha::SHAEngine;

extern "C" {
    fn ref_lcw_uncomp(src: *const u8, dest: *mut u8, cap: u32, align: u32) -> i32;
    fn ref_rle_compress(src: *const u8, src_len: u32, length: i32, after: u8, dest: *mut u8, dest_cap: u32, measure: i32) -> i32;
    fn ref_rle_line_compress(src: *const u8, src_len: u32, length: i32, after: u8, dest: *mut u8, dest_cap: u32, measure: i32) -> i32;
    fn ref_rle_decompress(src: *const u8, src_len: u32, length: i32, dest: *mut u8, dest_cap: u32) -> i32;
    fn ref_rle_line_decompress(src: *const u8, src_len: u32, dest: *mut u8, dest_cap: u32) -> i32;
    fn ref_base64_encode(src: *const u8, slen: i32, dest: *mut u8, dlen: i32, null_src: i32, null_dest: i32) -> i32;
    fn ref_base64_decode(src: *const u8, slen: i32, dest: *mut u8, dlen: i32, null_src: i32, null_dest: i32) -> i32;
    fn ref_md5_case(ctx_in: *const u8, init: i32, data: *const u8, data_len: u32, cuts: *const u32, ncuts: u32, ctx_mid: *mut u8, digest: *mut u8, ctx_final: *mut u8);
    fn ref_sha_case(data: *const u8, data_len: u32, ops: *const i32, nops: u32, out: *mut u8) -> i32;
}

/// A random valid LCW stream (lcw.cpp's own grammar) and its decoded length. The 0xFE long run has at least 4 bytes (see the
/// header). The stream ends with the 0x80 end code.
fn gen_lcw_stream(g: &mut Gen) -> (Vec<u8>, usize) {
    let mut s: Vec<u8> = Vec::new();
    let mut op = 0usize;
    let budget = if g.below(10) == 0 { 60_000 } else { 2_000 };
    // the first code must produce output: a medium copy from the source
    let mut first = true;
    let mut steps = 0;
    loop {
        steps += 1;
        if !first && (op > budget || steps > 300 || g.below(30) == 0) {
            break;
        }
        let kind = if first { 0 } else { g.below(6) };
        first = false;
        match kind {
            0 => {
                // 10xxxxxx: copy x bytes from the source, x in 1..=63
                let x = 1 + g.below(63) as usize;
                s.push(0x80 | x as u8);
                for _ in 0..x {
                    s.push(g.u32() as u8);
                }
                op += x;
            }
            1 => {
                // 0xxxyyyy yyyyyyyy: copy back y bytes (1..=4095), run x + 3 (3..=10)
                let maxy = op.min(4095);
                let y = match g.below(5) {
                    0 => 1,
                    1 => maxy,
                    _ => 1 + g.below(maxy as u32) as usize,
                };
                let x = g.below(8) as usize;
                s.push(((x << 4) | (y >> 8)) as u8);
                s.push((y & 0xff) as u8);
                op += x + 3;
            }
            2 => {
                // 11xxxxxx w1: copy x + 3 bytes from the absolute offset w1 (x in 0..=61: 0xFE and 0xFF are other codes)
                let x = g.below(62) as usize;
                let w = match g.below(4) {
                    0 => 0,
                    1 => op - 1,
                    _ => g.below(op as u32) as usize,
                };
                s.push(0xC0 | x as u8);
                s.push((w & 0xff) as u8);
                s.push((w >> 8) as u8);
                op += x + 3;
            }
            3 => {
                // 11111111 w1 w2: copy w1 bytes from the absolute offset w2
                let w1 = 1 + mix(g, 4, 3000, 80) as usize;
                let w2 = g.below(op as u32) as usize;
                s.push(0xFF);
                s.push((w1 & 0xff) as u8);
                s.push((w1 >> 8) as u8);
                s.push((w2 & 0xff) as u8);
                s.push((w2 >> 8) as u8);
                op += w1;
            }
            _ => {
                // 11111110 w1 b1: a run of w1 bytes of b1, w1 >= 4
                let w1 = 4 + mix(g, 4, 2000, 30) as usize;
                s.push(0xFE);
                s.push((w1 & 0xff) as u8);
                s.push((w1 >> 8) as u8);
                s.push(g.u32() as u8);
                op += w1;
            }
        }
    }
    s.push(0x80);
    (s, op)
}

fn rust_lcw(s: &[u8], want: usize) -> Option<(usize, Vec<u8>)> {
    rust(|| {
        let mut dest = vec![0u8; want];
        let n = lcw_uncomp(s, &mut dest);
        dest.truncate(n);
        (n, dest)
    })
}

fn cpp_lcw(s: &[u8], want: usize, align: u32) -> (usize, Vec<u8>) {
    let mut dest = vec![0u8; want];
    let n = unsafe { ref_lcw_uncomp(s.as_ptr(), dest.as_mut_ptr(), want as u32, align) };
    dest.truncate((n as usize).min(want));
    (n as usize, dest)
}

#[test]
fn lcw_uncomp_matches_upstream() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_2001);
    let mut c = Check::new("LCW_Uncomp");
    let mut by_code = [0u64; 6];
    for i in 0..n {
        let (s, want) = gen_lcw_stream(&mut g);
        let align = g.below(4);
        for &b in &s {
            let _ = b;
        }
        let cp = cpp_lcw(&s, want, align);
        let rs = rust_lcw(&s, want);
        by_code[(s[0] >> 6) as usize & 3] += 1;
        c.eq(rs, Some(cp), || format!("case {i}, stream of {} bytes, {want} decoded, align {align}", s.len()));
    }
    println!("{n} streams");
    finish(c);
}

// ---------------------------------------------------------------------------------------------------------------------------
// RLE
// ---------------------------------------------------------------------------------------------------------------------------

/// Bytes with runs of zeros (RLE's input is sprite data: mostly zero runs and other bytes).
fn gen_rle_data(g: &mut Gen, n: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(n);
    while v.len() < n {
        if g.below(3) == 0 {
            let r = 1 + mix(g, 4, 700, 10) as usize;
            for _ in 0..r.min(n - v.len()) {
                v.push(0);
            }
        } else {
            let r = 1 + g.below(8) as usize;
            for _ in 0..r.min(n - v.len()) {
                v.push(1 + g.below(255) as u8);
            }
        }
    }
    v
}

#[test]
fn rle_compress_matches_upstream() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_3001);
    let mut c = Check::new("RLEEngine::Compress, Line_Compress");
    let mut ends_in_zeros = 0u64;
    let mut sentinel_dependent = 0u64;
    for i in 0..n {
        let len = 1 + match g.below(10) {
            0 => g.below(4),
            1..=7 => g.below(120),
            _ => g.below(3000),
        } as usize;
        let data = gen_rle_data(&mut g, len);
        // `length` is the whole input, or (a few cases) a prefix: the byte after the prefix is then real data
        let length = if g.below(8) == 0 { 1 + g.below(len as u32) as usize } else { len };
        let measure = g.below(4) == 0;
        if data[length - 1] == 0 {
            ends_in_zeros += 1;
        }
        let cap = 2 * len + 16;
        let mut dest = vec![0u8; cap];
        let nc = unsafe { ref_rle_compress(data.as_ptr(), len as u32, length as i32, 0xAA, dest.as_mut_ptr(), cap as u32, measure as i32) };
        dest.truncate(if measure { 0 } else { nc as usize });
        let rs = rust(|| {
            let mut out = Vec::new();
            let r = rle_compress(&data, if measure { None } else { Some(&mut out) }, length as i32);
            (r, out)
        });
        c.eq(rs, Some((nc, dest.clone())), || format!("Compress case {i}, {len} bytes, length {length}, measure {measure}"));
        // Line_Compress
        let mut dest2 = vec![0u8; cap + 2];
        let nl = unsafe { ref_rle_line_compress(data.as_ptr(), len as u32, length as i32, 0xAA, dest2.as_mut_ptr(), (cap + 2) as u32, measure as i32) };
        dest2.truncate(if measure { 0 } else { nl as usize });
        let rs = rust(|| {
            let mut out = Vec::new();
            let r = rle_line_compress(&data, if measure { None } else { Some(&mut out) }, length as i32);
            (r, out)
        });
        c.eq(rs, Some((nl, dest2.clone())), || format!("Line_Compress case {i}, {len} bytes, length {length}, measure {measure}"));
        // the C++ result when the byte after the input is zero: reported, not gated (undefined upstream, see the header)
        if length == len && data[len - 1] == 0 {
            let mut d3 = vec![0u8; cap];
            let n3 = unsafe { ref_rle_compress(data.as_ptr(), len as u32, length as i32, 0, d3.as_mut_ptr(), cap as u32, 0) };
            d3.truncate(n3 as usize);
            let mut d4 = vec![0u8; cap];
            let n4 = unsafe { ref_rle_compress(data.as_ptr(), len as u32, length as i32, 0xAA, d4.as_mut_ptr(), cap as u32, 0) };
            d4.truncate(n4 as usize);
            if d3 != d4 {
                sentinel_dependent += 1;
            }
        }
    }
    println!("{n} inputs; {ends_in_zeros} end in zeros; for inputs that end in zeros, the C++ output depends on the byte after the input in {sentinel_dependent} cases");
    finish(c);
}

/// A random valid RLE stream: bytes other than 0 stand for themselves, `0, n` is a run of n zeros (n may be 0).
fn gen_rle_stream(g: &mut Gen) -> (Vec<u8>, usize, Vec<usize>) {
    let mut s = Vec::new();
    let mut out = 0usize;
    let mut bounds = vec![0usize];
    let items = 1 + mix(g, 5, 400, 30);
    for _ in 0..items {
        if g.below(3) == 0 {
            let r = match g.below(4) {
                0 => 0,
                1 => 255,
                _ => g.below(256) as u8,
            };
            s.push(0);
            s.push(r);
            out += r as usize;
        } else {
            s.push(1 + g.below(255) as u8);
            out += 1;
        }
        bounds.push(s.len());
    }
    (s, out, bounds)
}

#[test]
fn rle_decompress_matches_upstream() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_3002);
    let mut c = Check::new("RLEEngine::Decompress, Line_Decompress");
    for i in 0..n {
        let (s, out, bounds) = gen_rle_stream(&mut g);
        // `length`: the whole stream, or a prefix that ends at an item boundary
        let b = if g.below(5) == 0 { bounds[g.below(bounds.len() as u32) as usize] } else { s.len() };
        let b = b.max(1).min(s.len());
        let b = *bounds.iter().filter(|&&x| x >= b).next().unwrap_or(&s.len());
        let cap = out + 8;
        let mut dest = vec![0u8; cap];
        let nc = unsafe { ref_rle_decompress(s.as_ptr(), s.len() as u32, b as i32, dest.as_mut_ptr(), cap as u32) };
        dest.truncate((nc as usize).min(cap));
        let rs = rust(|| {
            let mut d = vec![0u8; cap];
            let r = rle_decompress(&s, &mut d, b as i32);
            d.truncate((r as usize).min(cap));
            (r, d)
        });
        c.eq(rs, Some((nc, dest.clone())), || format!("Decompress case {i}, stream {} bytes, length {b}", s.len()));
        // Line_Decompress: a 16-bit length (this stream's + 2) then the stream
        let mut line = Vec::new();
        line.extend_from_slice(&((s.len() + 2) as u16).to_le_bytes());
        line.extend_from_slice(&s);
        let mut dest = vec![0u8; cap];
        let nc = unsafe { ref_rle_line_decompress(line.as_ptr(), line.len() as u32, dest.as_mut_ptr(), cap as u32) };
        dest.truncate((nc as usize).min(cap));
        let rs = rust(|| {
            let mut d = vec![0u8; cap];
            let r = rle_line_decompress(&line, &mut d);
            d.truncate((r as usize).min(cap));
            (r, d)
        });
        c.eq(rs, Some((nc, dest)), || format!("Line_Decompress case {i}, stream {} bytes", s.len()));
    }
    println!("{n} streams");
    finish(c);
}

#[test]
fn rle_round_trip_through_the_cpp_and_the_rust() {
    let n = cases(5_000);
    let mut g = Gen::new(0x215_3003);
    let mut c1 = Check::new("RLE: C++ compress, Rust decompress");
    let mut c2 = Check::new("RLE: Rust compress, C++ decompress");
    for i in 0..n {
        let len = 1 + g.below(2000) as usize;
        let data = gen_rle_data(&mut g, len);
        let cap = 2 * len + 16;
        let mut comp = vec![0u8; cap];
        let nc = unsafe { ref_rle_compress(data.as_ptr(), len as u32, len as i32, 0xAA, comp.as_mut_ptr(), cap as u32, 0) } as usize;
        comp.truncate(nc);
        let back = rust(|| {
            let mut d = vec![0u8; len + 8];
            let r = rle_decompress(&comp, &mut d, comp.len() as i32);
            d.truncate(r as usize);
            d
        });
        c1.eq(back, Some(data.clone()), || format!("case {i}, {len} bytes"));
        let rc = rust(|| {
            let mut out = Vec::new();
            rle_compress(&data, Some(&mut out), len as i32);
            out
        });
        if let Some(rc) = rc {
            let mut d = vec![0u8; len + 8];
            let r = unsafe { ref_rle_decompress(rc.as_ptr(), rc.len() as u32, rc.len() as i32, d.as_mut_ptr(), (len + 8) as u32) };
            d.truncate(r as usize);
            c2.eq(d, data.clone(), || format!("case {i}, {len} bytes"));
        }
    }
    c1.finish_or_report();
    c2.finish_or_report();
}

// ---------------------------------------------------------------------------------------------------------------------------
// Base64
// ---------------------------------------------------------------------------------------------------------------------------

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[test]
fn base64_encode_matches_upstream() {
    let n = cases(30_000);
    let mut g = Gen::new(0x215_4001);
    let mut c = Check::new("Base64_Encode");
    for i in 0..n {
        let len = match g.below(10) {
            0 => g.below(3) as usize,
            1..=7 => g.below(40) as usize,
            _ => g.below(600) as usize,
        };
        let src: Vec<u8> = (0..len).map(|_| g.u32() as u8).collect();
        let slen = match g.below(12) {
            0 => 0,
            1 => -(1 + g.below(5) as i32),
            2 if len > 0 => g.below(len as u32) as i32,
            _ => len as i32,
        };
        let need = ((slen.max(0) as usize + 2) / 3 * 4) as i32;
        let dlen = match g.below(10) {
            0 => 0,
            1 => -(g.below(5) as i32),
            2 | 3 => g.below(need as u32 + 6) as i32,
            4 => need + 1 + g.below(5) as i32,
            _ => need + 1,
        };
        let null_src = len == 0; // an empty slice is the draft's NULL (a non-NULL pointer with slen <= 0 is not representable)
        let null_dest = g.below(20) == 0;
        let bufn = dlen.max(0) as usize;
        let mut cd = vec![0xCCu8; bufn.max(1)];
        let nc = unsafe { ref_base64_encode(src.as_ptr(), slen, cd.as_mut_ptr(), dlen, null_src as i32, null_dest as i32) };
        let rs = rust(|| {
            let mut d = vec![0xCCu8; bufn];
            let s: &[u8] = if null_src { &[] } else { &src };
            let dd: &mut [u8] = if null_dest { &mut [] } else { &mut d };
            let r = base64_encode(s, slen, dd, dlen);
            (r, d)
        });
        let cd = if null_dest { vec![0xCCu8; bufn] } else { cd[..bufn].to_vec() };
        c.eq(rs, Some((nc, cd)), || format!("case {i}: {len} bytes, slen {slen}, dlen {dlen}"));
    }
    println!("{n} inputs");
    finish(c);
}

#[test]
fn base64_decode_matches_upstream() {
    let n = cases(30_000);
    let mut g = Gen::new(0x215_4002);
    let mut c = Check::new("Base64_Decode");
    let mut c_rt = Check::new("Base64: encode, decode round trip (Rust encode, C++ decode)");
    for i in 0..n {
        let len = match g.below(10) {
            0 => g.below(3) as usize,
            1..=7 => g.below(60) as usize,
            _ => g.below(600) as usize,
        };
        let src: Vec<u8> = (0..len)
            .map(|_| match g.below(20) {
                0 => b'=',
                1 => b' ',
                2 => b'\n',
                3 => g.u32() as u8,
                _ => B64[g.below(64) as usize],
            })
            .collect();
        let slen = match g.below(12) {
            0 => 0,
            1 => -(1 + g.below(5) as i32),
            2 if len > 0 => g.below(len as u32) as i32,
            _ => len as i32,
        };
        let need = (len / 4 * 3 + 3) as i32;
        let dlen = match g.below(10) {
            0 => 0,
            1 => -(g.below(5) as i32),
            2 | 3 => g.below(need as u32 + 3) as i32,
            _ => need + g.below(4) as i32,
        };
        let null_src = len == 0 && g.below(2) == 0;
        let bufn = dlen.max(0) as usize;
        let mut cd = vec![0xCCu8; bufn.max(1)];
        let nc = unsafe { ref_base64_decode(src.as_ptr(), slen, cd.as_mut_ptr(), dlen, null_src as i32, 0) };
        let rs = rust(|| {
            let mut d = vec![0xCCu8; bufn];
            let s: &[u8] = if null_src { &[] } else { &src };
            let r = base64_decode(s, slen, &mut d, dlen);
            (r, d)
        });
        c.eq(rs, Some((nc, cd[..bufn].to_vec())), || format!("case {i}: {len} chars, slen {slen}, dlen {dlen}"));
        // round trip
        let raw: Vec<u8> = (0..g.below(50)).map(|_| g.u32() as u8).collect();
        if !raw.is_empty() {
            let en = rust(|| {
                let mut e = vec![0u8; raw.len() / 3 * 4 + 5];
                let el = e.len() as i32;
                let r = base64_encode(&raw, raw.len() as i32, &mut e, el);
                e.truncate(r as usize);
                e
            });
            if let Some(e) = en {
                let mut d = vec![0u8; raw.len() + 4];
                let r = unsafe { ref_base64_decode(e.as_ptr(), e.len() as i32, d.as_mut_ptr(), raw.len() as i32 + 4, 0, 0) };
                d.truncate(r as usize);
                c_rt.eq(d, raw.clone(), || format!("case {i}"));
            }
        }
    }
    println!("{n} inputs");
    c_rt.finish_or_report();
    finish(c);
}

// ---------------------------------------------------------------------------------------------------------------------------
// MD5
// ---------------------------------------------------------------------------------------------------------------------------

fn ctx_bytes(c: &MD5Ctx) -> Vec<u8> {
    let mut v = Vec::with_capacity(88);
    for w in c.state.iter().chain(c.count.iter()) {
        v.extend_from_slice(&w.to_ne_bytes());
    }
    v.extend_from_slice(&c.buffer);
    v
}

fn ctx_from(b: &[u8]) -> MD5Ctx {
    let w = |k: usize| u32::from_ne_bytes([b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]]);
    let mut c = MD5Ctx::new();
    c.state = [w(0), w(1), w(2), w(3)];
    c.count = [w(4), w(5)];
    c.buffer.copy_from_slice(&b[24..88]);
    c
}

#[test]
fn md5_matches_upstream() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_5001);
    let mut c = Check::new("MD5Init, MD5Update, MD5Final");
    let mut carries = 0u64;
    for i in 0..n {
        let init = g.below(2) == 0;
        // the starting context: a real MD5Init's, or arbitrary state, count and buffer (any count is a context an
        // earlier update could have reached; a count near 2^32 makes the carry into count[1] reachable)
        let mut ctx_in = vec![0u8; 88];
        for b in ctx_in.iter_mut() {
            *b = g.u32() as u8;
        }
        if !init && g.below(3) == 0 {
            let near = 0xFFFF_FFFFu32 - g.below(2000);
            ctx_in[16..20].copy_from_slice(&near.to_ne_bytes());
        }
        let len = match g.below(10) {
            0 => 0,
            1..=6 => g.below(150) as usize,
            _ => g.below(3000) as usize,
        };
        let data: Vec<u8> = (0..len).map(|_| g.u32() as u8).collect();
        let mut cuts: Vec<u32> = Vec::new();
        let mut left = len;
        while left > 0 || cuts.is_empty() {
            let k = if g.below(4) == 0 { 0 } else { (1 + mix(&mut g, 3, 200, 70) as usize).min(left) };
            cuts.push(k as u32);
            left -= k;
            if cuts.len() > 400 {
                if left > 0 {
                    cuts.push(left as u32);
                }
                break;
            }
        }
        let mut mid = vec![0u8; 88];
        let mut digest = vec![0u8; 16];
        let mut fin = vec![0xEEu8; 88];
        unsafe { ref_md5_case(ctx_in.as_ptr(), init as i32, data.as_ptr(), len as u32, cuts.as_ptr(), cuts.len() as u32, mid.as_mut_ptr(), digest.as_mut_ptr(), fin.as_mut_ptr()) };
        let rs = rust(|| {
            let mut ctx = ctx_from(&ctx_in);
            if init {
                rts_lockstep_engine::drafts::wwlib::md5::md5_init(&mut ctx);
            }
            let mut at = 0usize;
            let before = ctx.count[0];
            for &k in &cuts {
                md5_update(&mut ctx, &data[at..at + k as usize]);
                at += k as usize;
            }
            let m = ctx_bytes(&ctx);
            let d = md5_final(&mut ctx);
            (m, d.to_vec(), ctx_bytes(&ctx), ctx.count[0] < before)
        });
        if let Some((_, _, _, true)) = rs {
            carries += 1;
        }
        c.eq(rs.map(|(m, d, f, _)| (m, d, f)), Some((mid, digest, fin)), || format!("case {i}: init {init}, {len} bytes in {} pieces", cuts.len()));
    }
    println!("{n} inputs; {carries} wrap count[0]");
    finish(c);
}

#[test]
fn md5_known_vectors_agree_with_rfc_1321() {
    // not a differential check: the RFC 1321 test suite, so that "matches upstream" is anchored to the standard
    let cases: [(&[u8], &str); 4] = [
        (b"", "d41d8cd98f00b204e9800998ecf8427e"),
        (b"a", "0cc175b9c0f1b6a831c399e269772661"),
        (b"abc", "900150983cd24fb0d6963f7d28e17f72"),
        (b"message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
    ];
    for (m, hex) in cases {
        let mut ctx = MD5Ctx::new();
        md5_update(&mut ctx, m);
        let d = md5_final(&mut ctx);
        let s: String = d.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(s, hex);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// SHA-1
// ---------------------------------------------------------------------------------------------------------------------------

#[test]
fn sha_matches_upstream() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_6001);
    let mut c = Check::new("SHAEngine::Hash, Result, Init");
    let mut results = 0u64;
    for i in 0..n {
        let mut ops: Vec<i32> = Vec::new();
        let mut data: Vec<u8> = Vec::new();
        let nops = 1 + g.below(14);
        for _ in 0..nops {
            match g.below(10) {
                0..=5 => {
                    let k = match g.below(8) {
                        0 => 0,
                        1 => 64,
                        2 => 128,
                        3 => 55 + g.below(12) as usize, // around the padding boundary
                        4 => 1 + g.below(3) as usize,
                        5 => g.below(400) as usize,
                        _ => g.below(100) as usize,
                    };
                    ops.extend([0, k as i32]);
                    for _ in 0..k {
                        data.push(g.u32() as u8);
                    }
                }
                6..=7 => ops.extend([1, 0]),
                8 => ops.extend([3, 0]),
                _ => ops.extend([2, 0]),
            }
        }
        ops.extend([1, 0]);
        let mut out = vec![0u8; 20 * 3 * (ops.len() / 2) + 8];
        let nc = unsafe { ref_sha_case(data.as_ptr(), data.len() as u32, ops.as_ptr(), (ops.len() / 2) as u32, out.as_mut_ptr()) } as usize;
        out.truncate(nc);
        let rs = rust(|| {
            let mut e = SHAEngine::new();
            let mut at = 0usize;
            let mut o: Vec<u8> = Vec::new();
            for w in ops.chunks(2) {
                match w[0] {
                    0 => {
                        e.hash(&data[at..at + w[1] as usize]);
                        at += w[1] as usize;
                    }
                    1 => o.extend_from_slice(&e.result()),
                    3 => {
                        o.extend_from_slice(&e.result());
                        o.extend_from_slice(&e.result());
                    }
                    _ => e.init(),
                }
            }
            o
        });
        results += (nc / 20) as u64;
        c.eq(rs, Some(out), || format!("case {i}: {} bytes, {} operations", data.len(), ops.len() / 2));
    }
    println!("{n} sequences, {results} digests");
    finish(c);
}

#[test]
fn sha_known_vectors_agree_with_fips_180() {
    // anchor, not a differential check: FIPS 180-1's "abc"
    let mut e = SHAEngine::new();
    e.hash(b"abc");
    let d = e.result();
    let s: String = d.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(s, "a9993e364706816aba3e25717850c26c9cd0d89d");
}
