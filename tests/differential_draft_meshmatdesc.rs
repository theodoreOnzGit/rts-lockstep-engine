// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d4) of `UvBufferClass::{update_crc, is_equal_to}` in the draft
//! `src/drafts/sweep_ww3d/meshmatdesc.rs` against upstream `WW3D2/meshmatdesc.cpp` `UVBufferClass::{Update_CRC, Is_Equal_To}`, cut
//! VERBATIM (build.rs `drafts_ww3d4`, differential/wrap_draft_dw4_mmdesc.cpp). Stand-ins (C++): the buffer's fields without the
//! `ShareBufferClass` base; `CRC_Memory` is the real `WWLib/realcrc.cpp`.
//!
//! # Method
//!
//! 20,000 pairs of buffers (fixed seed, `DRAFT_CASES` overrides): 0 to 40 `Vector2`s of random finite floats, zeros, negative zeros,
//! infinities and NaNs, the second buffer sometimes a copy of the first or a copy with one component changed. Compared: the CRC
//! before the update (`0xFFFFFFFF`), both CRCs after, `Is_Equal_To(a, b)` and `Is_Equal_To(a, a)`.
//!
//! # Not covered
//!
//! The rest of the file (`MatBufferClass`/`TexBufferClass` elements, `Get_Material_Array`, `Get_Shader_Array`,
//! `Make_UV_Array_Unique`, `Make_Color_Array_Unique`) is `Arc` bookkeeping standing in for upstream's intrusive reference counts
//! (`ShareBufferClass::Num_Refs`); the C++ needs the whole render stack to run, so there is no meaningful reference to compare to.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::sweep_ww3d::meshmatdesc::UvBufferClass;
use std::sync::Mutex;
use support::{Check, Gen};
use support_sweep::{cases, finish};

extern "C" {
    fn ref_dw4_mmdesc_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn comp(g: &mut Gen) -> f32 {
    match g.below(8) {
        0 => 0.0,
        1 => -0.0,
        2 => f32::INFINITY,
        3 => f32::NAN,
        4 => 1.0,
        _ => g.uniform(-10.0, 10.0),
    }
}

#[test]
fn uv_buffer_crc_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_d406);
    let mut c = Check::new("UVBufferClass Update_CRC / Is_Equal_To");
    for case in 0..cases() {
        let na = g.below(41) as usize;
        let a: Vec<[f32; 2]> = (0..na).map(|_| [comp(&mut g), comp(&mut g)]).collect();
        let b: Vec<[f32; 2]> = match g.below(3) {
            0 => a.clone(),
            1 => {
                let mut b = a.clone();
                if !b.is_empty() {
                    let i = g.below(b.len() as u32) as usize;
                    b[i][g.below(2) as usize] = comp(&mut g);
                }
                b
            }
            _ => (0..g.below(41)).map(|_| [comp(&mut g), comp(&mut g)]).collect(),
        };
        let mut inp = vec![a.len() as u32, b.len() as u32];
        for v in a.iter().chain(b.iter()) {
            inp.extend([v[0].to_bits(), v[1].to_bits()]);
        }
        let mut ua = UvBufferClass::new(a.len());
        ua.array = a;
        let mut ub = UvBufferClass::new(b.len());
        ub.array = b;
        let before = ua.crc;
        ua.update_crc();
        ub.update_crc();
        let rust = vec![before, ua.crc, ub.crc, ua.is_equal_to(&ub) as u32, ua.is_equal_to(&ua) as u32];
        let mut out = vec![0u32; 8];
        let n = unsafe { ref_dw4_mmdesc_run(inp.as_ptr(), inp.len() as i32, out.as_mut_ptr(), 8) };
        assert_eq!(n, 5);
        out.truncate(5);
        c.eq(rust, out, || format!("case {case}: {inp:?}"));
    }
    finish(vec![c]);
}
