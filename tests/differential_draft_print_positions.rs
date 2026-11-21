// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/control_bar_print_positions.rs` (`print_info_recursive`,
//! `print_offsets_from_control_bar_parent`) against upstream `ControlBarPrintPositions.cpp`'s `PrintInfoRecursive` and
//! `PrintOffsetsFromControlBarParent`, cut verbatim by build.rs (`drafts_ui3b`; `differential/wrap_draft_print_positions.cpp`, whose stand-ins,
//! `fopen`/`fclose` on an in-memory stream among them, are listed there).
//!
//! # Method
//!
//! 20,000 cases (fixed seed, `DRAFT_CASES` overrides). A forest of 1 to 8 windows made on the verified `GameWindowManager` with `win_create`
//! (each a top-level window or the child of an earlier one; position -500 to 2,000, size 1 to 2,000, a decorated name of 0 to 12 printable ASCII
//! bytes, `%` and spaces included), whose first-child and next-sibling links the C++'s stand-in windows are given as the Rust has them. Half the
//! cases call `PrintInfoRecursive(window)` on a random window (or NULL) with a stream and compare the text; half call
//! `PrintOffsetsFromControlBarParent()` with the control bar parent present or not, the layout loaded or not and a random first window, and
//! compare whether the file was opened and its text. EXCLUDED: names with non-ASCII or NUL bytes (the draft prints through a lossy UTF-8
//! conversion, upstream `%s` stops at a NUL); a failing `fopen` (the draft's caller writes the file); the layout's destruction (the caller's).
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches. The likeliest miss: the position, which the draft reads from the region's low corner and upstream from `winGetPosition`.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21101 | `AltPosition` prints the size |
//! | 21102 | `AltSize` prints Y before X |
//! | 21103 | the block ends with `END` and one newline, not two |
//! | 21104 | the next sibling is printed before the first child |
//! | 21105 | the siblings are not visited |
//! | 21106 | `AltPosition` prints the region's high corner |
//! | 21107 | a layout that did not load is printed (as empty text) |
//! | 21108 | the control bar parent's absence is not checked |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences**, max ULP 0 (nothing is floating point here). No draft change was needed. Prediction held: the position the draft reads from the region's low corner is `winGetPosition`.
//! Planted bugs (of 20,000): 21101 14,113; 21102 14,113; 21103 14,113; 21104 2,043; 21105 5,918; 21106 14,113; 21107 1,484; 21108 1,955. **8 of 8 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::window_manager::GameWindowManager;
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::control_bar_print_positions::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_print_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn name(g: &mut Gen) -> Vec<u8> {
    let n = g.below(13) as usize;
    (0..n).map(|_| if g.below(8) == 0 { b'%' } else { 0x20 + g.below(0x5f) as u8 }).collect()
}

#[test]
fn print_positions_match_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_03);
    let mut chk = Check::new("ControlBar PrintInfoRecursive / PrintOffsets");
    for _ in 0..cases(20_000) {
        let n = 1 + g.below(8) as usize;
        let mut wm = GameWindowManager::new((800, 600));
        let mut h = NoHost;
        let mut ids: Vec<WinId> = Vec::new();
        let mut geo: Vec<(i32, i32, i32, i32)> = Vec::new();
        let mut names: Vec<Vec<u8>> = Vec::new();
        for i in 0..n {
            let parent = if i > 0 && g.below(3) != 0 { Some(ids[g.below(i as u32) as usize]) } else { None };
            let (x, y, w, hh) = (g.below(2501) as i32 - 500, g.below(2501) as i32 - 500, 1 + g.below(2000) as i32, 1 + g.below(2000) as i32);
            let id = wm.win_create(&mut h, parent, 0, x, y, w, hh, rts_lockstep_engine::client::window_manager::SystemFunc::Default, None);
            let nm = name(&mut g);
            wm.tree.windows[id].inst.decorated_name_string = nm.clone();
            ids.push(id);
            geo.push((x, y, w, hh));
            names.push(nm);
        }
        let idx = |w: Option<WinId>| w.map_or(0u32, |w| ids.iter().position(|&x| x == w).unwrap() as u32 + 1);
        let kind = g.below(2);
        let win = if g.below(8) == 0 { None } else { Some(ids[g.below(n as u32) as usize]) };
        let parent_present = g.below(4) != 0;
        let layout_answered = g.below(5) != 0;
        let layout_first = if g.below(8) == 0 { None } else { Some(ids[g.below(n as u32) as usize]) };
        let mut input: Vec<u32> = vec![kind, idx(win), u32::from(parent_present), u32::from(layout_answered), idx(layout_first), n as u32];
        for i in 0..n {
            let (x, y, w, hh) = geo[i];
            input.extend([x as u32, y as u32, w as u32, hh as u32]);
            input.push(idx(wm.tree.windows[ids[i]].children.first().copied()));
            input.push(idx(wm.next_of(ids[i])));
            input.push(names[i].len() as u32);
            input.extend(names[i].iter().map(|&b| u32::from(b)));
        }
        let mut rust: Vec<u32> = Vec::new();
        let text = if kind == 0 {
            let mut out = String::new();
            print_info_recursive(&wm, win, &mut out);
            rust.push(1);
            Some(out)
        } else {
            let t = print_offsets_from_control_bar_parent(&wm, parent_present, layout_first, layout_answered);
            rust.push(u32::from(t.is_some()));
            t
        };
        let text = text.unwrap_or_default();
        rust.push(text.len() as u32);
        rust.extend(text.bytes().map(u32::from));
        let mut buf = vec![0u32; 1 << 16];
        let len = unsafe { ref_ui3b_print_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
