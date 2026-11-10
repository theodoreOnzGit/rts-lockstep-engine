#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/WindowXlat.cpp
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (the input gate, isSystemMessage)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/SelectionXlat.cpp (the input gate)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (setInputEnabled)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The client's input gates (GitHub #116) against the C++: `client::window_xlat`, `client::command_xlat`'s
//! `input_disabled_destroys`, `client::selection_xlat`'s `input_disabled_gate` and `client::in_game_ui`'s
//! `set_input_enabled`.
//!
//! # Method
//!
//! The C++ side is the upstream text cut verbatim by build.rs (`differential/wrap_input_gate.cpp` lists the cuts; the two
//! `translateGameMessage` pieces end at the gate and are closed with `return disp; }`), compiled on stand-ins for the
//! window manager, the shell, the tactical view, the mouse, the display, the global data and the InGameUI, with the real
//! `GameMessage` of the message stream build. Each case is random: a message type (every raw mouse and key message,
//! `MSG_META_TOGGLE_ATTACKMOVE`, `MSG_META_OPTIONS`, `MSG_CLEAR_GAME_DATA`, the four system messages, and any number
//! below 1,200), its arguments (positions, a drag's delta, the wheel's count with 0 and negatives, keys with Escape one
//! time in three and integers past a byte), and every flag the gates read. The four kinds of case are drawn in turn:
//!
//! - the window translator: its disposition, the `stopMovie` calls, and every `winProcessMouseEvent` (window message,
//!   position, the data's kind and value) and `winProcessKey` (key and state bytes) call, in order;
//! - the command translator's gate: its disposition;
//! - the selection translator's gate: its disposition, `m_dragSelecting` and `isSelecting` after, and the calls that end a
//!   drag (`endAreaSelectHint`, `setMouseLock(FALSE)`), which the Rust counts as "a drag ended";
//! - `setInputEnabled`: the input and the five flags after; the four camera flags it clears (not InGameUI members in
//!   the port) are compared with the branch the Rust takes (enabled before and disabling).
//!
//! # Results (2026-11-10, `INPUT_GATE_CASES=20000`, the default)
//!
//! 20,000 cases, 5,000 of each kind: **0 mismatches** in each of the four. Of the window translator's cases 1,001 were
//! destroyed and 2 stopped a movie (the movie branch needs an unused Escape release, a movie and the exit allowed: few
//! cases reach it); 1,235 selection cases ended a drag.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose): **8 of 8 caught** (2026-11-10).
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 9021 | `setInputEnabled` clears the modes on every disable, not only from enabled | 1,163 cases |
//! | 9022 | `setInputEnabled` leaves prefer selection on | 627 cases |
//! | 9023 | the command gate destroys `MSG_CLEAR_GAME_DATA` | 61 cases |
//! | 9024 | `isSystemMessage` forgets `MSG_DESTROY_SELECTED_GROUP` | 68 cases |
//! | 9025 | the selection gate does not end the drag | 1,235 cases |
//! | 9026 | the window translator destroys a release while a placement is anchored | 43 cases |
//! | 9027 | the window translator lets a drag through with the input off | 37 cases |
//! | 9028 | the window translator lets an Escape release through with the input off | 3 cases (few: see the results) |

mod support;

use rts_lockstep_engine::client::command_xlat::input_disabled_destroys;
use rts_lockstep_engine::client::in_game_ui::InGameUiSelection;
use rts_lockstep_engine::client::selection_xlat::{Disposition, SelectionTranslator};
use rts_lockstep_engine::client::window_manager::WinInputReturnCode;
use rts_lockstep_engine::client::window_xlat::{translate_game_message, MouseData, RawMessage, WindowXlatHost};
use rts_lockstep_engine::common::base_type::ICoord2D;
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::common::message_types::*;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

fn cpp_case(kind: i32, input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 256];
    let n = unsafe { cpp::ref_input_gate_case(kind, input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn disp(d: Disposition) -> i32 {
    match d {
        Disposition::Keep => 0,
        Disposition::Destroy => 1,
    }
}

/// The window translator's host: the flags of a case, and its calls in the C++ stand-ins' words.
struct Host {
    flags: i32,
    status: ICoord2D,
    out: Vec<i32>,
    stops: i32,
}

impl Host {
    fn flag(&self, bit: i32) -> bool {
        (self.flags >> bit) & 1 != 0
    }
}

impl WindowXlatHost for Host {
    fn mouse_locked(&self) -> bool {
        self.flag(0)
    }
    fn ui_is_scrolling(&self) -> bool {
        self.flag(1)
    }
    fn ui_input_enabled(&self) -> bool {
        self.flag(2)
    }
    fn ui_placement_anchored(&self) -> bool {
        self.flag(3)
    }
    fn shell_active(&self) -> bool {
        self.flag(4)
    }
    fn mouse_status_pos(&self) -> ICoord2D {
        self.status
    }
    fn win_process_mouse_event(&mut self, gwm: u32, pos: ICoord2D, data: MouseData) -> WinInputReturnCode {
        let (kind, x, y) = match data {
            MouseData::None => (0, 0, 0),
            MouseData::Delta(d) => (1, d.x, d.y),
            MouseData::Wheel(w) => (2, w, 0),
        };
        self.out.extend([gwm as i32, pos.x, pos.y, kind, x, y]);
        if self.flag(5) { WinInputReturnCode::Used } else { WinInputReturnCode::NotUsed }
    }
    fn win_process_key(&mut self, key: u8, state: u8) -> WinInputReturnCode {
        self.out.extend([100, i32::from(key), i32::from(state), 0, 0, 0]);
        if self.flag(6) { WinInputReturnCode::Used } else { WinInputReturnCode::NotUsed }
    }
    fn movie_playing(&self) -> bool {
        self.flag(7)
    }
    fn allow_exit_out_of_movies(&self) -> bool {
        self.flag(8)
    }
    fn stop_movie(&mut self) {
        self.stops += 1;
    }
}

fn gen_type(g: &mut Gen) -> MessageType {
    const POOL: [MessageType; 9] =
        [MSG_META_TOGGLE_ATTACKMOVE, MSG_META_OPTIONS, MSG_CLEAR_GAME_DATA, MSG_DESTROY_SELECTED_GROUP, MSG_LOGIC_CRC, MSG_SET_REPLAY_CAMERA, MSG_FRAME_TICK, MSG_DO_MOVETO, MSG_MOUSE_LEFT_CLICK];
    match g.below(4) {
        0 | 1 => MessageType((3 + g.below(18)) as _),
        2 => POOL[g.below(POOL.len() as u32) as usize],
        _ => MessageType(g.below(1200) as _),
    }
}

fn small(g: &mut Gen) -> i32 {
    g.below(1601) as i32 - 800
}

/// One window translator case: its input words and the Rust's output.
fn window_case(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let t = gen_type(g);
    let is_key = t == MSG_RAW_KEY_DOWN || t == MSG_RAW_KEY_UP;
    let pos = ICoord2D { x: small(g), y: small(g) };
    let arg0 = if g.below(3) == 0 { 1 } else if g.below(2) == 0 { g.below(256) as i32 } else { g.i32() };
    let arg1 = match g.below(6) {
        0 => 0,
        1 => 120,
        2 => -120,
        3 => g.below(4) as i32,
        _ => g.i32(),
    };
    let delta = ICoord2D { x: arg1, y: small(g) };
    let flags = g.below(512) as i32;
    let status = ICoord2D { x: small(g), y: small(g) };
    let m = RawMessage { t, pos: if is_key { ICoord2D { x: arg0, y: 0 } } else { pos }, delta, arg0, arg1 };
    let mut h = Host { flags, status, out: Vec::new(), stops: 0 };
    let d = translate_game_message(&m, &mut h);
    let mut rust = vec![disp(d), h.stops];
    rust.extend(h.out);
    let x = if is_key { arg0 } else { pos.x };
    let input = vec![t.0 as i32, x, pos.y, arg1, delta.y, flags, status.x, status.y];
    (input, rust)
}

#[test]
fn input_gates_match_upstream() {
    let cases: u64 = std::env::var("INPUT_GATE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x116_1160);
    let (mut win, mut cmd, mut sel, mut ui) =
        (Check::new("WindowTranslator::translateGameMessage"), Check::new("CommandTranslator input gate"), Check::new("SelectionTranslator input gate"), Check::new("InGameUI::setInputEnabled"));
    let (mut destroyed, mut stops, mut ended) = (0u64, 0u64, 0u64);
    for i in 0..cases {
        match i % 4 {
            0 => {
                let (input, rust) = window_case(&mut g);
                let c = cpp_case(0, &input);
                destroyed += c.first().copied().unwrap_or(0) as u64;
                stops += c.get(1).copied().unwrap_or(0) as u64;
                win.eq(rust, c, || format!("case {i} {input:?}"));
            }
            1 => {
                let t = gen_type(&mut g);
                let on = g.below(2) == 1;
                let rust = vec![i32::from(input_disabled_destroys(t, on))];
                cmd.eq(rust, cpp_case(1, &[t.0 as i32, i32::from(on)]), || format!("case {i} {t:?} input {on}"));
            }
            2 => {
                let t = gen_type(&mut g);
                let (on, drag, selecting) = (g.below(2) == 1, g.below(2) == 1, g.below(2) == 1);
                let mut x = SelectionTranslator { drag_selecting: drag, ..Default::default() };
                let mut s = InGameUiSelection::new();
                s.set_input_enabled(on);
                s.set_selecting(selecting);
                let d = x.input_disabled_gate(&mut s).unwrap_or(Disposition::Keep);
                let end = i32::from(drag && !x.drag_selecting);
                ended += end as u64;
                let rust = vec![disp(d), i32::from(x.drag_selecting), i32::from(s.is_selecting()), end, end];
                sel.eq(rust, cpp_case(2, &[t.0 as i32, i32::from(on), i32::from(drag), i32::from(selecting)]), || format!("case {i} input {on} drag {drag} selecting {selecting}"));
            }
            _ => {
                let b: Vec<bool> = (0..7).map(|_| g.below(2) == 1).collect();
                let mut s = InGameUiSelection::new();
                s.set_input_enabled(b[1]);
                s.set_selecting(b[2]);
                s.set_force_attack_mode(b[3]);
                s.set_force_move_mode(b[4]);
                s.set_waypoint_mode(b[5]);
                s.set_prefer_selection_mode(b[6]);
                s.set_input_enabled(b[0]);
                let cam = i32::from(b[1] && !b[0]);
                let rust = vec![
                    i32::from(s.input_enabled()),
                    i32::from(s.is_selecting()),
                    i32::from(s.is_in_force_attack_mode()),
                    i32::from(s.is_in_force_move_to_mode()),
                    i32::from(s.is_in_waypoint_mode()),
                    i32::from(s.is_in_prefer_selection_mode()),
                    cam,
                    cam,
                    cam,
                    cam,
                ];
                let input: Vec<i32> = b.iter().map(|&x| i32::from(x)).collect();
                ui.eq(rust, cpp_case(3, &input), || format!("case {i} {b:?}"));
            }
        }
    }
    println!("window cases destroyed {destroyed}, movies stopped {stops}; selection drags ended {ended}");
    for c in [win, cmd, sel, ui] {
        if planted {
            c.report();
        } else {
            c.finish();
        }
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}
