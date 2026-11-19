// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui2) of the draft `src/drafts/sweep_common_client/function_lexicon.rs`
//! (`FunctionLexicon::load_table`, `key_to_func`, `find_function`, `validate`, `game_win_draw_func`,
//! `win_layout_init_func`) against upstream `FunctionLexicon.cpp`'s `loadTable`, `keyToFunc`, `findFunction`, `validate`,
//! `gameWinDrawFunc` and `winLayoutInitFunc`, cut verbatim by build.rs (`drafts_ui2`;
//! `differential/wrap_draft_function_lexicon.cpp` lists the cuts and the stand-ins).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). Each starts with every table NULL, then 1 to 24 steps: `loadTable`
//! of a table of 0 to 7 entries at a random index (names from a pool of 12, so names repeat within and across tables; funcs from
//! a pool of 8 or, for a third of the sequences, distinct, so `validate` is often TRUE; function 0 is NULL), and lookups:
//! `findFunction`, `gameWinDrawFunc`, `winLayoutInitFunc` (each at `TABLE_ANY` or a table index) and `keyToFunc` on an
//! installed table, with keys from 0 to 13 (0 is NAMEKEY_INVALID), and `validate`. Both sides run the same name-key generator
//! (keys 1, 2, 3 in order of first sight); the answers (the keys `loadTable` assigned, the functions, the validity)
//! are compared in order.
//!
//! **Stand-in choices (stated):** the name "n11" is given NAMEKEY_INVALID by the generator on both sides. The real generator
//! never returns it, but upstream's tables end with an entry whose key is NAMEKEY_INVALID and every walk stops there, so a
//! name with key 0 exercises the draft's stop at it. The draft's tables are slices without the terminator entry.
//! **Excluded input:** an out-of-range table index (undefined upstream: `m_tables[index]` past the array); two table slots
//! holding the very same array (upstream's `sourceEntry != lookAtEntry` compares addresses, the draft compares positions);
//! looking a table up before it was loaded at `TABLE_ANY` is covered (NULL tables), at one index the table is NULL and the
//! walk returns NULL on both sides.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches over 20,000 sequences: the draft is a close reading of the C++. Plants 15466 to 15473 all caught; 15466
//! (no stop at a key of NAMEKEY_INVALID) and 15469 (last match instead of first) the fewest: they need a repeated name
//! or a key-0 name in a table and a lookup that reaches it.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`:
//!
//! | id | planted bug |
//! |---|---|
//! | 15466 | `keyToFunc` does not stop at an entry whose key is NAMEKEY_INVALID |
//! | 15467 | `findFunction` at `TABLE_ANY` skips the last table |
//! | 15468 | `findFunction` at `TABLE_ANY` searches the tables last to first |
//! | 15469 | `keyToFunc` returns the last matching entry, not the first |
//! | 15470 | `validate` does not tell tables apart when excluding the entry itself (compares only the position) |
//! | 15471 | `validate` also takes entries with a NULL function as a source |
//! | 15472 | `gameWinDrawFunc` at `TABLE_ANY` searches the plain draw table first |
//! | 15473 | `winLayoutInitFunc` at `TABLE_ANY` does not fall back to the init table |
//!
//! # Results (2026-11-19)
//!
//! 20,000 sequences: **0 mismatches** on the first run, as predicted. Planted bugs (mismatching sequences of 20,000): 15466 2,372; 15467 884; 15468 2,007; 15469 2,606; 15470 307; 15471 1,885; 15472 26; 15473 455. **8 of 8 caught.** **Prediction missed:** the fewest was 15472 (`gameWinDrawFunc` order, which needs the same key in both draw tables with different functions), not 15466 or 15469.
//! Mismatch before fixes: none to fix; no draft changed behaviour. The `DIFF` line is printed by the run.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::function_lexicon::*;
use std::collections::HashMap;
use support::{Check, Gen};

extern "C" {
    fn ref_u2_fl_run(input: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

const NAMES: [&str; 12] = ["n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11"];

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Check) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    println!("DIFF {} compared {} mismatches {} (sequences)", chk.name, chk.compared, chk.mismatches);
    if planted {
        assert!(chk.mismatches > 0, "plant not caught");
    } else {
        chk.finish();
    }
}

fn ti(i: i32) -> TableIndex {
    use TableIndex::*;
    [Any, GameWinSystem, GameWinInput, GameWinTooltip, GameWinDeviceDraw, GameWinDraw, WinLayoutInit, WinLayoutDeviceInit, WinLayoutUpdate, WinLayoutShutdown][(i + 1) as usize]
}

fn fo(f: i32) -> Option<u32> {
    if f == 0 { None } else { Some(f as u32) }
}
fn of(f: Option<u32>) -> i32 {
    f.map_or(0, |x| x as i32)
}

#[test]
fn function_lexicon_matches_upstream() {
    let mut g = Gen::new(0x215_2_02);
    let mut chk = Check::new("FunctionLexicon tables and lookups");
    for _ in 0..cases() {
        let mut lex: FunctionLexicon<u32> = FunctionLexicon::new();
        let mut keys: HashMap<&'static str, i32> = HashMap::new();
        let distinct = g.below(3) == 0;
        let mut next_func = 1;
        let mut loaded: Vec<usize> = Vec::new();
        let mut input: Vec<i32> = Vec::new();
        let mut rust: Vec<i32> = Vec::new();
        let steps = 1 + g.below(24);
        for k in 0..steps {
            let kind = if k < 2 { 0 } else { [0, 0, 1, 1, 1, 2, 3, 4, 5][g.below(9) as usize] };
            match kind {
                0 => {
                    let index = g.below(9) as i32;
                    let count = g.below(8) as i32;
                    input.extend([0, index, count]);
                    let mut table = Vec::new();
                    for _ in 0..count {
                        let name = g.below(12) as i32;
                        let func = if distinct {
                            if g.below(6) == 0 { 0 } else { next_func += 1; next_func }
                        } else {
                            g.below(9) as i32
                        };
                        input.extend([name, func]);
                        table.push(TableEntry { key: 0, name: NAMES[name as usize], func: fo(func) });
                    }
                    lex.load_table(table, ti(index), &mut |n: &str| {
                        if n == "n11" {
                            return NAMEKEY_INVALID;
                        }
                        let next = keys.len() as i32 + 1;
                        *keys.entry(NAMES[NAMES.iter().position(|x| *x == n).unwrap()]).or_insert(next)
                    });
                    for e in &lex.tables[index as usize] {
                        rust.push(e.key);
                    }
                    if !loaded.contains(&(index as usize)) {
                        loaded.push(index as usize);
                    }
                }
                2 => {
                    input.push(2);
                    rust.push(lex.validate() as i32);
                }
                _ => {
                    let key = g.below(14) as i32;
                    if kind == 5 {
                        // keyToFunc on a table: an installed one, or a NULL one
                        let index = if !loaded.is_empty() && g.below(5) != 0 { loaded[g.below(loaded.len() as u32) as usize] } else { g.below(9) as usize };
                        input.extend([5, key, index as i32]);
                        rust.push(of(FunctionLexicon::key_to_func(key, &lex.tables[index])));
                    } else {
                        let index = if g.below(2) == 0 { -1 } else { g.below(9) as i32 };
                        input.extend([kind as i32, key, index]);
                        rust.push(of(match kind {
                            1 => lex.find_function(key, ti(index)),
                            3 => lex.game_win_draw_func(key, ti(index)),
                            _ => lex.win_layout_init_func(key, ti(index)),
                        }));
                    }
                }
            }
        }
        let mut out = vec![0i32; 512];
        let n = unsafe { ref_u2_fl_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), 512) } as usize;
        assert!(n <= 512);
        out.truncate(n);
        chk.eq(rust, out, || format!("{input:?}"));
    }
    finish(chk);
}
