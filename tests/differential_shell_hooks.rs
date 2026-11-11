#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/ShellHooks.h (the SHELL_SCRIPT_HOOK_* enumeration)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/Scripts.cpp (TheShellHookNames, SignalUIInteraction)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp (signalUIInteract, allocateFlag,
//     evaluateFlag, update's clear of m_uiInteractions)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::shell_hooks` (GitHub #142: #140's table, #139's path) against the C++.
//!
//! # Method
//!
//! The C++ side is ShellHooks.h's enum, Scripts.cpp's `TheShellHookNames` and `SignalUIInteraction`, and
//! ScriptEngine.cpp's `signalUIInteract`, `allocateFlag`, `evaluateFlag` and `update`'s two lines that clear
//! `m_uiInteractions`, cut verbatim by build.rs (`differential/wrap_shell_hooks.cpp` lists the cuts, the stand-ins and
//! the words).
//!
//! 1. **The enum:** its 30 values, `SHELL_SCRIPT_HOOK_TOTAL` last, against [`SHELL_SCRIPT_HOOK_VALUES`].
//! 2. **The table:** the 29 strings, byte for byte, against [`the_shell_hook_name`] of each hook.
//! 3. **Sequences** (`SHELL_HOOKS_CASES`, 20,000 by default) through the whole path the client takes. The C++ calls
//!    `SignalUIInteraction(i)` at once. The Rust side goes as the Bevy client does: the menu's
//!    [`signal_ui_interaction`] on the real shell host (`ShellGui`, which records the hook), then at the frame's end
//!    [`deliver_ui_interacts`] into the World's script engine (the gate's map World, made once). Both sides then dump
//!    the list from the front. Queries are `evaluateFlag` of 58 FLAG conditions (each hook's name, with value 0 or 1)
//!    on both sides: the Rust side through `ScriptEngine::evaluate_conditions` of a one-condition script, so the port's
//!    `evaluate_flag` and `allocate_flag` run; the C++ answers with `m_numFlags`, the Rust side with `num_flags`.
//!    Update's clear is `m_uiInteractions.clear()` against the port's `ui_interactions.clear()` (the line itself: the
//!    rest of `update` is `tests/differential_script_engine.rs`'s). Each sequence starts a fresh engine (the Rust side
//!    by `newMap`, with the conditions' cached flag indices 0, as the C++'s). A frame: 0 to 6 hooks raised (any of the
//!    29, often the same one twice), the frame's end, then 0 to 4 queries, and sometimes the update's clear.
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - The enum and the table: equal (the table was typed from Scripts.cpp; 29 entries, `TOTAL` 29).
//! - Sequences: 0 mismatches. The list's order is the subtle part: upstream pushes each hook at the front at once, the
//!   port records a frame's hooks and delivers them in the order raised, each at the front, which leaves the same list.
//! - Plants 9301-9307 all caught; 9303 (the last hook, `ShellLANEnteredFromGame`, missing) the fewest sequences, as
//!   only 1 in 29 hooks raised is that one.
//!
//! # Results (2026-11-11, `SHELL_HOOKS_CASES=20000`, the default)
//!
//! - The enum's 30 values and the table's 29 strings: equal.
//! - 20,000 sequences: 390,449 hooks signalled, 130,170 frames, 260,012 `evaluateFlag` queries (200,707 true), the
//!   longest list 54 hooks. **0 mismatches** on the first run, as predicted.
//! - Planted bugs (same sequences), mismatching sequences of 20,000: 9301 11,748 (and the table test); 9302 19,687 (and
//!   the table); 9303 7,498 (and the table); 9304 19,150; 9305 19,211; 9306 18,258; 9307 17,984. **7 of 7 caught**, and
//!   9303 the fewest, as predicted.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9301 | the first two names exchanged |
//! | 9302 | each hook named as the next one |
//! | 9303 | the last hook has no name (not signalled) |
//! | 9304 | a frame's hooks delivered last first |
//! | 9305 | each hook put at the back of the list |
//! | 9306 | a hook already listed is not listed again |
//! | 9307 | the recorded hooks kept after delivery (delivered again next frame) |

mod support;

use rts_lockstep_engine::client::shell::WndFiles;
use rts_lockstep_engine::client::shell_gui::ShellGui;
use rts_lockstep_engine::client::shell_hooks::{deliver_ui_interacts, signal_ui_interaction, the_shell_hook_name, SHELL_SCRIPT_HOOK_TOTAL, SHELL_SCRIPT_HOOK_VALUES};
use rts_lockstep_engine::client::window_script::WndHost;
use rts_lockstep_engine::common::dict::Dict;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::scripts::{condition_type as ct, Condition, OrCondition, Parameter, Script, ScriptList};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::script_engine::ScriptRef;
use support::{Check, Gen};

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const TOTAL: usize = SHELL_SCRIPT_HOOK_TOTAL as usize;

fn cpp_case(kind: i32, input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1 << 20];
    let n = unsafe { cpp::ref_shell_hooks_case(kind, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn put_name(out: &mut Vec<i32>, s: &[u8]) {
    out.extend(s.iter().map(|&b| i32::from(b)));
    out.push(0);
}

/// The shell's host on no files (only its hook record is used).
struct NoFiles;
impl WndHost for NoFiles {
    fn name_to_key(&mut self, _name: &[u8]) -> u32 {
        0
    }
    fn find_image(&self, _name: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn fetch_text(&self, _label: &[u8]) -> Vec<u16> {
        Vec::new()
    }
    fn display_size(&self) -> (i32, i32) {
        (800, 600)
    }
}
impl WndFiles for NoFiles {
    fn open_wnd(&mut self, _path: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

/// Condition `2j + v`: FLAG on `TheShellHookNames[j]` with value `v`, as one script of side 0 (cached index 0).
fn flag_scripts() -> ScriptList {
    let mut scripts = Vec::new();
    for j in 0..TOTAL {
        for v in 0..2 {
            let mut c = Condition::new_for_read();
            c.condition_type = ct::FLAG;
            c.num_parms = 2;
            let mut p = Parameter::new(0, 0);
            p.string = the_shell_hook_name(j as i32).map(|s| s.as_bytes().to_vec()).unwrap_or_default();
            c.parms[0] = Some(p);
            c.parms[1] = Some(Parameter::new(0, v));
            let mut s = Script::new();
            s.script_name = format!("Flag{j}_{v}").into_bytes();
            s.is_active = true;
            s.easy = true;
            s.normal = true;
            s.hard = true;
            s.conditions = vec![OrCondition { conditions: vec![c] }];
            scripts.push(s);
        }
    }
    ScriptList { groups: Vec::new(), scripts }
}

fn rust_seq(w: &mut World, input: &[i32]) -> Vec<i32> {
    w.sides.get_side_info_mut(0).expect("side 0").scripts = Some(flag_scripts());
    w.script_engine_new_map().expect("newMap");
    let mut gui = ShellGui::new(NoFiles);
    let mut out = Vec::new();
    for s in input.chunks(2) {
        match s[0] {
            0 => signal_ui_interaction(&mut gui, s[1]),
            1 => {
                deliver_ui_interacts(&mut w.script_engine, &mut gui.ui_interacts);
                out.push(10);
                out.push(w.script_engine.ui_interactions.len() as i32);
                for u in &w.script_engine.ui_interactions {
                    put_name(&mut out, u);
                }
            }
            2 => w.script_engine.ui_interactions.clear(),
            _ => {
                let mut se = std::mem::take(&mut w.script_engine);
                let r = se.evaluate_conditions(w, ScriptRef { side: 0, group: None, index: s[1] as usize }, None, None);
                let n = se.num_flags;
                w.script_engine = se;
                out.extend([20, i32::from(r.expect("evaluateFlag")), n]);
            }
        }
    }
    out
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let mut v = Vec::new();
    let frames = 1 + g.below(12);
    let mut last = g.below(TOTAL as u32) as i32;
    for _ in 0..frames {
        for _ in 0..g.below(7) {
            if g.below(4) != 0 {
                last = g.below(TOTAL as u32) as i32;
            }
            v.extend([0, last]);
        }
        v.extend([1, 0]);
        for _ in 0..g.below(5) {
            // (mostly a hook raised recently, so the list's answer is often true)
            let j = if g.below(2) == 0 { last } else { g.below(TOTAL as u32) as i32 };
            v.extend([3, 2 * j + g.below(2) as i32]);
        }
        if g.below(3) == 0 {
            v.extend([2, 0]);
        }
    }
    v
}

#[test]
fn the_enum_and_the_table_are_upstreams() {
    let _guard = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let values: Vec<i32> = SHELL_SCRIPT_HOOK_VALUES.iter().map(|(_, v)| *v).collect();
    assert_eq!(values, cpp_case(0, &[]), "ShellHooks.h's values");
    let mut names = Vec::new();
    for i in 0..TOTAL {
        put_name(&mut names, the_shell_hook_name(i as i32).unwrap_or("").as_bytes());
    }
    let c = cpp_case(1, &[]);
    let text = |v: &[i32]| v.iter().map(|&b| if b == 0 { '|' } else { b as u8 as char }).collect::<String>();
    println!("C++ TheShellHookNames: {}", text(&c));
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    if planted && names != c {
        println!("MEASURED: the table differs (rust {})", text(&names));
        return;
    }
    assert_eq!(text(&names), text(&c), "TheShellHookNames");
}

#[test]
fn signalled_hooks_reach_the_flags_as_upstream() {
    let _guard = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: u64 = std::env::var("SHELL_HOOKS_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut w = rts_lockstep_engine::logic::world::gate_scenario::gate_map_world();
    if w.sides.get_num_sides() == 0 {
        w.sides.add_side(&Dict::default());
    }
    let mut g = Gen::new(0x142_0139);
    let mut whole = Check::new("the shell's hooks through signalUIInteract and evaluateFlag");
    let (mut signals, mut frames, mut queries, mut fired, mut longest) = (0u64, 0u64, 0u64, 0u64, 0i32);
    for i in 0..cases {
        let input = gen_case(&mut g);
        let r = rust_seq(&mut w, &input);
        let c = cpp_case(2, &input);
        for s in input.chunks(2) {
            match s[0] {
                0 => signals += 1,
                1 => frames += 1,
                3 => queries += 1,
                _ => {}
            }
        }
        // (the C++ records: count the true answers and the longest list)
        let mut k = 0;
        while k < c.len() {
            match c[k] {
                10 => {
                    longest = longest.max(c[k + 1]);
                    let mut n = c[k + 1];
                    k += 2;
                    while n > 0 {
                        while c[k] != 0 {
                            k += 1;
                        }
                        k += 1;
                        n -= 1;
                    }
                }
                20 => {
                    fired += c[k + 1] as u64;
                    k += 3;
                }
                _ => panic!("an unknown C++ record {} at {k}", c[k]),
            }
        }
        if r != c && whole.mismatches < 4 {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(8);
            println!("sequence {i}: input {input:?}\n  rust {:?}\n  c++  {:?}", &r[k0..(k + 24).min(r.len())], &c[k0..(k + 24).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences: {signals} hooks signalled, {frames} frames, {queries} evaluateFlag queries ({fired} true), longest list {longest}");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
