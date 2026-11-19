#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp (ControlBarScheme::updateAnim,
//   ControlBarSchemeManager::setControlBarScheme)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::control_bar_scheme` against upstream's (#215).
//!
//! # Method
//!
//! `updateAnim`, `findControlBarScheme` and `setControlBarScheme` are cut verbatim (build.rs, drafts_guilogic3) into
//! `differential/wrap_draft_cbscheme.cpp` (stand-ins in its header). 20,000 random cases each:
//!
//! - `updateAnim`: an animation type from -3 to 5 and some large values (only 0, `CB_ANIM_SLIDE_RIGHT`, calls `animSlideRight`).
//! - `setControlBarScheme`: 0 to 5 schemes with lower-case names from a small set (so that a search finds duplicates and misses), creation
//!   resolutions 1 to 3,000 (and sometimes negative, which `UnsignedInt / Int` turns into a huge unsigned divisor), a search name in any case, a
//!   display of 1 to 8,000 by 1 to 8,000. Compared: the current scheme's place, both multiplier floats by bits, and the number of `init` calls.
//!   The current scheme starts as the first scheme (so that a miss that fails to clear it is seen).
//!
//! Excluded: a creation resolution of 0 (upstream divides an unsigned by zero: a crash; the draft guards it with 0.0).
//! This is also the test that settles the draft's doc note: upstream's multiplier is the INTEGER quotient (1024 / 800 is 1.0).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches: the draft already says it takes the integer quotient.
//!
//! # Results
//!
//! 2026-11-19: 20,000 cases per function, **0 mismatches**, first run (prediction met). This settles the draft's note: `setControlBarScheme(name)` stores the INTEGER quotient
//! (1024 / 800 is 1.0); `setControlBarSchemeByPlayer` (:1148) and `...ByPlayerTemplate` (:1216) divide as `Real` (read, not run here). Planted bugs, 6 of 6 caught (mismatches of
//! 20,000): 15737 18,159 (every type animates); 15738 1,876 (type 1 animates); 15739 7,825 (float quotient); 15740 6,729 (x and y swapped); 15741 8,872 (a miss keeps the previous
//! scheme current); 15742 12,175 (init runs with no scheme).

mod support;

use rts_lockstep_engine::client::control_bar_scheme::{ControlBarScheme, ControlBarSchemeAnimation, ControlBarSchemeManager};
use rts_lockstep_engine::common::base_type::ICoord2D;
use rts_lockstep_engine::drafts::sweep_gui_logic::control_bar_scheme as cs;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_cbs_anim(t: i32) -> i32;
    fn ref_cbs_set(inp: *const i32, n: i32, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

const NAMES: [&str; 5] = ["alpha", "beta", "gamma", "a", "betaa"];

#[test]
fn control_bar_scheme() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_CB5C);
    let mut anim = Check::new("ControlBarScheme::updateAnim");
    let mut set = Check::new("ControlBarSchemeManager::setControlBarScheme");
    for i in 0..20_000 {
        let t = if g.below(6) == 0 { g.i32() } else { g.below(9) as i32 - 3 };
        let mut called = 0;
        let r = cs::update_anim(&ControlBarSchemeAnimation::default(), t, || called += 1);
        let c = unsafe { ref_cbs_anim(t) };
        anim.eq((r as i32, called), (c, c), || format!("type {t}"));

        let n = g.below(6) as usize;
        let mut inp = vec![n as i32];
        let mut mgr = ControlBarSchemeManager::default();
        for _ in 0..n {
            let name = NAMES[g.below(5) as usize];
            let (rx, ry) = (if g.below(12) == 0 { -(1 + g.below(5) as i32) } else { 1 + g.below(3000) as i32 }, if g.below(12) == 0 { -(1 + g.below(5) as i32) } else { 1 + g.below(3000) as i32 });
            inp.push(name.len() as i32);
            inp.extend(name.bytes().map(|b| b as i32));
            inp.extend([rx, ry]);
            mgr.schemes.push(ControlBarScheme { name: name.as_bytes().to_vec(), screen_creation_res: ICoord2D { x: rx, y: ry }, ..ControlBarScheme::default() });
        }
        let base = NAMES[g.below(5) as usize];
        let search: Vec<u8> = base.bytes().map(|b| if g.below(3) == 0 { b.to_ascii_uppercase() } else { b }).collect();
        inp.push(search.len() as i32);
        inp.extend(search.iter().map(|&b| b as i32));
        let (w, h) = (1 + g.below(8000), 1 + g.below(8000));
        inp.extend([w as i32, h as i32]);
        // the manager as the stand-in starts it: no multiplier set (-1), current = none; the first scheme is the "previous" current only through
        // the planted bug, so start with it
        mgr.current = if n > 0 { Some(0) } else { None };
        mgr.multiplier = (-1.0, -1.0);
        let mut inits = 0;
        cs::set_control_bar_scheme(&mut mgr, &search, (w, h), |_| inits += 1);
        let want = vec![mgr.current.map_or(-1, |c| c as i32), mgr.multiplier.0.to_bits() as i32, mgr.multiplier.1.to_bits() as i32, inits];
        let mut out = [0i32; 4];
        unsafe { ref_cbs_set(inp.as_ptr(), inp.len() as i32, out.as_mut_ptr()) };
        set.eq(want, out.to_vec(), || format!("case {i}: {inp:?}"));
    }
    if planted() {
        anim.report();
        set.report();
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    anim.finish();
    set.finish();
}
