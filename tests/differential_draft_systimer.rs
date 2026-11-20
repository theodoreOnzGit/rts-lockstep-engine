#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/systimer.cpp (SysTimeClass::Is_Getting_Late)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_wwlib::systimer::is_getting_late` against the upstream C++ (#215, group wwlib3).
//!
//! # Method
//!
//! `differential/wrap_draft_systimer.cpp` holds `Is_Getting_Late` cut verbatim; `Get()` (`timeGetTime`, an OS call) is a stand-in
//! returning the value the test sets. The rest of `SysTimeClass` is `timeGetTime` and cannot be tested. 20,000 random 32-bit values and
//! the boundary 0x6fffffff, 0x70000000, 0, 0xffffffff. Predicted: 0 mismatches.

//! **Results.** 0 mismatches over 20,008 values. Planted bugs: 17391 `>=`: 9 values; 17392 threshold 0x7fffffff: 3,448; 17393 signed cast: 7,542.
//! 3 of 3 caught.

mod support;

use rts_lockstep_engine::drafts::sweep_wwlib::systimer::is_getting_late;
use support::{Check, Gen};

extern "C" {
    fn ref_st_is_getting_late(now: u32) -> i32;
}

#[test]
fn is_getting_late_matches_upstream() {
    let mut g = Gen::new(0x215_17391);
    let mut c = Check::new("SysTimeClass::Is_Getting_Late");
    let mut values: Vec<u32> = vec![0, 1, 0x6fff_fffe, 0x6fff_ffff, 0x7000_0000, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff];
    for _ in 0..20_000 {
        values.push(if g.below(4) == 0 { 0x6fff_ff00 + g.below(512) } else { g.u32() });
    }
    for v in values {
        let cpp = unsafe { ref_st_is_getting_late(v) } != 0;
        c.eq(is_getting_late(v), cpp, || format!("now {v:#x}"));
    }
    if std::env::var("L9_PLANT").is_ok() {
        c.report();
        panic!("a planted run: see the MEASURED line");
    }
    c.finish();
}
