#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/Thing/W3DModuleFactory.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::w3d_module_factory` against `W3DModuleFactory::init` (#215, group lc2).
//!
//! # Method
//!
//! `W3DModuleFactory::init` is cut verbatim by build.rs into `differential/wrap_draft_w3d_module_factory.cpp`; its `addModule( X )` macro (ModuleFactory.h,
//! `#classname`) is stood in by one that records the stringised name. The list of names, in order, is compared with `w3d_module_factory_init()`
//! and with `W3D_DRAW_MODULE_NAMES`. Not compared: the draw classes' factory and data procs, module type and interface mask (they need the 19 draw
//! classes), and that the draw classes exist under these names. One case only: the function has no input.
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches (19 names).
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches** (the prediction held, no draft change): 21 comparisons (the count, 19 names in order, the constant). Planted bugs, 4 of 4 caught: 16891 two names
//! swapped 2; 16892 the last name dropped 1; 16893 the order reversed 18; 16894 a name duplicated 19.

mod support;

use rts_lockstep_engine::drafts::logic_common::w3d_module_factory::*;
use support::Check;

extern "C" {
    fn ref_lc2_w3d_names(out: *mut u8, cap: i32) -> i32;
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

#[test]
fn w3d_module_factory_matches() {
    let mut c = Check::new("W3DModuleFactory::init names");
    let mut buf = vec![0u8; 4096];
    let len = unsafe { ref_lc2_w3d_names(buf.as_mut_ptr(), buf.len() as i32) };
    assert!(len > 0);
    let cpp: Vec<String> = String::from_utf8(buf[..len as usize].to_vec()).unwrap().lines().map(|s| s.to_string()).collect();
    let rust: Vec<String> = w3d_module_factory_init().into_iter().map(|s| s.to_string()).collect();
    c.eq(rust.len(), cpp.len(), || "name count".to_string());
    for (i, (r, k)) in rust.iter().zip(cpp.iter()).enumerate() {
        c.eq(r.clone(), k.clone(), || format!("name {i}"));
    }
    c.eq(W3D_DRAW_MODULE_NAMES.iter().map(|s| s.to_string()).collect::<Vec<_>>(), cpp, || "the constant".to_string());
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}
