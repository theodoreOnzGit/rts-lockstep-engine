#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarPopupDescription.cpp (the update function,
//     showBuildTooltipLayout, repopulateBuildTooltipLayout, hideBuildTooltipLayout, deleteBuildTooltipLayout)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (commandButtonTooltip, ControlBar::update's
//     popup part)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::build_tooltip` (GitHub #138) against the C++.
//!
//! # Method
//!
//! The C++ side is ControlBarPopupDescription.cpp's statics, update function, `showBuildTooltipLayout`,
//! `repopulateBuildTooltipLayout`, `hideBuildTooltipLayout` and `deleteBuildTooltipLayout`, ControlBar.cpp's
//! `commandButtonTooltip` and `ControlBar::update`'s popup part, cut verbatim by build.rs
//! (`differential/wrap_build_tooltip.cpp` lists the cuts, the stand-ins and the words). Both sides run the same
//! sequence over eight windows and record each hide of the popup's layout and each `populateBuildTooltipLayout`, then
//! the popup's mark, the remembered window and the layout's hidden bit after each step.
//!
//! Each case: eight windows, each a push button (with a command, or none), a user window, static text, or another
//! style, with a tooltip delay of -1, 0, 50, 100, 200, 500 or random to 2,000; the layout shown or hidden. Then 1 to
//! 120 steps, mostly frames as the client runs them (the bar's update, then a mouse event's callback over a window,
//! often the same one for a while, or none; the clock moving 0 to 60 ms), with `hideBuildTooltipLayout`,
//! `deleteBuildTooltipLayout` (a script's), `repopulateBuildTooltipLayout` (the stages'), jumps of the clock up to
//! 5,000 ms, and the flags turned on and off: tooltips disabled, the game ending, a replay game, the quit menu, the
//! disconnect screen (and no disconnect menu at all), `m_animateWindows`.
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - 0 mismatches over 20,000 sequences: the only arithmetic is the unsigned wait (`beginWaitTime + delay < now`),
//!   which the translation wraps as the C++ does; a delay of -1 at clock 0 (`0 + 0xFFFFFFFF`) never passes on either
//!   side.
//! - No abort: the animation's branches are dead (`useAnimation` FALSE).
//! - Every plant 9286-9300 caught; 9300 (`repopulate` past a non-push-button) and 9293 (a push button without a
//!   command) the fewest.
//!
//! # Results (2026-11-11, `BUILD_TOOLTIP_CASES=20000`, the default)
//!
//! 20,000 sequences, 3,124,154 steps: the popup filled 46,699 times, shown after 601,518 steps, its layout hidden 88,779
//! times. **0 mismatches** on the first run, and no abort (the animation's branches stayed dead), as predicted.
//!
//! Planted bugs (same sequences), mismatching sequences of 20,000: 9286 4,419; 9287 2,381; 9288 19,047; 9289 668;
//! 9290 13,296; 9291 10,087; 9292 10,087; 9293 2,812; 9294 1,370; 9295 1,382; 9296 4,353; 9297 616; 9298 16,463; 9299
//! 13,960; 9300 6,451. **15 of 15 caught.** **Prediction missed:** 9300 and 9293 were not the fewest; 9297 (the game's
//! end in the update function, 616) and 9289 (`<=`, which differs only when an event comes on the exact millisecond
//! the wait ends, 668) were. 9297 needs the game to end while the popup shows, and the update function's later
//! `!getShowBuildTooltipLayout()` hides it anyway one frame later unless a call that frame marked it wanted. 9291 and
//! 9292 catch the same 10,087 sequences: each leaves the shown popup's state wrong at the same step (the layout shown,
//! or `prevWindow` kept, both only when another window's call meets a shown popup).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9286 | `showBuildTooltipLayout` ignores the tooltips being disabled |
//! | 9287 | it ignores the game ending |
//! | 9288 | a call over the same window does not mark the popup wanted |
//! | 9289 | the wait passes at `<=` |
//! | 9290 | the wait ignores `isInitialized` (the popup is refilled every call) |
//! | 9291 | a call over another window leaves the shown popup |
//! | 9292 | a call over another window keeps `prevWindow` |
//! | 9293 | a push button without a command fills the popup |
//! | 9294 | the replay game does not stop it |
//! | 9295 | the quit menu does not stop it |
//! | 9296 | a window of any style fills the popup |
//! | 9297 | the update function does not hide it when the game ends |
//! | 9298 | `deleteBuildTooltipLayout` keeps `prevWindow` |
//! | 9299 | `ControlBar::update` does not clear the mark |
//! | 9300 | `repopulateBuildTooltipLayout` refills from a window that is not a push button |

mod support;

use rts_lockstep_engine::client::build_tooltip::{command_button_tooltip, BuildTooltip, BuildTooltipHost};
use rts_lockstep_engine::client::window_script::{gws, WinId};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

/// The C++ stand-ins' world.
struct Env {
    style: [u32; 8],
    delay: [i32; 8],
    command: [Option<usize>; 8],
    hidden: bool,
    now: u32,
    tooltips_disabled: bool,
    ending: bool,
    replay: bool,
    quit_menu: bool,
    disconnect: Option<bool>,
    out: Vec<i32>,
}

impl BuildTooltipHost for Env {
    fn tooltips_disabled(&self) -> bool {
        self.tooltips_disabled
    }
    fn is_game_ending(&self) -> bool {
        self.ending
    }
    fn time_get_time(&self) -> u32 {
        self.now
    }
    fn win_style(&self, w: WinId) -> u32 {
        self.style[w]
    }
    fn tooltip_delay(&self, w: WinId) -> i32 {
        self.delay[w]
    }
    fn gadget_button_command(&self, w: WinId) -> Option<usize> {
        self.command[w]
    }
    fn is_in_replay_game(&self) -> bool {
        self.replay
    }
    fn is_quit_menu_visible(&self) -> bool {
        self.quit_menu
    }
    fn disconnect_menu_visible(&self) -> bool {
        self.disconnect == Some(true)
    }
    fn populate_build_tooltip_layout(&mut self, command: Option<usize>, tooltip_win: Option<WinId>) {
        self.out.extend([2, command.map_or(-1, |c| c as i32), tooltip_win.map_or(-1, |w| w as i32), 0]);
    }
    fn layout_is_hidden(&self) -> bool {
        self.hidden
    }
    fn layout_hide(&mut self, hide: bool) {
        self.out.extend([1, i32::from(hide), 0, 0]);
        self.hidden = hide;
    }
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut env = Env {
        style: [0; 8],
        delay: [0; 8],
        command: [None; 8],
        hidden: input[24] != 0,
        now: 0,
        tooltips_disabled: false,
        ending: false,
        replay: false,
        quit_menu: false,
        disconnect: Some(false),
        out: Vec::new(),
    };
    for i in 0..8 {
        env.style[i] = input[i * 3] as u32;
        env.delay[i] = input[i * 3 + 1];
        let c = input[i * 3 + 2];
        env.command[i] = (c >= 0).then_some((c & 15) as usize);
    }
    let mut bar = BuildTooltip::default();
    for w in input[25..].chunks(4) {
        match w[0] {
            0 => command_button_tooltip(&mut bar, (w[1] & 7) as usize, &mut env),
            1 => bar.update(&mut env),
            2 => bar.hide_build_tooltip_layout(&mut env),
            3 => bar.delete_build_tooltip_layout(&mut env),
            4 => bar.repopulate_build_tooltip_layout(&mut env),
            5 => env.now = env.now.wrapping_add(w[1] as u32),
            _ => match w[1] {
                0 => env.tooltips_disabled = w[2] != 0,
                1 => env.ending = w[2] != 0,
                2 => env.replay = w[2] != 0,
                3 => env.quit_menu = w[2] != 0,
                4 => env.disconnect = Some(w[2] != 0),
                5 => env.disconnect = None,
                // (`m_animateWindows`: read only beside `!useAnimation`, which is always true)
                _ => {}
            },
        }
        let prev = bar.prev_window.map_or(-1, |p| p as i32);
        env.out.extend([100, i32::from(bar.show_build_tooltip_layout), prev, i32::from(env.hidden)]);
    }
    env.out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 200_000];
    let n = unsafe { cpp::ref_build_tooltip_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let mut v = Vec::new();
    for _ in 0..8 {
        let style = match g.below(6) {
            0..=2 => gws::PUSH_BUTTON,
            3 => gws::USER_WINDOW,
            4 => gws::STATIC_TEXT,
            _ => [gws::CHECK_BOX, gws::ENTRY_FIELD, 0, gws::USER_WINDOW | gws::PUSH_BUTTON][g.below(4) as usize],
        };
        let delay = [-1, 0, 50, 100, 200, 500, g.below(2001) as i32][g.below(7) as usize];
        let command = if g.below(5) == 0 { -1 } else { g.below(16) as i32 };
        v.extend([style as i32, delay, command]);
    }
    v.push(i32::from(g.below(3) != 0));
    let steps = 1 + g.below(120);
    let mut over = g.below(9) as i32;
    for _ in 0..steps {
        match g.below(40) {
            0 => v.extend([2, 0, 0, 0]),
            1 => v.extend([3, 0, 0, 0]),
            2 => v.extend([4, 0, 0, 0]),
            3 => v.extend([5, g.below(5001) as i32, 0, 0]),
            4..=6 => {
                let k = g.below(7) as i32;
                let on = i32::from(if k == 1 { g.below(4) == 0 } else { g.below(2) == 0 });
                v.extend([6, k, on, 0]);
            }
            _ => {
                // a client frame: the bar's update, the mouse's event over a window (8: none), the clock
                if g.below(8) == 0 {
                    over = g.below(9) as i32;
                }
                v.extend([1, 0, 0, 0]);
                if over < 8 {
                    v.extend([0, over, 0, 0]);
                }
                v.extend([5, g.below(61) as i32, 0, 0]);
            }
        }
    }
    v
}

#[test]
fn the_popup_description_shows_and_hides_as_upstream() {
    let cases: u64 = std::env::var("BUILD_TOOLTIP_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x138_2380);
    let mut whole = Check::new("the popup description's timing");
    let (mut steps, mut populated, mut shown_steps, mut hides) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        steps += ((input.len() - 25) / 4) as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        for rec in c.chunks(4) {
            match rec[0] {
                1 if rec[1] == 1 => hides += 1,
                2 => populated += 1,
                100 if rec[3] == 0 => shown_steps += 1,
                _ => {}
            }
        }
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len())) / 4 * 4;
            let k0 = k.saturating_sub(8);
            println!("sequence {i}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 12).min(r.len())], &c[k0.min(c.len())..(k + 12).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps} steps: the popup filled {populated} times, shown after {shown_steps} steps, hidden {hides} times");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
