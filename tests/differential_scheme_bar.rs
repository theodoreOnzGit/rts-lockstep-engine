#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp (ControlBarScheme::ControlBarScheme,
//     init's colour lines, findControlBarScheme, setControlBarSchemeByPlayerTemplate, setControlBarSchemeByPlayer)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (ControlBar::ControlBar's colours,
//     setControlBarSchemeByPlayer, setControlBarSchemeByPlayerTemplate, updateBuildUpClockColor,
//     updateCommanBarBorderColors, setCommandBarBorder)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (getBorderColor, updateBorderColor)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Color.h (GAME_COLOR_UNDEFINED, GameMakeColor)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! GitHub #145: the control bar's scheme setters and the colours a scheme gives the bar, against the C++.
//!
//! # Method
//!
//! The C++ side is cut verbatim by build.rs (`differential/wrap_scheme_bar.cpp` lists the cuts and the stand-ins) and
//! runs the whole chain: `ControlBar::setControlBarSchemeByPlayer(p)` or `setControlBarSchemeByPlayerTemplate(pt)` (or
//! the manager's `setControlBarSchemeByPlayerTemplate(pt, useSmall)` alone), through the manager's communicator switch
//! and scheme choice, the chosen scheme's `init` (its colour lines on `TheControlBar`), back to the bar's buttons, context,
//! observer flag and stage; the bar and every scheme are built by their real constructors. The Rust side runs
//! `client::control_bar_scheme::{control_bar_set_control_bar_scheme_by_player,
//! control_bar_set_control_bar_scheme_by_player_template}` with a host whose manager call is
//! `set_control_bar_scheme_by_player_communicator` + `ControlBarSchemeManager::set_control_bar_scheme_by_player` (or
//! `set_control_bar_scheme_by_player_template`) + `ControlBarScheme::init_colors` on a `ControlBarColors::new()`. After the
//! call, both give the current scheme and the multiplier's bits, the bar's six colours, and `setCommandBarBorder` for
//! border types 0..5 (the Rust `client::build_tooltip::set_command_bar_border` through `ControlBarColors::border_colors`).
//!
//! Each case: the kind (the bar's player version 40 %, the bar's template version 40 %, the manager's template version
//! with `useSmall` 20 %), multiplayer or not, the communicator window and its two buttons each present or not, a display,
//! a side from 15 strings mixing case and "Small" (`America`, `america`, `AmericaSmall`, `Americasmall`, `Observer`,
//! `Default`, the empty string, ...), a template or none, a current scheme or none, the bar with or without a scheme
//! manager, the player active or not, the game mode (0..6), `TheGameInfo->isMultiPlayer()`, the beacon, idle worker and
//! general's windows each present or not, the store with or without `FactionObserver`, the template the observer's or
//! not, and 0 to 8 schemes, each with its six colours each set (to a random word, or `GAME_COLOR_UNDEFINED`) or left at
//! the constructor's.
//!
//! # Predictions (written before the first run, 2026-11-17)
//!
//! - 0 mismatches over 20,000 cases: assignments, string comparisons and one `float` division each way, the same chain.
//! - Plants 10401-10415 all caught. The fewest: 10413 ("small" for "Small"), which needs the manager-only kind (20 %)
//!   with `useSmall`, a scheme whose side is `Americasmall`/`AmericaSmall`/`ChinaSmall` matching the template's side +
//!   "Small" without case, and **no** current scheme of the exact side: a few hundred. 10411 (a `NULL` template going
//!   on) and 10414 (useSmall ignored) about the same order. 10407 (the observer flag after the switch) only in observer
//!   cases: thousands.
//!
//! # Results (2026-11-17, `SCHEME_BAR_CASES=20000`, the default)
//!
//! 20,000 cases (kinds 7,928 / 8,076 / 3,996; 9,034 scheme inits; the observer's bar 4,647 times;
//! `NonCommand_BriefingHistory` given 2,221 times; `TheGameInfo` asked 2,584 times): **0 mismatches** on the first run, as
//! predicted. Plants, mismatching cases of 20,000: 10401 5,330; 10402 8,178; 10403 4,523; 10404 8,491; 10405 10,966;
//! 10406 5,330; 10407 4,647; 10408 9,040; 10409 9,098; 10410 8,076; 10411 1,289; 10412 329; 10413 8; 10414 682;
//! 10415 2,584. **15 of 15 caught.** **Prediction missed:** 10413 was caught by 8 cases, not a few hundred: the scheme
//! search compares sides without case, so "small" and "Small" find the same schemes; only the current scheme's exact
//! comparison tells them apart, which needs a current scheme whose side is exactly the template's side + "Small".
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 10401 | `updateCommanBarBorderColors` swaps action and upgrade |
//! | 10402 | `init` gives `updateBorderColor` the clock's colour |
//! | 10403 | a scheme's default `CommandBarBorderColor` is `GameMakeColor(0,0,0,100)` |
//! | 10404 | a scheme's default border colours are 0 |
//! | 10405 | `ControlBar::ControlBar`'s command bar border is 0 |
//! | 10406 | `border_colors` gives action and upgrade swapped to `setCommandBarBorder` |
//! | 10407 | the observer flag set after `switchToContext` |
//! | 10408 | the beacon hidden only outside a network game **and** without `isMultiPlayer` |
//! | 10409 | the general's button not shown for an active player |
//! | 10410 | the template version without `hidePurchaseScience` |
//! | 10411 | the manager's template version goes on with a `NULL` template |
//! | 10412 | a `NULL` template never the observer's |
//! | 10413 | "small" appended for `useSmall` |
//! | 10414 | `useSmall` ignored |
//! | 10415 | the beacon's `isMultiPlayer` not asked |

mod support;

use rts_lockstep_engine::client::build_tooltip::{set_command_bar_border, ControlBarInitHost};
use rts_lockstep_engine::client::control_bar_scheme::{
    control_bar_set_control_bar_scheme_by_player, control_bar_set_control_bar_scheme_by_player_template, set_control_bar_scheme_by_player_communicator, ControlBarColors, ControlBarScheme,
    ControlBarSchemeManager, SchemeByPlayerBarHost,
};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::common::base_type::ICoord2D;
use rts_lockstep_engine::differential as cpp;
use std::cell::RefCell;
use support::{Check, Gen};

/// The wrapper's strings, in its order.
const STRINGS: [&[u8]; 15] = [b"", b"America", b"america", b"AMERICA", b"China", b"GLA", b"Observer", b"observer", b"AmericaSmall", b"Americasmall", b"Default", b"default", b"America8x6", b"ChinaSmall", b"Civilian"];
/// The colour fields, in the wrapper's mask order.
const COLOR_FIELDS: [&str; 6] = ["BuildUpClockColor", "ButtonBorderBuildColor", "CommandBarBorderColor", "ButtonBorderActionColor", "ButtonBorderUpgradeColor", "ButtonBorderSystemColor"];
/// The window ids the host gives: beacon, idle worker, general, communicator.
const WINDOWS: [&[u8]; 4] = [b"ControlBar.wnd:ButtonPlaceBeacon", b"ControlBar.wnd:ButtonIdleWorker", b"ControlBar.wnd:ButtonGeneral", b"ControlBar.wnd:PopupCommunicator"];

struct Env<'a> {
    input: &'a [i32],
    m: ControlBarSchemeManager,
    colors: ControlBarColors,
    out: RefCell<Vec<i32>>,
    /// The border type `setCommandBarBorder` is asked for (`None`: a `setControlCommand`'s, not recorded).
    border_type: Option<i32>,
}

impl Env<'_> {
    fn rec(&self, w: [i32; 4]) {
        self.out.borrow_mut().extend(w);
    }
    fn side(&self) -> &'static [u8] {
        STRINGS[self.input[8] as usize % 15]
    }
}

impl ControlBarInitHost for Env<'_> {
    fn win_get_window_from_id(&self, name: &[u8]) -> Option<WinId> {
        let i = WINDOWS.iter().position(|n| *n == name)?;
        let present = if i == 3 { self.input[2] } else { self.input[14 + i] };
        (present != 0).then_some(i)
    }
    fn find_command_button(&self, name: &[u8]) -> Option<usize> {
        let id = if name == b"NonCommand_Communicator" { 0 } else if name == b"NonCommand_BriefingHistory" { 1 } else { 99 };
        self.rec([10, id, 0, 0]);
        (id < 2 && self.input[3 + id as usize] != 0).then_some(id as usize)
    }
    fn win_set_tooltip_func(&mut self, _w: WinId) {}
    fn is_push_button_input(&self, _w: WinId) -> bool {
        true
    }
    fn command_options(&self, _c: usize) -> u32 {
        0
    }
    fn command_button_image(&self, _c: usize) -> Option<Vec<u8>> {
        None
    }
    fn command_text_label(&self, _c: usize) -> Vec<u8> {
        Vec::new()
    }
    fn command_has_sciences(&self, _c: usize) -> bool {
        false
    }
    fn command_border(&self, _c: usize) -> i32 {
        0
    }
    fn gadget_button_enable_check_like(&mut self, _w: WinId, _check_like: bool, _checked: bool) {}
    fn gadget_button_set_enabled_image(&mut self, _w: WinId, _image: Vec<u8>) {}
    fn gadget_button_clear_text(&mut self, _w: WinId) {}
    fn gadget_button_set_data(&mut self, _w: WinId, c: usize) {
        self.rec([11, c as i32, 0, 0]);
    }
    fn gadget_button_set_border(&mut self, _w: WinId, color: i32, draw: bool) {
        if let Some(t) = self.border_type {
            self.rec([6, t, color, i32::from(draw)]);
        }
    }
    fn border_colors(&self) -> [i32; 4] {
        self.colors.border_colors()
    }
    fn search_hot_key(&mut self, _label: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn add_hot_key(&mut self, _w: WinId, _key: Vec<u8>) {}
    fn gadget_button_set_alt_sound(&mut self, _w: WinId, _sound: &[u8]) {}
}

impl SchemeByPlayerBarHost for Env<'_> {
    fn scheme_manager_by_player(&mut self) {
        if self.input[10] == 0 {
            return;
        }
        set_control_bar_scheme_by_player_communicator(self.input[1] != 0, self);
        let display = (self.input[5], self.input[6]);
        if let Some(i) = self.m.set_control_bar_scheme_by_player(Some(self.side()), display) {
            self.rec([30, i as i32, 0, 0]);
            self.m.schemes[i].init_colors(&mut self.colors);
        }
    }
    fn scheme_manager_by_player_template(&mut self) {
        if self.input[10] == 0 {
            return;
        }
        let display = (self.input[5], self.input[6]);
        let side = (self.input[7] != 0).then(|| self.side());
        if let Some(i) = self.m.set_control_bar_scheme_by_player_template(side, false, display) {
            self.rec([30, i as i32, 0, 0]);
            self.m.schemes[i].init_colors(&mut self.colors);
        }
    }
    fn win_get_window_from_id(&self, name: &[u8]) -> Option<WinId> {
        ControlBarInitHost::win_get_window_from_id(self, name)
    }
    fn win_hide(&mut self, w: WinId, hide: bool) {
        self.rec([42, w as i32, i32::from(hide), 0]);
    }
    fn win_enable(&mut self, w: WinId, enable: bool) {
        self.rec([43, w as i32, i32::from(enable), 0]);
    }
    fn switch_to_context(&mut self, context: i32) {
        self.rec([40, context, 0, 0]);
    }
    fn set_observer_command_bar(&mut self, observer: bool) {
        self.rec([41, i32::from(observer), 0, 0]);
    }
    fn game_mode(&self) -> i32 {
        self.input[12]
    }
    fn game_info_is_multi_player(&self) -> bool {
        self.rec([44, 0, 0, 0]);
        self.input[13] != 0
    }
    fn switch_control_bar_stage_default(&mut self) {
        self.rec([45, 0, 0, 0]);
    }
    fn hide_purchase_science(&mut self) {
        self.rec([46, 0, 0, 0]);
    }
}

fn rust_case(input: &[i32]) -> Vec<i32> {
    let n = input[21] as usize;
    let mut m = ControlBarSchemeManager::new();
    for i in 0..n {
        let w = &input[22 + i * 11..33 + i * 11];
        let mut s = ControlBarScheme { name: STRINGS[w[0] as usize % 15].to_ascii_lowercase(), side: STRINGS[w[1] as usize % 15].to_vec(), screen_creation_res: ICoord2D { x: w[2], y: w[3] }, ..Default::default() };
        for (k, f) in COLOR_FIELDS.iter().enumerate() {
            if w[4] & (1 << k) != 0 {
                s.colors.insert(*f, w[5 + k]);
            }
        }
        m.schemes.push(s);
    }
    m.current = (input[9] >= 0 && (input[9] as usize) < n).then_some(input[9] as usize);
    let mut env = Env { input, m, colors: ControlBarColors::new(), out: RefCell::new(Vec::new()), border_type: None };
    match input[0] {
        0 => control_bar_set_control_bar_scheme_by_player(input[11] != 0, &mut env),
        1 => {
            // pt: the side's template (0) or FactionObserver's (1); the store's FactionObserver
            let template = (input[7] != 0).then_some(usize::from(input[18] != 0));
            let observer = (input[17] != 0).then_some(1);
            control_bar_set_control_bar_scheme_by_player_template(template, observer, &mut env);
        }
        _ => {
            let side = (input[7] != 0).then(|| env.side());
            if let Some(i) = env.m.set_control_bar_scheme_by_player_template(side, input[19] != 0, (input[5], input[6])) {
                env.rec([30, i as i32, 0, 0]);
                env.m.schemes[i].init_colors(&mut env.colors);
            }
        }
    }
    let c = env.colors;
    env.rec([20, env.m.current.map_or(-1, |c| c as i32), env.m.multiplier.0.to_bits() as i32, env.m.multiplier.1.to_bits() as i32]);
    env.rec([21, c.build_up_clock, c.get_border_color(), 0]);
    env.rec([22, c.border_build, c.border_action, c.border_upgrade]);
    env.rec([23, c.border_system, 0, 0]);
    for t in 0..6 {
        env.border_type = Some(t);
        set_command_bar_border(0, t, &mut env);
    }
    env.out.into_inner()
}

fn cpp_case(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 2000];
    let n = unsafe { cpp::ref_scheme_bar_case(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn color(g: &mut Gen) -> i32 {
    match g.below(4) {
        0 => 0x00FF_FFFF,
        1 => 0,
        _ => g.below(u32::MAX) as i32,
    }
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let kind = match g.below(5) {
        0 | 1 => 0,
        2 | 3 => 1,
        _ => 2,
    };
    let (w, h) = match g.below(5) {
        0 => (800, 600),
        1 => (1024, 768),
        2 => (390, 844),
        3 => (844, 390),
        _ => (1 + g.below(4000) as i32, 1 + g.below(4000) as i32),
    };
    let n = g.below(9) as i32;
    let current = if g.below(3) == 0 { -1 } else { g.below(n as u32 + 1) as i32 };
    // (the bar's player version always has a player: upstream dereferences it)
    let present = kind == 0 || g.below(6) != 0;
    let mut v = vec![
        kind,
        i32::from(g.below(2) == 0),
        i32::from(g.below(6) != 0),
        i32::from(g.below(5) != 0),
        i32::from(g.below(5) != 0),
        w,
        h,
        i32::from(present),
        g.below(15) as i32,
        current,
        i32::from(g.below(8) != 0),
        i32::from(g.below(3) != 0),
        g.below(7) as i32,
        i32::from(g.below(2) == 0),
        i32::from(g.below(5) != 0),
        i32::from(g.below(5) != 0),
        i32::from(g.below(5) != 0),
        i32::from(g.below(4) != 0),
        i32::from(g.below(3) == 0),
        i32::from(g.below(2) == 0),
        0,
        n,
    ];
    for _ in 0..n {
        let res = match g.below(4) {
            0 | 1 => (800, 600),
            2 => (1600, 1200),
            _ => (1 + g.below(3000) as i32, 1 + g.below(3000) as i32),
        };
        v.extend([g.below(15) as i32, g.below(15) as i32, res.0, res.1, g.below(64) as i32]);
        for _ in 0..6 {
            let c = color(g);
            v.push(c);
        }
    }
    v
}

#[test]
fn the_scheme_bar_is_upstreams() {
    let cases: u64 = std::env::var("SCHEME_BAR_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x145_2911);
    let mut whole = Check::new("scheme setters and colours");
    let (mut inits, mut observer, mut briefing, mut asked) = (0u64, 0u64, 0u64, 0u64);
    let mut kinds = [0u64; 3];
    for i in 0..cases {
        let input = gen_case(&mut g);
        kinds[input[0] as usize] += 1;
        let r = rust_case(&input);
        let c = cpp_case(&input);
        for w in c.chunks(4) {
            match (w[0], w[1]) {
                (30, _) => inits += 1,
                (41, 1) => observer += 1,
                (11, 1) => briefing += 1,
                (44, _) => asked += 1,
                _ => {}
            }
        }
        if r != c && !planted {
            println!("case {i} {input:?}:\n rust {r:?}\n c++  {c:?}");
        }
        whole.eq(r, c, || format!("case {i}"));
    }
    println!("{cases} cases (kinds {kinds:?}): {inits} scheme inits; the observer's bar {observer}; NonCommand_BriefingHistory given {briefing}; TheGameInfo asked {asked}");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
