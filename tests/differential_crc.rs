// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::crc` against upstream
//! `GameEngine/Include/Common/crc.h` + `Source/Common/crc.cpp` (commit 0a05454d).
//!
//! Two references: (1) the C++ `CRC` class compiled unchanged with `_DEBUG`
//! (the out-of-line C++ path); (2) a GNU-asm transcription of the release
//! `_asm` block (g++ cannot compile MSVC inline asm), see
//! `differential/wrap_crc.cpp`. Pass criterion: zero mismatches on 100k
//! random buffers (length 0..=600, including empty) and on 100k chunked
//! accumulations.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::crc::Crc;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen, N};

fn buffer(g: &mut Gen) -> Vec<u8> {
    let len = match g.below(10) {
        0 => 0,
        1 => 1,
        2 => 24,
        _ => g.below(601) as usize,
    };
    let fill = g.below(4);
    (0..len)
        .map(|_| match fill {
            0 => 0xff,
            1 => 0x00,
            _ => g.u32() as u8,
        })
        .collect()
}

#[test]
fn single_buffers_match_debug_cpp_and_release_asm() {
    let mut cd = Check::new("CRC::computeCRC vs C++ (_DEBUG path)");
    let mut ca = Check::new("CRC::computeCRC vs release asm transcription");
    let mut g = Gen::new(10);
    for i in 0..N {
        let b = buffer(&mut g);
        let mut c = Crc::new();
        c.compute_crc(&b);
        let d = unsafe { cpp::ref_crc_debug_buffer(b.as_ptr(), b.len() as i32) };
        let a = unsafe { cpp::ref_crc_asm_update(0, b.as_ptr(), b.len() as i32) };
        cd.eq(c.get(), d, || format!("buffer {i}, len {}", b.len()));
        ca.eq(c.get(), a, || format!("buffer {i}, len {}", b.len()));
    }
    cd.finish();
    ca.finish();
}

#[test]
fn chunked_accumulation_matches() {
    let mut cd = Check::new("CRC accumulated over chunks (C++)");
    let mut ca = Check::new("CRC accumulated over chunks (asm)");
    let mut g = Gen::new(11);
    for i in 0..N {
        let b = buffer(&mut g);
        let mut lens: Vec<i32> = Vec::new();
        let mut rest = b.len();
        while rest > 0 {
            let l = (g.below(40) as usize).min(rest);
            lens.push(l as i32);
            rest -= l;
        }
        // Upstream ignores len < 1; include some zero and negative lengths.
        lens.push(0);
        lens.push(-5);
        let mut c = Crc::new();
        let mut asm = 0u32;
        let mut off = 0usize;
        for &l in &lens {
            let n = l.max(0) as usize;
            c.compute_crc(&b[off..off + n]);
            asm = unsafe { cpp::ref_crc_asm_update(asm, b[off..].as_ptr(), l) };
            off += n;
        }
        let d = unsafe { cpp::ref_crc_debug_chunks(b.as_ptr(), lens.as_ptr(), lens.len() as i32) };
        cd.eq(c.get(), d, || format!("case {i}"));
        ca.eq(c.get(), asm, || format!("case {i}"));
    }
    cd.finish();
    ca.finish();
}
