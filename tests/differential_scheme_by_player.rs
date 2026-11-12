#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarScheme.cpp (findControlBarScheme,
//     setControlBarSchemeByPlayer)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ControlBarSchemeManager::setControlBarSchemeByPlayer` (GitHub #146) against the C++: the communicator switch
//! (`client::control_bar_scheme::set_control_bar_scheme_by_player_communicator`) and the scheme of the player's side
//! (`ControlBarSchemeManager::set_control_bar_scheme_by_player`).
//!
//! # Method
//!
//! The C++ side is `setControlBarSchemeByPlayer` and `findControlBarScheme`, cut verbatim by build.rs, with stand-ins for
//! the scheme (its name, side and creation resolution; `init` records its index), the player, `TheRecorder`, the window
//! manager, `TheControlBar` and `TheDisplay` (`differential/wrap_scheme_by_player.cpp` lists them and the words). Each
//! case: multiplayer or not, the layout with or without `PopupCommunicator`, each `NonCommand_*` button found or not, a
//! display (800x600, 1024x768, 390x844, 844x390 or random), a player or none and its side, a current scheme or none, and
//! 0 to 8 schemes whose names and sides come from a list mixing case (`America`, `america`, `AMERICA`, `Observer`,
//! `observer`, `Default`, `default`, the empty string, ...) and whose creation resolutions are 800x600, 1600x1200 or
//! random (equal widths on purpose). The words (the buttons looked up and given, the schemes `init` ran on, the current
//! scheme and the multiplier's bits) are compared whole.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches over 20,000 cases: string comparisons and one `float` division each way.
//! - Plants 9378-9384 all caught. The fewest: 9382 (the widest of two equal widths taken last), which needs two schemes of
//!   the player's side with the same width: with 8 schemes from 15 strings and widths mostly 800 or 1600, a few percent.
//!
//! # Results (2026-11-12, `SCHEME_BY_PLAYER_CASES=20000`, the default)
//!
//! 20,000 cases (the communicator given `NonCommand_Communicator` 6,792 times and `NonCommand_BriefingHistory` 6,558
//! times; 9,338 scheme inits; 9,175 cases ending with no scheme): **0 mismatches** on the first run, as predicted.
//! Plants, mismatching cases of 20,000: 9378 16,620; 9379 2,370; 9380 341; 9381 534; 9382 408; 9383 5,229; 9384 3,563.
//! **7 of 7 caught.** **Prediction missed:** 9382 was not the fewest; 9380 (the current scheme's side without case) was,
//! since it needs a current scheme whose side differs from the player's only in case.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 9378 | the communicator's two buttons swapped |
//! | 9379 | no player goes on to the scheme (as an empty side) |
//! | 9380 | the current scheme's side compared without case |
//! | 9381 | an empty side is not `Observer` |
//! | 9382 | of equal widths, the last scheme is taken |
//! | 9383 | the multiplier's y divides the display's width |
//! | 9384 | no `Default` scheme when the side has none |

mod support;

use rts_lockstep_engine::client::build_tooltip::ControlBarInitHost;
use rts_lockstep_engine::client::control_bar_scheme::{set_control_bar_scheme_by_player_communicator, ControlBarScheme, ControlBarSchemeManager};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::common::base_type::ICoord2D;
use rts_lockstep_engine::differential as cpp;
use std::cell::RefCell;
use support::{Check, Gen};

/// The wrapper's strings, in its order.
const STRINGS: [&[u8]; 15] = [b"", b"America", b"america", b"AMERICA", b"China", b"GLA", b"Observer", b"observer", b"AmericaSmall", b"Default", b"default", b"America8x6", b"America16x12", b"ChinaSuper", b"Civilian"];

/// The bar's windows and buttons as the case has them; the calls recorded.
struct Env {
    has_window: bool,
    found: [bool; 2],
    out: RefCell<Vec<i32>>,
}

impl ControlBarInitHost for Env {
    fn win_get_window_from_id(&self, name: &[u8]) -> Option<WinId> {
        (self.has_window && name == b"ControlBar.wnd:PopupCommunicator").then_some(7)
    }
    fn find_command_button(&self, name: &[u8]) -> Option<usize> {
        let id = if name == b"NonCommand_Communicator" { 0 } else if name == b"NonCommand_BriefingHistory" { 1 } else { 99 };
        self.out.borrow_mut().extend([10, id]);
        (id < 2 && self.found[id as usize]).then_some(id as usize)
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
        self.out.borrow_mut().extend([11, c as i32]);
    }
    fn gadget_button_set_border(&mut self, _w: WinId, _color: i32, _draw: bool) {}
    fn border_colors(&self) -> [i32; 4] {
        [0; 4]
    }
    fn search_hot_key(&mut self, _label: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn add_hot_key(&mut self, _w: WinId, _key: Vec<u8>) {}
    fn gadget_button_set_alt_sound(&mut self, _w: WinId, _sound: &[u8]) {}
}

fn rust_case(input: &[i32]) -> Vec<i32> {
    let mut env = Env { has_window: input[1] != 0, found: [input[2] != 0, input[3] != 0], out: RefCell::new(Vec::new()) };
    let display = (input[4], input[5]);
    let n = input[9] as usize;
    let mut m = ControlBarSchemeManager::new();
    for i in 0..n {
        let w = &input[10 + i * 4..14 + i * 4];
        m.schemes.push(ControlBarScheme { name: STRINGS[w[0] as usize % 15].to_ascii_lowercase(), side: STRINGS[w[1] as usize % 15].to_vec(), screen_creation_res: ICoord2D { x: w[2], y: w[3] }, ..Default::default() });
    }
    m.current = (input[8] >= 0 && (input[8] as usize) < n).then_some(input[8] as usize);
    // setControlBarSchemeByPlayer: the communicator, then the scheme and its init
    set_control_bar_scheme_by_player_communicator(input[0] != 0, &mut env);
    let side = STRINGS[input[7] as usize % 15];
    let init = m.set_control_bar_scheme_by_player((input[6] != 0).then_some(side), display);
    let mut out = env.out.into_inner();
    if let Some(i) = init {
        out.extend([30, i as i32]);
    }
    out.extend([20, m.current.map_or(-1, |c| c as i32), m.multiplier.0.to_bits() as i32, m.multiplier.1.to_bits() as i32]);
    out
}

fn cpp_case(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1000];
    let n = unsafe { cpp::ref_scheme_by_player_case(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let (w, h) = match g.below(5) {
        0 => (800, 600),
        1 => (1024, 768),
        2 => (390, 844),
        3 => (844, 390),
        _ => (1 + g.below(4000) as i32, 1 + g.below(4000) as i32),
    };
    let n = g.below(9) as i32;
    let current = if g.below(3) == 0 { -1 } else { g.below(n as u32 + 1) as i32 };
    let mut v = vec![i32::from(g.below(2) == 0), i32::from(g.below(6) != 0), i32::from(g.below(5) != 0), i32::from(g.below(5) != 0), w, h, i32::from(g.below(6) != 0), g.below(15) as i32, current, n];
    for _ in 0..n {
        let res = match g.below(4) {
            0 | 1 => (800, 600),
            2 => (1600, 1200),
            _ => (1 + g.below(3000) as i32, 1 + g.below(3000) as i32),
        };
        v.extend([g.below(15) as i32, g.below(15) as i32, res.0, res.1]);
    }
    v
}

#[test]
fn the_scheme_by_player_is_upstreams() {
    let cases: u64 = std::env::var("SCHEME_BY_PLAYER_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x146_1165);
    let mut whole = Check::new("setControlBarSchemeByPlayer");
    let (mut briefing, mut communicator, mut inits, mut none) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        let r = rust_case(&input);
        let c = cpp_case(&input);
        let mut k = 0;
        while k + 4 < c.len() {
            match (c[k], c[k + 1]) {
                (11, 0) => communicator += 1,
                (11, 1) => briefing += 1,
                (30, _) => inits += 1,
                _ => {}
            }
            k += 2;
        }
        if c[c.len() - 3] == -1 {
            none += 1;
        }
        if r != c {
            println!("case {i} {input:?}: rust {r:?} c++ {c:?}");
        }
        whole.eq(r, c, || format!("case {i}"));
    }
    println!("{cases} cases: the communicator given NonCommand_Communicator {communicator}, NonCommand_BriefingHistory {briefing}; {inits} scheme inits; {none} with no scheme");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
