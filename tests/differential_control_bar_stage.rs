#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (enum ControlBarStages)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (the stages, setUpDownImages,
//     hidePurchaseScience)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarCallback.cpp (ShowControlBar,
//     HideControlBar, ToggleControlBar)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (the replay controls)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (MSG_META_TOGGLE_CONTROL_BAR)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::control_bar_stage` (GitHub #136) against the C++.
//!
//! # Method
//!
//! The C++ side is the stages, the up/down images, `hidePurchaseScience`, the three callbacks, the replay controls and
//! the meta case, cut verbatim by build.rs (`differential/wrap_control_bar_stage.cpp` lists the cuts, the stand-ins and
//! the words). Both sides run the same sequence on four windows (`ControlBarParent`, also `CP_MASTER`; `ButtonLarge`;
//! the replay window; the purchase-science window) and record every call that leaves `TheControlBar` (hide, position,
//! the view's height, the images, the special power shortcut, the animation, the popup's layout, the UI's dirty flag,
//! the shell's top layout), then the state after each step.
//!
//! Each case: a random start (the stage 0..3, as upstream's constructor leaves it unset; the default position; the
//! display's height among 600, 768, 844, 0, 1, 7 and random to 100,000; the recorder playing back or not; a replay
//! game; the shell active; the shell's top layout absent, shown or hidden; with or without an animate window manager;
//! `ControlBarParent` found by name or not; `ButtonLarge` and the replay window present or not; the windows' hidden bits
//! and positions), then 1 to 40 steps: the meta case, `toggleControlBarStage`, `switchControlBarStage(-2..5)`, the
//! three callbacks immediate or not (never the two `NULL` dereferences of `HideControlBar`), and changes of the world
//! (playback, replay game, the shell, the display's height, a window hidden or shown behind the bar's back).
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - 0 mismatches over 20,000 sequences: the only arithmetic is `getHeight() * 0.80f` (float) and
//!   `getHeight() - .1 * getHeight()` (double), which both sides do in the same precision.
//! - Every plant 9241-9255 caught. 9243 (85 %) and 9245 (`0.9 * h` for `h - .1 * h`) are the two that might hide: the
//!   first not (any height above 20 moves it), the second only where the two roundings differ — predicted caught, but
//!   in fewer sequences than any other.
//!
//! # Results (2026-11-11, `CONTROL_BAR_STAGE_CASES=20000`, the default)
//!
//! 20,000 sequences, 411,877 steps: **0 mismatches** on the first full run, as predicted. 103,021 meta cases ran, 108,413
//! steps ended in the low stage, `ControlBarParent` was hidden 75,809 times and the shell's top layout toggled 26,627
//! times.
//!
//! Planted bugs (same sequences), mismatching sequences of 20,000: 9241 11,203; 9242 7,838; 9243 9,572; 9244 14,659;
//! 9246 9,266; 9247 9,266; 9248 3,460; 9249 16,580; 9250 8,259; 9251 9,855; 9252 7,755; 9254 5,342; 9255 4,771.
//! **Two misses on the first plant run, both of the plants and not of the translation:**
//! - 9245 was first `0.9 * h` for `h - .1 * h`: 0 caught. The two truncate to the same integer for every height from
//!   0 to 200,000 (checked exhaustively afterwards, and so does the same sum in `float`), so it was an equivalent
//!   mutant; my prediction that it would be caught was wrong. Replaced by "the low stage moves the bar to x 0": 11,573.
//! - 9253 was first `!hidden || plant`: 0 caught, because that branch runs only when `hidden` is false, so the plant
//!   changed nothing. Rewritten as `!hidden && !plant` (never hides): 13,862.
//!
//! With the two rewritten, 15 of 15 plants are caught.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9241 | `toggleControlBarStage` always goes to the default stage |
//! | 9242 | `switchControlBarStage` ignores a replay's playback |
//! | 9243 | the default stage's view is 85 % of the display |
//! | 9244 | the default stage does not show the bar |
//! | 9245 | the low stage moves the bar to x 0, not the default x (replaced: see the results) |
//! | 9246 | the low stage leaves the up/down images |
//! | 9247 | the low stage gets the "down" images |
//! | 9248 | `toggleReplayControls` shows the replay window when it is shown |
//! | 9249 | `ShowControlBar` does not mark the UI dirty |
//! | 9250 | `HideControlBar` leaves the purchase-science screen |
//! | 9251 | `ToggleControlBar` does not switch to the default stage when it shows the bar |
//! | 9252 | `ToggleControlBar` animates even when immediate |
//! | 9253 | `ToggleControlBar` never hides the bar |
//! | 9254 | the meta case in the shell always hides the top layout |
//! | 9255 | the meta case toggles the bar during a replay's playback |

mod support;

use rts_lockstep_engine::client::control_bar_stage::{
    hide_control_bar, show_control_bar, toggle_control_bar, toggle_control_bar_meta, ControlBarStage, ControlBarStageHost, ControlBarStages, ToggleControlBarHost,
    ToggleImage, BUTTON_LARGE, CONTROL_BAR_PARENT,
};
use rts_lockstep_engine::client::selection_xlat::Disposition;
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

/// The C++ stand-ins' world.
struct Env {
    hidden: [bool; 4],
    pos: [(i32, i32); 4],
    display_height: u32,
    playback: bool,
    replay: bool,
    shell_active: bool,
    top: Option<bool>,
    animate: bool,
    parent_found: bool,
    button_large: bool,
    replay_window: bool,
    out: Vec<i32>,
}

impl Env {
    fn rec(&mut self, w: [i32; 6]) {
        self.out.extend(w);
    }
}

fn image_id(i: ToggleImage) -> i32 {
    match i {
        ToggleImage::UpIn => 1,
        ToggleImage::UpOn => 2,
        ToggleImage::UpPushed => 3,
        ToggleImage::DownIn => 4,
        ToggleImage::DownOn => 5,
        ToggleImage::DownPushed => 6,
    }
}

impl ControlBarStageHost for Env {
    fn win_from_name(&self, name: &[u8]) -> Option<WinId> {
        match name {
            CONTROL_BAR_PARENT => self.parent_found.then_some(0),
            BUTTON_LARGE => self.button_large.then_some(1),
            _ => None,
        }
    }
    fn win_is_hidden(&self, w: WinId) -> bool {
        self.hidden[w]
    }
    fn win_hide(&mut self, w: WinId, hide: bool) {
        self.rec([1, w as i32, i32::from(hide), 0, 0, 0]);
        self.hidden[w] = hide;
    }
    fn win_get_position(&self, w: WinId) -> (i32, i32) {
        self.pos[w]
    }
    fn win_set_position(&mut self, w: WinId, x: i32, y: i32) {
        self.rec([2, w as i32, x, y, 0, 0]);
        self.pos[w] = (x, y);
    }
    fn gadget_button_set_images(&mut self, w: WinId, enabled: ToggleImage, hilite: ToggleImage, hilite_selected: ToggleImage) {
        self.rec([4, w as i32, image_id(enabled), image_id(hilite), image_id(hilite_selected), 0]);
    }
    fn display_height(&self) -> u32 {
        self.display_height
    }
    fn set_tactical_view_height(&mut self, h: i32) {
        self.rec([3, h, 0, 0, 0, 0]);
    }
    fn recorder_playback(&self) -> bool {
        self.playback
    }
    fn is_in_replay_game(&self) -> bool {
        self.replay
    }
    fn replay_window(&self) -> Option<WinId> {
        self.replay_window.then_some(2)
    }
    fn show_special_power_shortcut(&mut self) {
        self.rec([5, 0, 0, 0, 0, 0]);
    }
    fn hide_special_power_shortcut(&mut self) {
        self.rec([6, 0, 0, 0, 0, 0]);
    }
    fn animate_special_power_shortcut(&mut self, on: bool) {
        self.rec([7, i32::from(on), 0, 0, 0, 0]);
    }
    fn repopulate_build_tooltip_layout(&mut self) {
        self.rec([8, 0, 0, 0, 0, 0]);
    }
    fn mark_ui_dirty(&mut self) {
        self.rec([9, 0, 0, 0, 0, 0]);
    }
    fn has_animate_window_manager(&self) -> bool {
        self.animate
    }
    fn animate_reset(&mut self) {
        self.rec([10, 0, 0, 0, 0, 0]);
    }
    fn animate_register_slide_bottom(&mut self, w: WinId) {
        self.rec([11, w as i32, 0, 0, 0, 0]);
    }
    fn animate_reverse(&mut self) {
        self.rec([12, 0, 0, 0, 0, 0]);
    }
}

/// The meta case's host: the shell and the bar over the world.
struct Meta {
    bar: ControlBarStage,
    env: Env,
}

impl ToggleControlBarHost for Meta {
    fn shell_is_active(&self) -> bool {
        self.env.shell_active
    }
    fn shell_top_is_hidden(&self) -> Option<bool> {
        self.env.top
    }
    fn shell_top_hide(&mut self, hide: bool) {
        self.env.rec([13, i32::from(hide), 0, 0, 0, 0]);
        self.env.top = Some(hide);
    }
    fn toggle_bar_recorder_playback(&self) -> bool {
        self.env.playback
    }
    fn toggle_control_bar(&mut self, immediate: bool) {
        toggle_control_bar(&mut self.bar, immediate, &mut self.env);
    }
}

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let env = Env {
        hidden: [0, 1, 2, 3].map(|i| (input[12] >> i) & 1 != 0),
        pos: [(input[13], input[14]), (input[15], 0), (0, 0), (0, 0)],
        display_height: input[3] as u32,
        playback: input[4] != 0,
        replay: input[5] != 0,
        shell_active: input[6] != 0,
        top: (input[7] != 0).then_some(input[7] == 2),
        animate: input[8] != 0,
        parent_found: input[9] != 0,
        button_large: input[10] != 0,
        replay_window: input[11] != 0,
        out: Vec::new(),
    };
    // `ControlBar::init`'s part is the setup's: the stage and the default position are given (upstream leaves the stage
    // unset until a scheme switches it)
    let bar = ControlBarStage {
        current: ControlBarStages::from_i32(input[0]).expect("a stage"),
        default_position: (input[1], input[2]),
        master: Some(0),
        purchase_science: Some(3),
    };
    let mut m = Meta { bar, env };
    for w in input[16..].chunks(4) {
        match w[0] {
            0 => {
                let d = toggle_control_bar_meta(&mut m);
                m.env.rec([20, i32::from(d == Disposition::Destroy), 0, 0, 0, 0]);
            }
            1 => m.bar.toggle_control_bar_stage(&mut m.env),
            2 => m.bar.switch_control_bar_stage(w[1], &mut m.env),
            3 => show_control_bar(&mut m.bar, w[1] != 0, &mut m.env),
            4 => hide_control_bar(&mut m.bar, w[1] != 0, &mut m.env),
            5 => toggle_control_bar(&mut m.bar, w[1] != 0, &mut m.env),
            _ => match w[1] {
                0 => m.env.playback = w[2] != 0,
                1 => m.env.replay = w[2] != 0,
                2 => m.env.shell_active = w[2] != 0,
                3 => {
                    if m.env.top.is_some() {
                        m.env.top = Some(w[2] != 0);
                    }
                }
                4 => m.env.display_height = w[2] as u32,
                _ => m.env.win_hide((w[2] & 3) as usize, w[3] != 0),
            },
        }
        let bits = (0..4).fold(0, |b, i| b | (i32::from(m.env.hidden[i]) << i));
        let top = i32::from(m.env.top.unwrap_or(false));
        let (x, y) = m.env.pos[0];
        m.env.rec([100, m.bar.current as i32, bits, x, y, top]);
    }
    m.env.out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 200_000];
    let n = unsafe { cpp::ref_control_bar_stage_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let height = |g: &mut Gen| [600, 768, 844, 0, 1, 7, g.below(100_001) as i32][g.below(7) as usize];
    let mut v = vec![
        g.below(4) as i32,
        g.below(801) as i32 - 100,
        g.below(801) as i32 - 100,
        height(g),
        i32::from(g.below(4) == 0),
        i32::from(g.below(3) == 0),
        i32::from(g.below(3) == 0),
        g.below(3) as i32,
        i32::from(g.below(4) != 0),
        i32::from(g.below(6) != 0),
        i32::from(g.below(5) != 0),
        i32::from(g.below(2) == 0),
        g.below(16) as i32,
        g.below(801) as i32,
        g.below(601) as i32,
        g.below(801) as i32,
    ];
    let (animate, parent) = (v[8] != 0, v[9] != 0);
    let steps = 1 + g.below(40);
    for _ in 0..steps {
        let s = match g.below(16) {
            0..=3 => [0, 0, 0, 0],
            4..=5 => [1, 0, 0, 0],
            6 => [2, g.below(8) as i32 - 2, 0, 0],
            7..=8 => [3, g.below(2) as i32, 0, 0],
            9..=10 => {
                // never `HideControlBar(TRUE)` without ControlBarParent, nor `HideControlBar(FALSE)` without an animate
                // window manager (NULL dereferences upstream)
                let mut imm = g.below(2) as i32;
                if !parent {
                    imm = 0;
                }
                if !animate {
                    imm = 1;
                }
                if !parent && !animate {
                    [1, 0, 0, 0]
                } else {
                    [4, imm, 0, 0]
                }
            }
            11..=12 => [5, g.below(2) as i32, 0, 0],
            _ => match g.below(6) {
                4 => [6, 4, height(g), 0],
                5 => [6, 5, g.below(4) as i32, g.below(2) as i32],
                k => [6, k as i32, g.below(2) as i32, 0],
            },
        };
        v.extend(s);
    }
    v
}

#[test]
fn the_control_bar_stages_match_upstream() {
    let cases: u64 = std::env::var("CONTROL_BAR_STAGE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x136_1360);
    let mut whole = Check::new("the control bar's stages and toggles");
    let (mut steps, mut metas, mut lows, mut toggled_hidden, mut shell_tops) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        steps += ((input.len() - 16) / 4) as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        for rec in c.chunks(6) {
            match rec[0] {
                20 => metas += 1,
                100 if rec[1] == ControlBarStages::Low as i32 => lows += 1,
                1 if rec[1] == 0 && rec[2] == 1 => toggled_hidden += 1,
                13 => shell_tops += 1,
                _ => {}
            }
        }
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len())) / 6 * 6;
            println!("sequence {i}: rust {:?} c++ {:?}", &r[k.min(r.len())..(k + 18).min(r.len())], &c[k.min(c.len())..(k + 18).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps} steps: {metas} meta cases, {lows} steps ending low, the bar hidden {toggled_hidden} times, the shell's top toggled {shell_tops} times");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
