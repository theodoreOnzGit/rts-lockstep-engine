#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp (the small members the draft translates)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_script::script_engine` against `ScriptEngine.cpp` (#215, group lc2).
//!
//! # Method
//!
//! `addActionTemplateInfo`, `addConditionTemplateInfo`, `removeSequentialScript`, `hasUnitCompletedSequentialScript`,
//! `hasTeamCompletedSequentialScript`, `getEnableVTune`, `setEnableVTune` and `debugVictory` are cut verbatim by build.rs into
//! `differential/wrap_draft_script_engine.cpp` (stand-ins listed there: a `Template` of the four string fields, tables of 6 actions and 5
//! conditions, a recording `executeAction`). A case is a table of internal names drawn from 6 short strings (so duplicates, case variants, the empty
//! name and misses all occur) with random UI names and help text, and one incoming entry; compared: every field of every table entry after
//! the call (20,000 cases for actions, 20,000 for conditions), and the draft's `bool` against a direct scan for a matching name (upstream is
//! `void`: the bool is the draft's own, "found"). The constants (`false` x5) and the action type `debugVictory` hands to `executeAction` are
//! compared too. `DEBUG_LOG`'s message is compiled out of release and is not compared; `AdjustDebugVariableData`, `dumpPriorityInfo` and the
//! other excluded members are listed in the draft's header.
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches** (the prediction held, no draft change): 40,000 table comparisons (20,000 each of actions and conditions), 40,000 "found" checks, and the
//! constants and `debugVictory` (action type 3, `VICTORY`). Planted bugs, 10 of 10 caught (table / found / constants): 16881 the LAST matching name wins 6,913;
//! 16882 `ui_name2` not copied 10,337; 16883 help text taken from `ui_name` 10,367; 16884 conditions match on `ui_name` 14,945 (+9,714 found); 16885 `ui_name`/`ui_name2`
//! swapped 9,529; 16886 help text not copied 9,402; 16887 `hasUnitCompletedSequentialScript` true 1; 16888 `getEnableVTune` true 1; 16889 `debugVictory` builds DEFEAT 1;
//! 16890 the draft's bool false on a match 12,391 (found check).

mod support;

use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::drafts::sweep_script::script_engine::*;
use rts_lockstep_engine::logic::script_templates::script_templates;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc2_se_templates(which: i32, tbl: *const i32, tpl: *const i32, out: *mut i32) -> i32;
    fn ref_lc2_se_misc(out: *mut i32) -> i32;
    fn ref_lc2_se_victory() -> i32;
}

static CPP: Mutex<()> = Mutex::new(());
const IDS: [&str; 6] = ["", "n0", "n1", "N0", "u", "h"];

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish_all(checks: Vec<Check>) {
    if planted() {
        for c in checks {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    } else {
        for c in checks {
            c.finish();
        }
    }
}

fn template(ids: &[i32]) -> Template {
    let b = |i: i32| IDS[i as usize].as_bytes().to_vec();
    Template { internal_name: b(ids[0]), ui_name: b(ids[1]), ui_name2: b(ids[2]), help_text: b(ids[3]) }
}

fn ids_of(t: &Template) -> [i32; 4] {
    let f = |v: &[u8]| IDS.iter().position(|s| s.as_bytes() == v).map(|i| i as i32).unwrap_or(-1);
    [f(&t.internal_name), f(&t.ui_name), f(&t.ui_name2), f(&t.help_text)]
}

#[test]
fn script_engine_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_5E);
    let mut ct = Check::new("add*TemplateInfo");
    let mut cf = Check::new("found (draft's bool)");
    let mut cm = Check::new("constants and debugVictory");
    for which in 0..2 {
        let n = if which == 0 { 6usize } else { 5 };
        for case in 0..20_000 {
            let tbl: Vec<i32> = (0..4 * n).map(|k| if k % 4 == 0 && g.below(4) != 0 { 1 + g.below(3) as i32 } else { g.below(6) as i32 }).collect();
            let tpl: Vec<i32> = (0..4).map(|k| if k == 0 { g.below(5) as i32 } else { g.below(6) as i32 }).collect();
            let mut cpp = vec![0i32; 4 * n];
            let len = unsafe { ref_lc2_se_templates(which, tbl.as_ptr(), tpl.as_ptr(), cpp.as_mut_ptr()) };
            assert_eq!(len as usize, 4 * n);
            let mut table: Vec<Template> = tbl.chunks(4).map(template).collect();
            let incoming = template(&tpl);
            let found = if which == 0 { add_action_template_info(&mut table, &incoming) } else { add_condition_template_info(&mut table, &incoming) };
            let rust: Vec<i32> = table.iter().flat_map(ids_of).collect();
            ct.eq(rust, cpp, || format!("case {case}: which {which}, table {tbl:?}, incoming {tpl:?}"));
            let expected = tbl.chunks(4).any(|e| e[0] == tpl[0]);
            cf.eq(found, expected, || format!("case {case}: which {which}, table {tbl:?}, incoming {tpl:?}"));
        }
    }
    let mut cpp = [0i32; 5];
    assert_eq!(unsafe { ref_lc2_se_misc(cpp.as_mut_ptr()) }, 5);
    let before = get_enable_vtune();
    set_enable_vtune(true);
    let after_true = get_enable_vtune();
    set_enable_vtune(false);
    let rust = [has_unit_completed_sequential_script() as i32, has_team_completed_sequential_script() as i32, before as i32, after_true as i32, get_enable_vtune() as i32];
    cm.eq(rust.to_vec(), cpp.to_vec(), || "removeSequentialScript, hasUnit, hasTeam, getEnableVTune, setEnableVTune".to_string());
    remove_sequential_script();
    let victory = unsafe { ref_lc2_se_victory() };
    let mut names = NameKeyGenerator::new();
    let templates = script_templates(&mut names);
    cm.eq(debug_victory(&templates).action_type, victory, || "debugVictory's action type".to_string());
    finish_all(vec![ct, cf, cm]);
}
