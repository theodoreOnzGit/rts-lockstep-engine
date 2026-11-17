#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/AnimateWindowManager.cpp (AnimateWindow, AnimateWindowManager)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ProcessAnimateWindow.cpp (the eight ProcessAnimateWindow kinds)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (the three managers in init, reset and
//     update; animateSpecialPowerShortcut)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The animate window manager and the control bar's slide (GitHub #137): `client::animate_window` and
//! `client::control_bar_animate` against the C++.
//!
//! # Method
//!
//! The C++ side is the upstream text cut verbatim by build.rs (`differential/wrap_animate_window.cpp` lists the cuts
//! and the words): both managers' files and both headers, GameWindow's position and size functions, and ControlBar's
//! lines, compiled on stand-ins for the window, the display, `timeGetTime()`, the name lookup of `ControlBarParent` and
//! the shortcut bar's buttons. `timeGetTime()` is the case's clock, which the steps move on; the client feeds the Rust
//! the same way (the app's clock in milliseconds, read once a client frame).
//!
//! Each sequence has a **focus type** (the sequence's index, cycling through the eight animation types): four in five
//! registrations are of it, the others of any type, now and then an invalid one (0, 9 or -1). The setup: the display's
//! width when the control bar's managers are made (800, 1024, 640, any up to 4,000, or under 50), one to six windows at
//! -1,000 to 2,000 (x and y), 0 to 600 wide and high, one in eight hidden; `ControlBarParent` (window 0) named three
//! times in four; the shortcut bar's parent (a window, or none one time in three), 0 to 11 buttons in use with random
//! user data; the clock anywhere, one time in ten within a few seconds of its wrap. Then 1 to 250 steps: a manager's
//! `update` (three in ten), ControlBar::update's animate lines (two in ten), `registerGameWindow` (any window, needing
//! to finish two times in three, 0 to 2,000 ms, 500 one time in three, no delay half the time, else up to 400 ms),
//! `reverseAnimateWindow` (so reversals come mid-animation, and after it), the clock moved on (0 to 120 ms, one time in
//! twenty up to a minute), `reset`, `resetToRestPosition`, `init`, ControlBar::reset's lines,
//! `animateSpecialPowerShortcut`, the display's width changed, a window moved or hidden from outside. The register
//! steps pick manager 0 (the bar's) half the time.
//!
//! After **every step** both sides dump every window (position and hidden bit) and each manager (finished, reversed,
//! empty, the lists' sizes) with every animation (window, type, current, start, end and rest positions, the velocity's
//! bits, the times, the delay, finished, needs to finish): window positions frame by frame, and the finish detection.
//! The sequence's dumps must be identical. Where the Rust meets upstream's undefined behaviour (a float past `Int`: the
//! timed slide after a later timed registration set the duration to 0) the sequence stops before that step and the
//! C++ runs only the steps before it.
//!
//! *Predictions, before the first run* (`issue137_predictions.md`): 0 mismatches in 20,000 sequences (#77's
//! `differential_wt.rs` compared the manager over 2,000 cases with 0 mismatches; new here are the random display widths,
//! three managers, the control bar's lines); stopped as undefined: a few dozen to a few hundred; plants 10386-10400 all
//! caught, the weakest 10395, 10399 and 10400.
//!
//! # Results (2026-11-17, `ANIMATE_WINDOW_SEQUENCES=20000`, the default)
//!
//! - First build: the wrapper and the cuts compiled at once (predicted 1-3 compile errors: missed, low); one error in
//!   this file's Rust.
//! - First run (2,000 sequences): **0 mismatches**. The generator then took fewer reversals and resets, so that more
//!   animations run to their end before being turned around (the forward finishes had been 313 of 30,000 registrations).
//! - 20,000 sequences, 2,508,749 steps, **0 mismatches** for every focus type (2,500 sequences each). But **0 stopped as
//!   undefined** (predicted a few dozen to a few hundred: missed): a zero duration came 1 time in 3,000 registrations.
//!   The generator then gave 0 ms one registration in twenty.
//! - **Final** (that generator): 20,000 sequences, 2,495,282 steps compared frame by frame, **0 mismatches** in each of
//!   the eight focus types; **235 sequences stopped as undefined** (the timed slide's duration set to 0 under a running
//!   one), at that step. Registered by type (right, right fast, left, top, bottom, spiral, bottom timed, top fast):
//!   24,391, 24,331, 24,169, 24,344, 24,451, 24,124, 23,040, 24,708; animations seen finishing forward 2,002, 3,464,
//!   796, 712, 808, 1,605, 1,635, 4,006 and reversed 15,988, 9,509, 10,622, 10,908, 10,540, 10,780, 2,506, 9,564; the
//!   bar hidden by `ControlBar::update` after its slide out 15,671 times; 30,646 shortcut slides registered by
//!   `animateSpecialPowerShortcut`.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`, each on the final generator's 20,000 sequences: **15 of 15 caught** (2026-11-17). Predicted weakest
//! 10395, 10399 and 10400: 10399 was (97), the other two were not; 10393 (431, the bottom slide only) was not
//! predicted weak.
//!
//! | id | planted bug | caught by (sequences) |
//! |---|---|---|
//! | 10386 | `ControlBar::update` hides the bar when its manager finishes forward too | 11,766 |
//! | 10387 | `ControlBar::update` skips the shortcut bar's manager | 15,457 |
//! | 10388 | `animateSpecialPowerShortcut` animates with no used button holding a special power | 2,682 |
//! | 10389 | `animateSpecialPowerShortcut` slides from the left | 5,295 |
//! | 10390 | `ControlBar::reset` skips the shortcut bar's manager | 9,537 |
//! | 10391 | `update` leaves the plain list alone once reversed | 12,577 |
//! | 10392 | `reverseAnimateWindow` mirrors the delays against 0 | 8,813 |
//! | 10393 | the bottom slide stops without putting the window at its rest | 431 |
//! | 10394 | the timed slide measures from its start time | 1,544 |
//! | 10395 | the spiral's reverse waits for its start time | 2,361 |
//! | 10396 | the reversed start time is `now + delay` | 10,556 |
//! | 10397 | `resetToRestPosition` leaves the manager not reversed | 16,055 |
//! | 10398 | `registerGameWindow` puts every window in the plain list | 18,650 |
//! | 10399 | the left slide's reverse goes past its top speed | 97 |
//! | 10400 | `ControlBar::init` makes the bar's manager on a display 0 wide (the spiral's radius) | 4,566 |

mod support;

use rts_lockstep_engine::client::animate_window::{AnimateWindow, AnimateWindowManager};
use rts_lockstep_engine::client::control_bar_animate::{ControlBarAnimate, SpecialPowerShortcut};
use rts_lockstep_engine::client::control_bar_stage::CONTROL_BAR_PARENT;
use rts_lockstep_engine::client::window_manager::{GameWindowManager, NoGuiHost, SystemFunc};
use rts_lockstep_engine::client::window_script::{win_status, WinInstanceData};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut cap = 1 << 16;
    loop {
        let mut out = vec![0i32; cap];
        let n = unsafe { cpp::ref_animate_window_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
        if n <= cap {
            out.truncate(n);
            return out;
        }
        cap = n;
    }
}

fn dump_list(out: &mut Vec<i32>, l: &[AnimateWindow]) {
    for a in l {
        out.extend([
            a.win.map_or(-1, |w| w as i32),
            a.anim_type,
            a.cur_pos.0,
            a.cur_pos.1,
            a.start_pos.0,
            a.start_pos.1,
            a.end_pos.0,
            a.end_pos.1,
            a.rest_pos.0,
            a.rest_pos.1,
            a.vel.0.to_bits() as i32,
            a.vel.1.to_bits() as i32,
            a.start_time as i32,
            a.end_time as i32,
            a.delay as i32,
            i32::from(a.is_finished),
            i32::from(a.needs_to_finish),
        ]);
    }
}

fn dump(out: &mut Vec<i32>, wm: &GameWindowManager, nwin: usize, cb: &ControlBarAnimate) {
    out.extend([300, nwin as i32]);
    for w in 0..nwin {
        let g = &wm.tree.windows[w];
        out.extend([g.region.0, g.region.1, i32::from(g.is_hidden())]);
    }
    for m in [&cb.animate_window_manager, &cb.animate_window_manager_for_gen_shortcuts, &cb.generals_screen_animate] {
        out.extend([301, i32::from(!m.is_finished()), i32::from(m.is_reversed()), i32::from(m.is_empty()), m.must_finish_list.len() as i32, m.win_list.len() as i32]);
        dump_list(out, &m.must_finish_list);
        dump_list(out, &m.win_list);
    }
}

fn manager(cb: &mut ControlBarAnimate, m: i32) -> &mut AnimateWindowManager {
    match m {
        0 => &mut cb.animate_window_manager,
        1 => &mut cb.animate_window_manager_for_gen_shortcuts,
        _ => &mut cb.generals_screen_animate,
    }
}

const TYPE_NAMES: [&str; 8] = ["slide right", "slide right fast", "slide left", "slide top", "slide bottom", "spiral", "slide bottom timed", "slide top fast"];

#[test]
fn animate_window_matches_upstream() {
    let sequences: u64 = std::env::var("ANIMATE_WINDOW_SEQUENCES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x137_A1E7);
    let mut checks: Vec<Check> = TYPE_NAMES.iter().map(|n| Check::new(&format!("animate window, focus {n}"))).collect();
    let (mut steps, mut stopped, mut finished_fwd, mut finished_rev, mut bar_hidden, mut shortcut_reg) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut registered = [0u64; 8];
    let mut done = [[0u64; 2]; 8];
    for i in 0..sequences {
        let focus = (i % 8) as i32 + 1;
        let display = match g.below(6) {
            0 => 800,
            1 => 1024,
            2 => 640,
            3 | 4 => g.below(4001) as i32,
            _ => g.below(50) as i32,
        };
        let nwin = 1 + g.below(6) as usize;
        let parent_named = g.below(4) != 0;
        let shortcut_parent: i32 = if g.below(3) == 0 { -1 } else { g.below(nwin as u32) as i32 };
        let used = if g.below(4) == 0 { 0 } else { g.below(12) as usize };
        let bits = if g.below(4) == 0 { 0 } else { g.below(1 << 11) };
        let clock: u32 = if g.below(10) == 0 { u32::MAX - g.below(5000) } else { g.u32() };
        let mut input = vec![display, nwin as i32, i32::from(parent_named), shortcut_parent, used as i32, bits as i32, clock as i32];
        let mut wm = GameWindowManager::new((display, 600));
        for w in 0..nwin {
            let (x, y) = (g.below(3001) as i32 - 1000, g.below(3001) as i32 - 1000);
            let (ww, hh) = (g.below(601) as i32, g.below(601) as i32);
            let hidden = g.below(8) == 0;
            input.extend([x, y, ww, hh, i32::from(hidden)]);
            let id = wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, x, y, ww, hh, SystemFunc::Default, Some(WinInstanceData::new()));
            assert_eq!(id, w);
            if hidden {
                wm.win_hide(id, true);
            }
        }
        if parent_named {
            wm.tree.windows[0].inst.decorated_name_string = CONTROL_BAR_PARENT.to_vec();
        }
        let user_data: Vec<bool> = (0..used).map(|b| (bits >> b) & 1 == 1).collect();
        let shortcut = SpecialPowerShortcut { parent: (shortcut_parent >= 0).then_some(shortcut_parent as usize), button_has_user_data: &user_data };
        let mut cb = ControlBarAnimate::new(display);
        let mut now = clock;
        let mut out = Vec::new();
        let mut starts = Vec::new();
        for _ in 0..1 + g.below(250) {
            let mut s = [0i32; 7];
            match g.below(100) {
                0..=33 => s = [2, mgr_pick(&mut g), 0, 0, 0, 0, 0],
                34..=55 => s[0] = 7,
                56..=63 => {
                    let t = match g.below(30) {
                        0 => [0, 9, -1][g.below(3) as usize],
                        1..=5 => 1 + g.below(8) as i32,
                        _ => focus,
                    };
                    let ms = match g.below(20) {
                        0 => 0,
                        1..=6 => 500,
                        _ => g.below(2001) as i32,
                    };
                    let delay = if g.below(2) == 0 { 0 } else { g.below(401) as i32 };
                    s = [0, if g.below(2) == 0 { 0 } else { g.below(3) as i32 }, g.below(nwin as u32) as i32, t, i32::from(g.below(3) != 0), ms, delay];
                }
                64..=66 => s = [1, mgr_pick(&mut g), 0, 0, 0, 0, 0],
                67..=77 => s = [6, if g.below(20) == 0 { g.below(60001) as i32 } else { g.below(121) as i32 }, 0, 0, 0, 0, 0],
                78..=79 => s = [3, mgr_pick(&mut g), 0, 0, 0, 0, 0],
                80..=81 => s = [4, mgr_pick(&mut g), 0, 0, 0, 0, 0],
                82 => s = [5, mgr_pick(&mut g), 0, 0, 0, 0, 0],
                83 => s[0] = 8,
                84..=88 => s = [9, g.below(2) as i32, 0, 0, 0, 0, 0],
                89..=90 => s = [10, g.below(4001) as i32, 0, 0, 0, 0, 0],
                91..=94 => s = [11, g.below(nwin as u32) as i32, g.below(3001) as i32 - 1000, g.below(3001) as i32 - 1000, 0, 0, 0],
                _ => s = [12, g.below(nwin as u32) as i32, g.below(2) as i32, 0, 0, 0, 0],
            }
            let before = out.len();
            let snap = finished_flags(&cb);
            match s[0] {
                0 => {
                    if (1..=8).contains(&s[3]) {
                        registered[(s[3] - 1) as usize] += 1;
                    }
                    manager(&mut cb, s[1]).register_game_window(&mut wm, Some(s[2] as usize), s[3], s[4] != 0, s[5] as u32, s[6] as u32, now);
                }
                1 => manager(&mut cb, s[1]).reverse_animate_window(&mut wm, now),
                2 => {
                    let m = manager(&mut cb, s[1]);
                    let was = (m.is_finished(), m.is_reversed());
                    m.update(&mut wm, now);
                    if !was.0 && m.is_finished() {
                        if m.is_reversed() {
                            finished_rev += 1;
                        } else {
                            finished_fwd += 1;
                        }
                    }
                }
                3 => manager(&mut cb, s[1]).reset(&mut wm),
                4 => manager(&mut cb, s[1]).reset_to_rest_position(&mut wm),
                5 => manager(&mut cb, s[1]).init(),
                6 => now = now.wrapping_add(s[1] as u32),
                7 => {
                    let shown = parent_named && !wm.tree.windows[0].is_hidden();
                    cb.update(&mut wm, now, shortcut);
                    if shown && wm.tree.windows[0].is_hidden() {
                        bar_hidden += 1;
                    }
                }
                8 => cb.reset(&mut wm),
                9 => {
                    cb.animate_special_power_shortcut(&mut wm, now, s[1] != 0, shortcut);
                    if s[1] != 0 && !cb.animate_window_manager_for_gen_shortcuts.must_finish_list.is_empty() {
                        shortcut_reg += 1;
                    }
                }
                10 => wm.display.0 = s[1],
                11 => wm.win_set_position(s[1] as usize, s[2], s[3]),
                _ => wm.win_hide(s[1] as usize, s[2] != 0),
            }
            if wm.undefined.is_some() {
                out.truncate(before);
                stopped += 1;
                break;
            }
            starts.push(out.len());
            dump(&mut out, &wm, nwin, &cb);
            // (the animations that finished in this step, by type, forward and reversed)
            if s[0] == 2 || s[0] == 7 {
                for (b, a) in snap.iter().zip(finished_flags(&cb)) {
                    if !b.1 && a.1 && b.0 == a.0 {
                        done[(a.0 - 1) as usize][usize::from(a.2)] += 1;
                    }
                }
            }
            input.extend(s);
            steps += 1;
        }
        let c = cpp_seq(&input);
        let check = &mut checks[(focus - 1) as usize];
        if out != c {
            let at = out.iter().zip(c.iter()).position(|(a, b)| a != b).unwrap_or(out.len().min(c.len()));
            let step = starts.iter().rposition(|&s| s <= at).unwrap_or(0);
            let end = |v: &Vec<i32>| starts.get(step + 1).copied().unwrap_or(v.len()).min(v.len());
            let (rs, cs) = (starts[step].min(out.len()), starts[step].min(c.len()));
            check.eq(out[rs..end(&out)].to_vec(), c[cs..end(&c)].to_vec(), || format!("sequence {i}, step {step} (input {:?})", &input[..7 + 5 * nwin]));
        } else {
            check.eq(0, 0, String::new);
        }
    }
    println!(
        "animate window: {sequences} sequences, {steps} steps, {stopped} stopped as undefined; registered by type {registered:?}; {finished_fwd} forward and {finished_rev} reversed finishes seen; the bar hidden by ControlBar::update {bar_hidden} times; {shortcut_reg} shortcut slides"
    );
    for (k, n) in TYPE_NAMES.iter().enumerate() {
        println!("animate window: {n}: {} registered, {} animations finished forward, {} reversed", registered[k], done[k][0], done[k][1]);
    }
    let mut total = 0;
    for c in checks {
        if planted {
            total += c.report();
        } else {
            total += c.mismatches;
            c.finish();
        }
    }
    println!("animate window: {total} mismatched sequences in all");
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}

/// Each animation's (type, finished, its manager reversed), in the dump's order.
fn finished_flags(cb: &ControlBarAnimate) -> Vec<(i32, bool, bool)> {
    let mut v = Vec::new();
    for m in [&cb.animate_window_manager, &cb.animate_window_manager_for_gen_shortcuts, &cb.generals_screen_animate] {
        for a in m.must_finish_list.iter().chain(m.win_list.iter()) {
            v.push((a.anim_type, a.is_finished, m.is_reversed()));
        }
    }
    v
}

fn mgr_pick(g: &mut Gen) -> i32 {
    if g.below(2) == 0 {
        0
    } else {
        g.below(3) as i32
    }
}
