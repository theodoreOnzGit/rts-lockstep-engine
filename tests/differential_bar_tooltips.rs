#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (ControlBar::init's tooltip callbacks,
//     lines 1190-1240; setCommandBarBorder; setControlCommand)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ControlBarPopupDescription.cpp
//     (populateBuildTooltipLayout's head and its money, power and general's experience branches)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::build_tooltip::control_bar_init_tooltips` and `client::control_bar_popup::{tooltip_window_of,
//! populate_window_tooltip}` (GitHub #144) against the C++.
//!
//! # Method
//!
//! The C++ side is ControlBar.cpp's `ControlBar::init` tooltip part, `setCommandBarBorder` and `setControlCommand`, and
//! ControlBarPopupDescription.cpp's `populateBuildTooltipLayout` head and `else if(tooltipWin)` branch, cut verbatim by
//! build.rs (`differential/wrap_bar_tooltips.cpp` lists the cuts, the stand-ins and the words). Both sides make the
//! init's calls and record each gadget call, `winSetTooltipFunc` and `addHotKey` in order; then both fill the popup for
//! a list of windows and record the name, the cost and the description (as code points), or nothing when the branch
//! returns.
//!
//! Each case: the ten windows each present (the communicator always: upstream reads it unchecked) or not, with
//! `GadgetPushButtonInput` (3 in 4) or another input function; the six `NonCommand_*` buttons each found (4 in 5) or
//! `NULL`, with random options (`CHECK_LIKE` half the time), an image or none, a text label or none, sciences or none,
//! a border type 0-5 (5 is out of the enum: the switch's `default`); random border colours (or `GAME_COLOR_UNDEFINED`);
//! `TheHotKeyManager` present or not; the observer's bar on or off; the local and the observer's players each present or
//! not, with random production and consumption (negative ones too); one of five power formats (`%d/%d`, one `%d`,
//! none, `%%` with two, a line break); then 1 to 12 popups over random windows (the three, the buttons, the other one,
//! `NULL`).
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - 0 mismatches over 20,000 cases. The likeliest miss: the power format's `%%` and its extra argument (`%d` with two
//!   arguments): `format_wide` drops an unused argument as `vswprintf` does, so none expected.
//! - Every plant 9341-9358 caught. The fewest: 9342 (`ButtonLarge` given the options' command: only when the two
//!   commands differ in what they record, which is most cases, but needs `ButtonLarge` present), 9357 (no player at all:
//!   one case in 4 has no local player, and the observer's bar must be off or its player missing, and a popup over the
//!   power window must be asked) and 9356 (the observer's bar on: half the cases, again only over the power window).
//!
//! # Results (2026-11-11, `BAR_TOOLTIPS_CASES=20000`, the default)
//!
//! 20,000 cases: 59,953 commands set on buttons, 194,825 tooltip callbacks set, 92,411 popups filled, 37,688 returned
//! unfilled (an unknown window). **0 mismatches** on the first run, as predicted (`%%` and the unused argument agreed).
//!
//! Planted bugs (same cases), mismatching cases of 20,000: 9341 20,000; 9342 11,498; 9343 16,009; 9344 16,011; 9345
//! 13,308; 9346 11,039; 9347 16,516; 9348 5,927; 9349 19,700; 9350 11,716; 9351 13,312; 9352 11,295; 9353 19,700; 9354
//! 5,766; 9355 15,156; 9356 3,724; 9357 2,483; 9358 10,871. **18 of 18 caught.** **Prediction missed:** 9342 was not
//! among the fewest (11,498: `ButtonLarge` is present in 4 cases in 5 and the two commands' records differ almost always).
//! 9357 (2,483) and 9356 (3,724) were the fewest, as predicted; next were 9354 (5,766: swapping production and
//! consumption changes nothing when they are equal, often both 0, or the format has no `%d` pair) and 9348 (5,927: a
//! label alone also gives the tooltip, so the sciences only matter for a command with no label).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9341 | the communicator does not get `commandButtonTooltip` |
//! | 9342 | `ButtonLarge` is given `NonCommand_Options` |
//! | 9343 | the power window does not get `commandButtonTooltip` |
//! | 9344 | the general's experience window does not get it |
//! | 9345 | `setControlCommand` does not check for a push button |
//! | 9346 | `setControlCommand` with no command still gives the tooltip |
//! | 9347 | `CHECK_LIKE` is ignored |
//! | 9348 | the sciences do not give the tooltip |
//! | 9349 | the command is not set as the button's data |
//! | 9350 | the build and upgrade border colours are swapped |
//! | 9351 | `COMMAND_BUTTON_BORDER_NONE` draws the border |
//! | 9352 | an empty hot key is added |
//! | 9353 | no alternate sound |
//! | 9354 | the power's production and consumption are swapped |
//! | 9355 | an unknown window gets the money's popup instead of none |
//! | 9356 | the observer's bar is ignored (the local player is read) |
//! | 9357 | no player: the power's format is left unformatted |
//! | 9358 | the general's experience window gets the money's popup |

mod support;

use rts_lockstep_engine::client::build_tooltip::{control_bar_init_tooltips, ControlBarInitHost};
use rts_lockstep_engine::client::control_bar_popup::{populate_window_tooltip, tooltip_window_of, PopupWindowHost};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const NAMES: [&[u8]; 10] = [
    b"ControlBar.wnd:PopupCommunicator",
    b"ControlBar.wnd:ButtonOptions",
    b"ControlBar.wnd:ButtonIdleWorker",
    b"ControlBar.wnd:ButtonPlaceBeacon",
    b"ControlBar.wnd:ButtonGeneral",
    b"ControlBar.wnd:ButtonLarge",
    b"ControlBar.wnd:PowerWindow",
    b"ControlBar.wnd:MoneyDisplay",
    b"ControlBar.wnd:GeneralsExp",
    b"ControlBar.wnd:Other",
];
const COMMANDS: [&[u8]; 6] = [b"NonCommand_Communicator", b"NonCommand_Options", b"NonCommand_IdleWorker", b"NonCommand_Beacon", b"NonCommand_GeneralsExperience", b"NonCommand_UpDown"];
const POWER_FORMATS: [&str; 5] = ["Power %d/%d", "%d", "no numbers", "100%% %d used of %d", "%d\n%d"];

#[derive(Default)]
struct Command {
    present: bool,
    options: u32,
    image: Option<i32>,
    label: i32,
    sciences: bool,
    border: i32,
}

/// The C++ stand-ins' world.
#[derive(Default)]
struct Env {
    present: [bool; 10],
    push: [bool; 10],
    commands: Vec<Command>,
    colors: [i32; 4],
    hot_keys: bool,
    observer_on: bool,
    local: Option<(i32, i32)>,
    observer: Option<(i32, i32)>,
    power_format: usize,
    out: Vec<i32>,
}

impl Env {
    fn rec(&mut self, w: [i32; 4]) {
        self.out.extend(w);
    }
}

impl ControlBarInitHost for Env {
    fn win_get_window_from_id(&self, name: &[u8]) -> Option<WinId> {
        NAMES.iter().position(|n| *n == name).filter(|&i| self.present[i])
    }
    fn find_command_button(&self, name: &[u8]) -> Option<usize> {
        COMMANDS.iter().position(|n| *n == name).filter(|&i| self.commands[i].present)
    }
    fn win_set_tooltip_func(&mut self, w: WinId) {
        self.rec([1, w as i32, 0, 0]);
    }
    fn is_push_button_input(&self, w: WinId) -> bool {
        self.push[w]
    }
    fn command_options(&self, c: usize) -> u32 {
        self.commands[c].options
    }
    fn command_button_image(&self, c: usize) -> Option<Vec<u8>> {
        self.commands[c].image.map(|i| (i & 63).to_string().into_bytes())
    }
    fn command_text_label(&self, c: usize) -> Vec<u8> {
        let l = self.commands[c].label;
        if l < 0 { Vec::new() } else { l.to_string().into_bytes() }
    }
    fn command_has_sciences(&self, c: usize) -> bool {
        self.commands[c].sciences
    }
    fn command_border(&self, c: usize) -> i32 {
        self.commands[c].border
    }
    fn gadget_button_enable_check_like(&mut self, w: WinId, check_like: bool, checked: bool) {
        self.rec([2, w as i32, i32::from(check_like), i32::from(checked)]);
    }
    fn gadget_button_set_enabled_image(&mut self, w: WinId, image: Vec<u8>) {
        let i: i32 = String::from_utf8(image).expect("text").parse().expect("a number");
        self.rec([3, w as i32, i, 0]);
    }
    fn gadget_button_clear_text(&mut self, w: WinId) {
        self.rec([4, w as i32, 0, 0]);
    }
    fn gadget_button_set_data(&mut self, w: WinId, c: usize) {
        self.rec([5, w as i32, c as i32, 0]);
    }
    fn gadget_button_set_border(&mut self, w: WinId, color: i32, draw: bool) {
        self.rec([6, w as i32, color, i32::from(draw)]);
    }
    fn border_colors(&self) -> [i32; 4] {
        self.colors
    }
    fn search_hot_key(&mut self, label: &[u8]) -> Option<Vec<u8>> {
        if !self.hot_keys {
            return None;
        }
        // the stand-in's `searchHotKey`: a label's key, none for every third label
        let l: i32 = if label.is_empty() { -1 } else { String::from_utf8(label.to_vec()).expect("text").parse().expect("a number") };
        Some(if l >= 0 && l % 3 != 0 { (1000 + l).to_string().into_bytes() } else { Vec::new() })
    }
    fn add_hot_key(&mut self, w: WinId, key: Vec<u8>) {
        let k: i32 = if key.is_empty() { -1 } else { String::from_utf8(key).expect("text").parse().expect("a number") };
        self.rec([7, w as i32, k, 0]);
    }
    fn gadget_button_set_alt_sound(&mut self, w: WinId, sound: &[u8]) {
        self.rec([8, w as i32, i32::from(sound == b"GUICommandBarClick"), 0]);
    }
}

impl PopupWindowHost for Env {
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        if label == b"CONTROLBAR:PowerDescription" {
            return POWER_FORMATS[self.power_format].encode_utf16().collect();
        }
        label.iter().map(|&b| u16::from(b)).collect()
    }
    fn is_observer_control_bar_on(&self) -> bool {
        self.observer_on
    }
    fn observer_look_at_player_energy(&self) -> Option<(i32, i32)> {
        self.observer
    }
    fn local_energy(&self) -> Option<(i32, i32)> {
        self.local
    }
}

const HEAD: usize = 20 + 36 + 4 + 9;

fn rust_case(input: &[i32]) -> Vec<i32> {
    let mut env = Env::default();
    for i in 0..10 {
        env.present[i] = input[i * 2] != 0;
        env.push[i] = input[i * 2 + 1] != 0;
    }
    for i in 0..6 {
        let p = &input[20 + i * 6..];
        env.commands.push(Command { present: p[0] != 0, options: p[1] as u32, image: (p[2] >= 0).then_some(p[2]), label: p[3], sciences: p[4] != 0, border: p[5] });
    }
    let p = &input[56..];
    env.colors = [p[0], p[1], p[2], p[3]];
    let p = &input[60..];
    env.hot_keys = p[0] != 0;
    env.observer_on = p[1] != 0;
    env.local = (p[2] != 0).then_some((p[3], p[4]));
    env.observer = (p[5] != 0).then_some((p[6], p[7]));
    env.power_format = (p[8] % 5) as usize;
    control_bar_init_tooltips(&mut env);
    for (k, &w) in input[HEAD..].iter().enumerate() {
        let win = (0..10).contains(&w).then_some(w as usize);
        // `if(commandButton) { } else if(tooltipWin) { ... }`: with no window, the empty popup
        let filled = match win {
            None => Some((Vec::new(), Vec::new())),
            Some(win) => tooltip_window_of(win, |n| env.win_get_window_from_id(n)).map(|t| {
                let t = populate_window_tooltip(t, &env);
                (t.name, t.description)
            }),
        };
        if let Some((name, description)) = filled {
            env.out.extend([21, 0, name.len() as i32, description.len() as i32]);
            env.out.extend(name.iter().map(|&c| i32::from(c)));
            env.out.extend(description.iter().map(|&c| i32::from(c)));
        }
        env.out.extend([22, k as i32, 0, 0]);
    }
    env.out
}

fn cpp_case(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 100_000];
    let n = unsafe { cpp::ref_bar_tooltips_case(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn energy(g: &mut Gen) -> i32 {
    match g.below(4) {
        0 => 0,
        1 => g.below(1000) as i32,
        2 => -(g.below(1000) as i32),
        _ => g.i32(),
    }
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let mut v = Vec::new();
    for i in 0..10 {
        let present = i == 0 || g.below(5) != 0;
        v.extend([i32::from(present), i32::from(g.below(4) != 0)]);
    }
    for _ in 0..6 {
        let other = if g.below(2) == 0 { 0 } else { g.u32() & !0x400 };
        let options = other | if g.below(2) == 0 { 0x400 } else { 0 };
        let image = if g.below(3) == 0 { -1 } else { g.below(64) as i32 };
        let label = if g.below(3) == 0 { -1 } else { g.below(30) as i32 };
        v.extend([i32::from(g.below(5) != 0), options as i32, image, label, i32::from(g.below(3) == 0), g.below(6) as i32]);
    }
    for _ in 0..4 {
        v.push(if g.below(3) == 0 { 0x00FF_FFFF } else { g.i32() });
    }
    v.extend([i32::from(g.below(3) != 0), i32::from(g.below(2) == 0), i32::from(g.below(4) != 0), energy(g), energy(g), i32::from(g.below(3) != 0), energy(g), energy(g), g.below(5) as i32]);
    let calls = 1 + g.below(12);
    for _ in 0..calls {
        v.push(match g.below(4) {
            0..=1 => 6 + g.below(3) as i32,
            2 => g.below(10) as i32,
            _ => -1,
        });
    }
    v
}

#[test]
fn the_bars_tooltip_windows_are_set_up_and_filled_as_upstream() {
    let cases: u64 = std::env::var("BAR_TOOLTIPS_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x144_1194);
    let mut whole = Check::new("ControlBar::init's tooltips and the window popups");
    let (mut set_commands, mut tooltips, mut filled, mut returned) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        let r = rust_case(&input);
        let c = cpp_case(&input);
        let mut k = 0;
        let mut last_filled = false;
        while k + 4 <= c.len() {
            match c[k] {
                1 => tooltips += 1,
                5 => set_commands += 1,
                21 => {
                    filled += 1;
                    last_filled = true;
                    k += (c[k + 2] + c[k + 3]) as usize;
                }
                22 => {
                    if !last_filled {
                        returned += 1;
                    }
                    last_filled = false;
                }
                _ => {}
            }
            k += 4;
        }
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(8);
            println!("case {i}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 12).min(r.len())], &c[k0.min(c.len())..(k + 12).min(c.len())]);
        }
        whole.eq(r, c, || format!("case {i}"));
    }
    println!("{cases} cases: {set_commands} commands set on buttons, {tooltips} tooltip callbacks set, {filled} popups filled, {returned} returned unfilled");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
