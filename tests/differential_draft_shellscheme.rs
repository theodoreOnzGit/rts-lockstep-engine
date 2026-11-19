#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/Shell/ShellMenuScheme.cpp (ShellMenuSchemeLine::ShellMenuSchemeLine,
//   ShellMenuSchemeManager::newShellMenuScheme, setShellMenuScheme)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::shell_menu_scheme` against upstream's (#215).
//!
//! # Method
//!
//! The line constructor, `newShellMenuScheme` and `setShellMenuScheme` are cut verbatim (build.rs, drafts_guilogic3) into
//! `differential/wrap_draft_shellscheme.cpp` (stand-ins in its header), on upstream's own AsciiString.cpp. The default line is compared once;
//! 10,000 random programs of 1 to 20 operations (`new` and `set` with names `Alpha alpha ALPHA beta Gamma gamma` and the empty name, and
//! dumps) are run through both, comparing the scheme list's names in order and which one is current, at every dump.
//!
//! Excluded: `newShellMenuScheme` of a name that replaces the CURRENT scheme: upstream `delete`s the scheme `m_currentScheme` points to and
//! leaves the pointer dangling (undefined behaviour); the draft's choice there (documented) is not compared.
//!
//! Not covered: `parseLinePart` (an INI block read through `INI::initFromINI` with `parseICoord2D`, `parseColorInt`, `parseInt`: it needs the INI
//! reader and a file; the field table is four lines that the draft's `shell_menu_scheme_line_fields` repeats, and the INI parsers are checked by
//! the INI differential tests).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! The line default and `set` match. `new` mismatches where the replaced scheme lies BEFORE the current one: the draft "keeps the index as it
//! was", so it then points at the next scheme, while upstream's pointer still names the right scheme object.
//!
//! # Results
//!
//! 2026-11-19: the line default **0 mismatches**. The programs: first run **805 of 10,000 mismatches** (prediction met): `new_shell_menu_scheme` "kept the index as it was" when
//! it replaced a scheme before the current one, so the current index then named the next scheme, where upstream's pointer follows its scheme. Fixed (the index moves down). After the
//! fix **0 mismatches**; the generator first excluded dangling cases by the draft's own index, which is the thing under test, so it now tracks the current scheme by name, and
//! the old behaviour (plant 15764 = the fix off) is **853 of 10,000** with that generator. Planted bugs, 9 of 9 caught (mismatches of 10,000; line default of 1): 15745 7,329 (name not
//! lower-cased); 15746 5,941 (duplicate not replaced); 15747 3,407 (replacement kept in place); 15748 3,262 (set does not lower-case); 15749 1,624 (empty name does not clear);
//! 15750 1,265 (unknown name clears); 15751 (line width 0); 15752 (line colour 0); 15764 853 (the fix off).

mod support;

use rts_lockstep_engine::drafts::sweep_gui_logic::shell_menu_scheme::{ShellMenuSchemeLine, ShellMenuSchemeManager};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_sms_line(out: *mut i32) -> i32;
    fn ref_sms_run(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

const NAMES: [&str; 7] = ["Alpha", "alpha", "ALPHA", "beta", "Gamma", "gamma", ""];

fn put(prog: &mut Vec<i32>, s: &str) {
    prog.push(s.len() as i32);
    prog.extend(s.bytes().map(|b| b as i32));
}

fn dump(m: &ShellMenuSchemeManager) -> Vec<i32> {
    let mut v = vec![m.scheme_list.len() as i32];
    for s in &m.scheme_list {
        v.push(s.name.len() as i32);
        v.extend(s.name.iter().map(|&b| b as i32));
    }
    v.push(m.current_scheme.map_or(-1, |c| c as i32));
    v
}

fn program(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let mut prog = Vec::new();
    let mut want = Vec::new();
    let mut m = ShellMenuSchemeManager::default();
    // the generator's own model of which scheme is current, by name (the draft's index is the thing under test, so it cannot decide the exclusion)
    let mut current: Option<String> = None;
    for _ in 0..(1 + g.below(20)) {
        match g.below(5) {
            0..=1 => {
                let n = NAMES[g.below(7) as usize];
                // excluded: replacing the current scheme (a dangling pointer upstream)
                let lower = n.to_ascii_lowercase();
                if current.as_deref() == Some(lower.as_str()) {
                    continue;
                }
                prog.push(0);
                put(&mut prog, n);
                m.new_shell_menu_scheme(n.as_bytes());
            }
            2 | 3 => {
                let n = NAMES[g.below(7) as usize];
                prog.push(1);
                put(&mut prog, n);
                m.set_shell_menu_scheme(n.as_bytes());
                if n.is_empty() {
                    current = None;
                } else if m.scheme_list.iter().any(|s| s.name == n.to_ascii_lowercase().as_bytes()) {
                    current = Some(n.to_ascii_lowercase());
                }
            }
            _ => {
                prog.push(2);
                want.extend(dump(&m));
            }
        }
    }
    prog.push(2);
    want.extend(dump(&m));
    (prog, want)
}

#[test]
fn shell_menu_scheme() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_5A15);
    let mut line = Check::new("ShellMenuSchemeLine::ShellMenuSchemeLine");
    let mut run = Check::new("newShellMenuScheme, setShellMenuScheme");
    let mut out = [0i32; 6];
    unsafe { ref_sms_line(out.as_mut_ptr()) };
    let l = ShellMenuSchemeLine::default();
    line.eq(vec![l.start_pos.x, l.start_pos.y, l.end_pos.x, l.end_pos.y, l.width, l.color], out.to_vec(), || "default".to_string());
    for i in 0..10_000 {
        let (prog, want) = program(&mut g);
        let mut o = vec![0i32; 1024];
        let k = unsafe { ref_sms_run(prog.as_ptr(), prog.len() as i32, o.as_mut_ptr(), o.len() as i32) };
        assert!(k >= 0);
        o.truncate(k as usize);
        run.eq(want, o, || format!("program {i}: {prog:?}"));
    }
    if planted() {
        line.report();
        run.report();
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    line.finish();
    run.finish();
}
