//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 12,000 inputs and 12,000 streams (test: tests/differential_draft_codec.rs);
//! not yet wired into the game.
//! Test notes: `lzo_compressor_compress` is `lzo1x_1_compress` here by design (a fresh work buffer); upstream's kept work buffer was measured (30 of 1000 outputs differ when a similar input follows another).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo1x.h, lzo_conf.h, lzoconf.h (the constants used)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo1x_c.cpp (`lzo1x_1_compress`)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/lzo1x_d.cpp (`lzo1x_decompress`)
//!
//! The LZO real-time data compression library (Copyright (C) 1996 Markus Franz Xaver Johannes Oberhumer, GNU Library
//! General Public License v2 or later; the notice in lzo1x_c.cpp / lzo1x_d.cpp must be retained). `LZO1X` is the variant
//! compiled (`M2_MAX_OFFSET = 0x800`). Upstream reads and writes through raw pointers with no bounds checks: here
//! every input and output access is a slice index, so malformed compressed data or a short output buffer PANICS where
//! C reads or writes out of range (undefined behaviour). `lzo_uint` is `usize`.
//!
//! Departures: `LZOCompressor`'s static, mutex-guarded work buffer is a fresh dictionary per call (its entries are
//! offsets into the input, `None` for a null pointer). Upstream keeps the buffer between calls, so an entry left by an
//! earlier call is a stale pointer that `LZO_CHECK_MPOS_NON_DET` happens to accept or reject depending on where the new
//! input lies in memory; a fresh dictionary is the deterministic reading of that code.

use crate::logic::modules::plant;

/// `LZO_E_OK`, `LZO_E_ERROR`.
pub const LZO_E_OK: i32 = 0;
pub const LZO_E_ERROR: i32 = -1;

/// `LZO1X_MEM_COMPRESS` = 16384 pointers.
pub const LZO1X_MEM_COMPRESS: usize = 16384 * std::mem::size_of::<usize>();

/// `LZO_BUFFER_SIZE(s)`: the output buffer for `s` uncompressed bytes ("LZO needs an additional 16 bytes per
/// uncompressed K").
pub fn lzo_buffer_size(s: usize) -> usize {
    s + (((s / 0x400) + 1) * 16)
}

const M2_MAX_OFFSET: usize = 0x0800;
const M3_MAX_OFFSET: usize = 0x4000;
const M4_MAX_OFFSET: usize = 0xbfff;
const M3_MARKER: u8 = 32;
const M4_MARKER: u8 = 16;

/// `lzo1x_decompress(in, in_len, out, &out_len, NULL)` (lzo1x_d.cpp:78). Returns the status and the number of bytes
/// written. `LZO_E_OK` only when the end-of-stream marker was found exactly at the end of the input.
pub fn lzo1x_decompress(input: &[u8], out: &mut [u8]) -> (i32, usize) {
    #[derive(Clone, Copy, PartialEq)]
    enum S {
        Outer,
        FirstLiteralRun,
        Match,
        MatchDone,
        Inner,
    }

    let in_len = input.len();
    let mut op: isize = 0;
    let mut ip: usize = 0;
    let mut t: usize;
    let mut m_pos: isize;

    let rd = |ip: usize| -> usize { input[ip] as usize };

    let mut state;
    if input[0] > 17 {
        t = input[ip] as usize - 17;
        ip += 1;
        state = S::FirstLiteralRun;
    } else {
        t = 0;
        state = S::Outer;
    }

    loop {
        match state {
            S::Outer => {
                t = rd(ip);
                ip += 1;
                if t >= 16 {
                    state = S::Match;
                    continue;
                }
                // a literal run
                if t == 0 {
                    t = 15;
                    while input[ip] == 0 {
                        t += 255;
                        ip += 1;
                    }
                    t += rd(ip);
                    ip += 1;
                }
                // copy literals
                for _ in 0..3 {
                    out[op as usize] = input[ip];
                    op += 1;
                    ip += 1;
                }
                state = S::FirstLiteralRun;
            }
            S::FirstLiteralRun => {
                // do *op++ = *ip++; while (--t > 0);
                loop {
                    out[op as usize] = input[ip];
                    op += 1;
                    ip += 1;
                    t -= 1;
                    if t == 0 {
                        break;
                    }
                }

                t = rd(ip);
                ip += 1;
                if t >= 16 {
                    state = S::Match;
                    continue;
                }
                // LZO1X: m_pos = op - 1 - 0x800
                m_pos = op - 1 - (if plant(10670) { 0x7ff } else { 0x800 });
                m_pos -= (t >> 2) as isize;
                m_pos -= (rd(ip) << 2) as isize;
                ip += 1;
                for _ in 0..3 {
                    out[op as usize] = out[idx(m_pos)];
                    op += 1;
                    m_pos += 1;
                }
                state = S::MatchDone;
            }
            S::Inner => {
                if t < 16 {
                    // a M1 match
                    m_pos = op - 1;
                    m_pos -= (t >> 2) as isize;
                    m_pos -= (rd(ip) << 2) as isize;
                    ip += 1;
                    out[op as usize] = out[idx(m_pos)];
                    op += 1;
                    m_pos += 1;
                    out[op as usize] = out[idx(m_pos)];
                    op += 1;
                    state = S::MatchDone;
                } else {
                    state = S::Match;
                }
            }
            S::Match => {
                if t >= 64 {
                    // a M2 match
                    m_pos = op - 1;
                    m_pos -= ((t >> 2) & 7) as isize;
                    m_pos -= (rd(ip) << 3) as isize;
                    ip += 1;
                    t = (t >> 5) - 1;
                } else if t >= 32 {
                    // a M3 match
                    t &= 31;
                    if t == 0 {
                        t = if plant(10667) { 30 } else { 31 };
                        while input[ip] == 0 {
                            t += 255;
                            ip += 1;
                        }
                        t += rd(ip);
                        ip += 1;
                    }
                    m_pos = op - 1;
                    m_pos -= (rd(ip) >> 2) as isize;
                    ip += 1;
                    m_pos -= (rd(ip) << 6) as isize;
                    ip += 1;
                } else {
                    // a M4 match
                    m_pos = op;
                    m_pos -= ((t & 8) << (if plant(10668) { 10 } else { 11 })) as isize;
                    t &= 7;
                    if t == 0 {
                        t = 7;
                        while input[ip] == 0 {
                            t += 255;
                            ip += 1;
                        }
                        t += rd(ip);
                        ip += 1;
                    }
                    m_pos -= (rd(ip) >> 2) as isize;
                    ip += 1;
                    m_pos -= (rd(ip) << 6) as isize;
                    ip += 1;
                    if m_pos == op {
                        // eof_found: assert(t == 1)
                        debug_assert!(t == 1);
                        return (if ip == in_len { LZO_E_OK } else { LZO_E_ERROR }, op as usize);
                    }
                    m_pos -= 0x4000;
                }
                // *op++ = *m_pos++; *op++ = *m_pos++; do *op++ = *m_pos++; while (--t > 0);
                out[op as usize] = out[idx(m_pos)];
                op += 1;
                m_pos += 1;
                out[op as usize] = out[idx(m_pos)];
                op += 1;
                m_pos += 1;
                loop {
                    out[op as usize] = out[idx(m_pos)];
                    op += 1;
                    m_pos += 1;
                    t -= 1;
                    if t == 0 {
                        break;
                    }
                }
                state = S::MatchDone;
            }
            S::MatchDone => {
                t = (input[ip - 2] & (if plant(10669) { 1 } else { 3 })) as usize;
                if t == 0 {
                    state = S::Outer;
                    continue;
                }
                // copy literals
                loop {
                    out[op as usize] = input[ip];
                    op += 1;
                    ip += 1;
                    t -= 1;
                    if t == 0 {
                        break;
                    }
                }
                t = rd(ip);
                ip += 1;
                state = S::Inner;
            }
        }
    }
}

/// A back-reference position as an index: a negative `m_pos` (before the start of the output) is an out-of-range read in C.
fn idx(pos: isize) -> usize {
    usize::try_from(pos).expect("LZO match reaches before the start of the output")
}

/// The dictionary hash of lzo1x_c.cpp: `DVAL_FIRST`, `DVAL_NEXT`, `DINDEX`.
fn dval_first(p: &[u8], i: usize) -> usize {
    // _DV2(p, 5, 5) = (((p[2] << 5) ^ p[1]) << 5) ^ p[0]
    ((((p[i + 2] as usize) << 5) ^ p[i + 1] as usize) << 5) ^ p[i] as usize
}
fn dval_next(dv: usize, p: &[u8], i: usize) -> usize {
    // dv ^= p[-1]; dv = (dv >> 5) ^ (p[2] << 10)
    let dv = dv ^ p[i - 1] as usize;
    (dv >> 5) ^ ((p[i + 2] as usize) << 10)
}
fn dindex(dv: usize) -> usize {
    // ((40799u * dv) >> 5) & 0x3fff  (32-bit unsigned arithmetic)
    ((((40799u32).wrapping_mul(dv as u32)) >> (if plant(10661) { 4 } else { 5 })) & 0x3fff) as usize
}

/// `do_compress(in, in_len, out, &out_len, wrkmem)` (lzo1x_c.cpp:112): LZ77 with a 16384 entry hash dictionary of the
/// last position seen for each 3-byte hash. Returns the number of bytes written.
fn do_compress(input: &[u8], out: &mut [u8]) -> usize {
    let in_len = input.len();
    let in_end = in_len;
    // ip_end = in + in_len - 9 - 4
    let ip_end = in_len - 9 - 4;
    let mut dict: Vec<Option<usize>> = vec![None; 16384];

    let mut op = 0usize;
    let mut ip = 0usize;
    let mut ii = ip;

    let mut dv = dval_first(input, ip);
    dict[dindex(dv)] = Some(ip);
    ip += 1;
    for _ in 0..3 {
        dv = dval_next(dv, input, ip);
        dict[dindex(dv)] = Some(ip);
        ip += 1;
    }

    loop {
        let dix = dindex(dv);
        let m_pos_opt = dict[dix];
        dict[dix] = Some(ip);

        // LZO_CHECK_MPOS_NON_DET: m_pos < in (null), m_off = ip - m_pos <= 0, or m_off > M4_MAX_OFFSET
        let mut matched = false;
        let mut m_pos = 0usize;
        let mut m_off = 0usize;
        let mut lit = 0usize;
        let mut goto_code_match = false;
        if let Some(mp) = m_pos_opt {
            if ip > mp && ip - mp <= M4_MAX_OFFSET {
                m_off = ip - mp;
                m_pos = mp;
                if input[m_pos] == input[ip] && input[m_pos + 1] == input[ip + 1] {
                    if input[m_pos + 2] == input[ip + 2] {
                        lit = ip - ii;
                        m_pos += 3;
                        if if plant(10662) { m_off < M2_MAX_OFFSET } else { m_off <= M2_MAX_OFFSET } {
                            matched = true;
                        } else if lit == 3 && !plant(10664) {
                            // better compression, but slower
                            out[op - 2] |= 3;
                            for _ in 0..3 {
                                out[op] = input[ii];
                                op += 1;
                                ii += 1;
                            }
                            matched = true;
                            goto_code_match = true;
                        } else if input[m_pos] == input[ip + 3] {
                            matched = true;
                        }
                    }
                    // else: "still need a better way for finding M1 matches"
                }
            }
        }

        if !matched {
            // a literal
            ip += 1;
            if ip >= ip_end {
                break;
            }
            dv = dval_next(dv, input, ip);
            continue;
        }

        if !goto_code_match {
            // a match: store current literal run
            if lit > 0 {
                let mut t = lit;
                if t <= 3 {
                    out[op - 2] |= t as u8;
                } else if if plant(10663) { t < 18 } else { t <= 18 } {
                    out[op] = (t - 3) as u8;
                    op += 1;
                } else {
                    let mut tt = t - 18;
                    out[op] = 0;
                    op += 1;
                    while tt > 255 {
                        tt -= 255;
                        out[op] = 0;
                        op += 1;
                    }
                    out[op] = tt as u8;
                    op += 1;
                }
                loop {
                    out[op] = input[ii];
                    op += 1;
                    ii += 1;
                    t -= 1;
                    if t == 0 {
                        break;
                    }
                }
            }
        }

        // code_match: code the match
        debug_assert!(ii == ip);
        ip += 3;
        // if (*m_pos++ != *ip++ || ... six comparisons ...)
        let mut all_equal = true;
        for _ in 0..6 {
            let a = input[m_pos];
            m_pos += 1;
            let b = input[ip];
            ip += 1;
            if a != b {
                all_equal = false;
                break;
            }
        }

        let mut m_len: usize;
        let mut skip_to_offset = false;
        if !all_equal {
            ip -= 1;
            m_len = ip - ii;
            if m_off <= M2_MAX_OFFSET {
                m_off -= 1;
                out[op] = (((m_len - 1) << 5) | ((m_off & 7) << 2)) as u8;
                op += 1;
                out[op] = (m_off >> 3) as u8;
                op += 1;
                skip_to_offset = false;
                // (the M2 code is two bytes and does not go to m3_m4_offset)
            } else if m_off <= M3_MAX_OFFSET {
                m_off -= 1;
                out[op] = M3_MARKER | (m_len - 2) as u8;
                op += 1;
                skip_to_offset = true;
            } else {
                m_off -= 0x4000;
                out[op] = M4_MARKER | ((m_off & 0x4000) >> 11) as u8 | (m_len - 2) as u8;
                op += 1;
                skip_to_offset = true;
            }
            if skip_to_offset {
                // m3_m4_offset:
                out[op] = ((m_off & 63) << 2) as u8;
                op += 1;
                out[op] = (m_off >> 6) as u8;
                op += 1;
            }
        } else {
            let end = in_end;
            while ip < end && input[m_pos] == input[ip] {
                m_pos += 1;
                ip += 1;
            }
            m_len = ip - ii;

            // 'goto m3_m4_len' flag: the length bytes still to be written
            let mut write_len_bytes = false;
            if m_off <= M3_MAX_OFFSET {
                m_off -= 1;
                if if plant(10665) { m_len < 33 } else { m_len <= 33 } {
                    out[op] = M3_MARKER | (m_len - 2) as u8;
                    op += 1;
                } else {
                    m_len -= 33;
                    out[op] = M3_MARKER;
                    op += 1;
                    write_len_bytes = true;
                }
            } else {
                m_off -= 0x4000;
                if m_len <= 9 {
                    out[op] = M4_MARKER | ((m_off & 0x4000) >> (if plant(10666) { 10 } else { 11 })) as u8 | (m_len - 2) as u8;
                    op += 1;
                } else {
                    m_len -= 9;
                    out[op] = M4_MARKER | ((m_off & 0x4000) >> 11) as u8;
                    op += 1;
                    write_len_bytes = true;
                }
            }
            if write_len_bytes {
                // m3_m4_len:
                while m_len > 255 {
                    m_len -= 255;
                    out[op] = 0;
                    op += 1;
                }
                out[op] = m_len as u8;
                op += 1;
            }
            // m3_m4_offset:
            out[op] = ((m_off & 63) << 2) as u8;
            op += 1;
            out[op] = (m_off >> 6) as u8;
            op += 1;
        }

        ii = ip;
        if ip >= ip_end {
            break;
        }
        dv = dval_first(input, ip);
    }

    // store final literal run
    if in_end - ii > 0 {
        let mut t = in_end - ii;
        if op == 0 && t <= 238 {
            out[op] = (17 + t) as u8;
            op += 1;
        } else if t <= 3 {
            out[op - 2] |= t as u8;
        } else if t <= 18 {
            out[op] = (t - 3) as u8;
            op += 1;
        } else {
            let mut tt = t - 18;
            out[op] = 0;
            op += 1;
            while tt > 255 {
                tt -= 255;
                out[op] = 0;
                op += 1;
            }
            out[op] = tt as u8;
            op += 1;
        }
        loop {
            out[op] = input[ii];
            op += 1;
            ii += 1;
            t -= 1;
            if t == 0 {
                break;
            }
        }
    }

    op
}

/// `lzo1x_1_compress(in, in_len, out, &out_len, wrkmem)` (lzo1x_c.cpp:326): a block of 13 bytes or fewer is stored as
/// one literal run; anything longer goes through `do_compress`; then the end-of-stream marker `M4_MARKER | 1, 0, 0`.
/// `out` should be at least [`lzo_buffer_size`]`(in.len())` bytes. Returns the status and the compressed length.
pub fn lzo1x_1_compress(input: &[u8], out: &mut [u8]) -> (i32, usize) {
    let in_len = input.len();
    let mut out_len = 0usize;
    let r = LZO_E_OK;

    if in_len == 0 {
        out_len = 0;
    } else if in_len <= 9 + 4 {
        let mut op = 0usize;
        out[op] = (17 + in_len) as u8;
        op += 1;
        for &b in input.iter() {
            out[op] = b;
            op += 1;
        }
        out_len = op;
    } else {
        out_len = do_compress(input, out);
    }

    if r == LZO_E_OK {
        let mut op = out_len;
        out[op] = M4_MARKER | 1;
        op += 1;
        out[op] = 0;
        op += 1;
        out[op] = 0;
        out_len += 3;
    }
    (r, out_len)
}

/// `LZOCompressor::Compress(in, in_len, out, &out_len)`: the mutex-guarded wrapper (the work buffer is per call here).
pub fn lzo_compressor_compress(input: &[u8], out: &mut [u8]) -> (i32, usize) {
    lzo1x_1_compress(input, out)
}

/// `LZOCompressor::Decompress(in, in_len, out, &out_len)`.
pub fn lzo_compressor_decompress(input: &[u8], out: &mut [u8]) -> (i32, usize) {
    lzo1x_decompress(input, out)
}
