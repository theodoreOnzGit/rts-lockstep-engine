#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarCommandProcessing.cpp (ControlBar::processCommandTransitionUI)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::control_bar_command_processing` against upstream's (#215).
//!
//! # Method
//!
//! `processCommandTransitionUI` and `enum ControlBarContext` are cut verbatim (build.rs, drafts_guilogic3) into
//! `differential/wrap_draft_cbtrans.cpp`. The input space is tiny, so it is covered EXHAUSTIVELY: every one of the nine contexts times the three
//! selections (no drawable, a drawable without an object, a drawable with an object): 27 cases. Compared: the status and whether
//! `switchToContext(CB_CONTEXT_NONE, NULL)` was called. The draft's single flag "the drawable has an object" stands for upstream's
//! `drawable != NULL && drawable->getObject() != NULL`.
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-19: all 27 inputs, **0 mismatches** (prediction met). Planted bugs, 5 of 5 caught (mismatches of 27): 15753 2 (multi-select not exempt); 15754 2 (switches from
//! NONE); 15755 2 (switches from the observer info); 15756 16 (reports Used with nothing selected); 15757 11 (switches when something is selected).

mod support;

use rts_lockstep_engine::drafts::sweep_gui_logic::control_bar_command_processing as cp;
use std::sync::Mutex;
use support::Check;

extern "C" {
    fn ref_cbt(context: i32, selection: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

/// ControlBarContext in the order of ControlBar.h:581-590: NONE, COMMAND, STRUCTURE_INVENTORY, BEACON, UNDER_CONSTRUCTION, MULTI_SELECT,
/// OBSERVER_INFO, OBSERVER_LIST, OCL_TIMER.
fn kind(context: i32) -> cp::ControlBarContextKind {
    match context {
        0 => cp::ControlBarContextKind::None,
        5 => cp::ControlBarContextKind::MultiSelect,
        6 => cp::ControlBarContextKind::ObserverInfo,
        7 => cp::ControlBarContextKind::ObserverList,
        _ => cp::ControlBarContextKind::Other,
    }
}

#[test]
fn transition_ui() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = Check::new("ControlBar::processCommandTransitionUI");
    for context in 0..9 {
        for selection in 0..3 {
            let mut switched = 0;
            let st = cp::process_command_transition_ui(kind(context), selection == 2, || switched = 1);
            let want = (st == cp::CbCommandStatus::Used) as i32 | (switched << 1);
            c.eq(want, unsafe { ref_cbt(context, selection) }, || format!("context {context} selection {selection}"));
        }
    }
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    c.finish();
}
