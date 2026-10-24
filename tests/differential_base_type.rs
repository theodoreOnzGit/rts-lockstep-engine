// SPDX-License-Identifier: GPL-3.0-only
//! Verification of `common::base_type` (the fast float helpers and REAL_TO_*
//! macros of `Libraries/Include/Lib/BaseType.h`, commit 0a05454d).
//!
//! Upstream is MSVC `__asm` only, so the reference is a GNU-asm transcription
//! of the same instruction sequences (`differential/wrap_base_type.cpp`), not
//! compiled upstream C++. Inputs: every 4099th 32-bit pattern (1,047,809
//! patterns spanning all exponents, NaN and inf included), plus 100k mixed
//! random inputs and the special-value list. Pass criterion: zero mismatches.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::*;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen, N, SPECIALS};

fn inputs() -> Vec<f32> {
    let mut v: Vec<f32> = SPECIALS.to_vec();
    let mut bits: u64 = 0;
    while bits <= u64::from(u32::MAX) {
        v.push(f32::from_bits(bits as u32));
        bits += 4099;
    }
    let mut g = Gen::new(20);
    v.extend((0..N).map(|_| g.mixed()));
    v
}

#[test]
fn fast_float_helpers_match_asm() {
    let xs = inputs();
    let mut c_round = Check::new("fast_float2long_round");
    let mut c_trunc = Check::new("fast_float_trunc");
    let mut c_floor = Check::new("fast_float_floor");
    let mut c_ceil = Check::new("fast_float_ceil");
    for &x in &xs {
        let d = || format!("x = {x:e} ({:#010x})", x.to_bits());
        c_round.eq(
            fast_float2long_round(x),
            unsafe { cpp::ref_fast_float2long_round(x) },
            d,
        );
        c_trunc.f32(
            fast_float_trunc(x),
            unsafe { cpp::ref_fast_float_trunc(x) },
            d,
        );
        c_floor.f32(
            fast_float_floor(x),
            unsafe { cpp::ref_fast_float_floor(x) },
            d,
        );
        c_ceil.f32(
            fast_float_ceil(x),
            unsafe { cpp::ref_fast_float_ceil(x) },
            d,
        );
    }
    c_round.finish();
    c_trunc.finish();
    c_floor.finish();
    c_ceil.finish();
}

#[test]
fn real_to_macros_match_asm() {
    let xs = inputs();
    let mut c = [
        Check::new("REAL_TO_INT"),
        Check::new("REAL_TO_UNSIGNEDINT"),
        Check::new("REAL_TO_SHORT"),
        Check::new("REAL_TO_UNSIGNEDSHORT"),
        Check::new("REAL_TO_BYTE / REAL_TO_CHAR"),
        Check::new("REAL_TO_UNSIGNEDBYTE"),
        Check::new("REAL_TO_INT_CEIL"),
        Check::new("REAL_TO_INT_FLOOR"),
        Check::new("DOUBLE_TO_INT"),
    ];
    let mut g = Gen::new(21);
    for &x in &xs {
        let d = || format!("x = {x:e} ({:#010x})", x.to_bits());
        unsafe {
            c[0].eq(real_to_int(x), cpp::ref_real_to_int(x), d);
            c[1].eq(real_to_unsigned_int(x), cpp::ref_real_to_unsigned_int(x), d);
            c[2].eq(real_to_short(x), cpp::ref_real_to_short(x), d);
            c[3].eq(
                real_to_unsigned_short(x),
                cpp::ref_real_to_unsigned_short(x),
                d,
            );
            c[4].eq(real_to_byte(x), cpp::ref_real_to_byte(x), d);
            c[5].eq(
                real_to_unsigned_byte(x),
                cpp::ref_real_to_unsigned_byte(x),
                d,
            );
            c[6].eq(real_to_int_ceil(x), cpp::ref_real_to_int_ceil(x), d);
            c[7].eq(real_to_int_floor(x), cpp::ref_real_to_int_floor(x), d);
            // A double input: the f32 value itself, and a random double near it.
            let xd = f64::from(x) * (1.0 + (g.u32() as f64 - 2147483648.0) * 1e-12);
            c[8].eq(
                double_to_int(f64::from(x)),
                cpp::ref_double_to_int(f64::from(x)),
                d,
            );
            c[8].eq(double_to_int(xd), cpp::ref_double_to_int(xd), || {
                format!("xd = {xd:e}")
            });
        }
    }
    for k in c {
        k.finish();
    }
}
