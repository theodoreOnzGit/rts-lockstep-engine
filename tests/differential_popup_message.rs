#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (popupMessage, clearPopupMessageData)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/InGamePopupMessage.cpp (InGamePopupMessageInit,
//     InGamePopupMessageInput, InGamePopupMessageSystem)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::in_game_popup_message` (GitHub #151) against InGameUI.cpp and InGamePopupMessage.cpp.
//!
//! # Method
//!
//! The C++ side is InGameUI.cpp's two `popupMessage`s and `clearPopupMessageData` and InGamePopupMessage.cpp's statics
//! and three callbacks, cut verbatim by build.rs, on stand-ins that record each call (`differential/wrap_popup_message.cpp`
//! documents the model and the words); the Rust side is the same model as a [`PopupMessageHost`]. Each sequence has a
//! display (800x600, a phone's 390x844, or any size), a `PopupMessageColor`, the static text's font (none, or 6 to 24
//! points) and the OK button's size, and one time in twenty a layout missing one of its three windows (undefined
//! upstream: the sequence stops before the step, on both sides). Steps: messages (five labels, the empty one and a long
//! one among them) at x and y from -30 to 130 % and widths from -10 to 400, pausing or not, with the default colour or
//! one given; clears; while the layout is up, `InGamePopupMessageInput` (Enter, Escape or another key, pressed or released,
//! or another message) and `InGamePopupMessageSystem` (create, destroy, the focus offered or not, OK, the text or the
//! parent clicked, another message). Compared: every call in order (the pause, the history, the layout made, run and
//! destroyed, each window's position, size, colours, modality, focus, visibility and order, the text set, the clear
//! message, the simulated click) and after each step the message's data, the statics, the handled result and the focus
//! flag.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches over 20,000 sequences. If any, the place's `UnsignedInt * Real` truncation on odd display sizes.
//! - Plants 9441-9448 all caught: 9441 (x clamped to 99) by any x of 100 or more; 9442 (no minimum width) by widths
//!   under 50; 9443 (the clear does not unpause) by any clear of a pausing message; 9444 (wrapped at width - 4) by
//!   messages that wrap differently, the long label; 9445 (a pausing message not modal) by any pausing message; 9446
//!   (Escape not taken) by Escape; 9447 (the focus refused) by the focus offered; 9448 (OK of a pausing message sends
//!   the message) by OK on a pausing message.
//!
//! # Results (2026-11-12, `POPUP_CASES=20000`, the default)
//!
//! - First run: 20,000 sequences, 390,313 steps, 184,086 messages; 1,029 stopped at a missing window (undefined);
//!   **0 mismatches**, as predicted.
//! - Plants, sequences that differ of 20,000: 9441 14,611; 9442 13,267; 9443 17,179; 9444 12,548; 9445 17,890; 9446
//!   12,842; 9447 5,414; 9448 10,402. **8 of 8 caught**, as predicted.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 9441 | x is clamped to 99 % |
//! | 9442 | the width has no lower limit |
//! | 9443 | clearing a pausing message does not unpause |
//! | 9444 | the text wraps at `width - 4` |
//! | 9445 | a pausing message is not modal |
//! | 9446 | Escape is not a click on OK |
//! | 9447 | the window refuses the focus |
//! | 9448 | OK of a pausing message sends `MSG_CLEAR_INGAME_POPUP_MESSAGE` |

mod support;

use rts_lockstep_engine::client::in_game_popup_message::*;
use rts_lockstep_engine::client::window_manager::{ggm, gwm, MsgHandled};
use rts_lockstep_engine::client::window_script::{GameFont, WinId};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

const NAMES: [&[u8]; 3] = [b"InGamePopupMessage.wnd:InGamePopupMessageParent", b"InGamePopupMessage.wnd:StaticTextMessage", b"InGamePopupMessage.wnd:ButtonOk"];
const LABELS: [&[u8]; 5] = [b"SCRIPT:Pop", b"SCRIPT:Brief1", b"", b"A much longer popup message that wraps over several lines of the box", b"MISSING"];

fn fnv(t: &[u16]) -> i32 {
    let mut h: u32 = 2166136261;
    for &u in t {
        h ^= u32::from(u);
        h = h.wrapping_mul(16777619);
    }
    h as i32
}

/// The wrapper's model.
struct Model {
    p: InGamePopupMessage,
    keys: Vec<Vec<u8>>,
    made: bool,
    size: (i32, i32),
    present: i32,
    font_size: i32,
    ok: (i32, i32),
    out: Vec<i32>,
}

impl PopupMessageHost for Model {
    fn popup(&mut self) -> &mut InGamePopupMessage {
        &mut self.p
    }
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        label.iter().map(|&b| (b as i8) as i16 as u16).collect()
    }
    fn update_diplomacy_briefing_text(&mut self, label: &[u8]) {
        let li = LABELS.iter().rposition(|l| *l == label).map_or(-1, |i| i as i32);
        self.out.extend([2, li]);
    }
    fn display_size(&self) -> (i32, i32) {
        self.size
    }
    fn set_game_paused(&mut self, paused: bool, pause_music: bool) {
        self.out.extend([1, i32::from(paused), i32::from(pause_music)]);
    }
    fn win_create_layout(&mut self, _file: &[u8]) -> Option<usize> {
        self.out.push(3);
        self.made = true;
        Some(0)
    }
    fn layout_run_init(&mut self, _layout: usize) {
        self.out.push(4);
        in_game_popup_message_init(self);
    }
    fn layout_destroy(&mut self, _layout: usize) {
        self.out.push(5);
        self.made = false;
    }
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        if let Some(i) = self.keys.iter().position(|k| k == name) {
            return i as i32 + 1;
        }
        self.keys.push(name.to_vec());
        self.keys.len() as i32
    }
    fn win_get_window_from_id(&mut self, _parent: Option<WinId>, id: i32) -> Option<WinId> {
        if !self.made {
            return None;
        }
        (0..3).find(|&i| self.present & (1 << i) != 0 && self.name_to_key(NAMES[i]) == id)
    }
    fn win_get_font(&mut self, _w: WinId) -> Option<GameFont> {
        (self.font_size != 0).then(|| GameFont { name: b"F".to_vec(), point_size: self.font_size, bold: false })
    }
    fn string_size(&mut self, text: &[u16], font: Option<&GameFont>, wrap: i32) -> (i32, i32) {
        let unit = font.map_or(8, |f| f.point_size / 2 + 1);
        let line = font.map_or(12, |f| f.point_size);
        let total = text.len() as i32 * unit;
        let wraps = wrap > 0 && total > wrap;
        let lines = if wraps { (total + wrap - 1) / wrap } else { 1 };
        (if wraps { wrap } else { total }, lines * line)
    }
    fn static_text_set_text(&mut self, w: WinId, text: &[u16]) {
        self.out.extend([13, w as i32, fnv(text)]);
    }
    fn win_get_size(&mut self, _w: WinId) -> (i32, i32) {
        self.ok
    }
    fn win_set_position(&mut self, w: WinId, x: i32, y: i32) {
        self.out.extend([6, w as i32, x, y]);
    }
    fn win_set_size(&mut self, w: WinId, width: i32, height: i32) {
        self.out.extend([7, w as i32, width, height]);
    }
    fn win_set_enabled_text_colors(&mut self, w: WinId, color: i32, border_color: i32) {
        self.out.extend([8, w as i32, color, border_color]);
    }
    fn win_set_modal(&mut self, w: WinId) {
        self.out.extend([9, w as i32]);
    }
    fn win_set_focus(&mut self, w: WinId) {
        self.out.extend([10, w as i32]);
    }
    fn win_hide(&mut self, w: WinId, hide: bool) {
        self.out.extend([11, w as i32, i32::from(hide)]);
    }
    fn win_bring_to_top(&mut self, w: WinId) {
        self.out.extend([12, w as i32]);
    }
    fn send_selected(&mut self, window: WinId, control: Option<WinId>, control_id: i32) {
        self.out.extend([15, window as i32, control.map_or(-1, |c| c as i32), control_id]);
        if window == 0 {
            let id = control.map(|c| self.name_to_key(NAMES[c]));
            let _ = in_game_popup_message_system(self, ggm::GBM_SELECTED, id, None);
        }
    }
    fn append_clear_popup_message(&mut self) {
        self.out.push(14);
    }
}

fn handled(m: MsgHandled) -> i32 {
    match m {
        MsgHandled::Ignored => 0,
        MsgHandled::Handled => 1,
    }
}

/// The Rust run of `input`; the steps it ran (it stops before one that meets upstream's undefined behaviour).
fn rust_seq(input: &[i32]) -> (Vec<i32>, usize) {
    let c = &input[..8];
    let mut m = Model { p: InGamePopupMessage::default(), keys: Vec::new(), made: false, size: (c[0], c[1]), present: c[3], font_size: c[4], ok: (c[5], c[6]), out: Vec::new() };
    let color = c[2];
    let mut ran = 0;
    for st in input[8..].chunks(8) {
        if st.len() < 8 {
            break;
        }
        let w = &st[1..];
        let before = (m.p.clone(), m.keys.clone(), m.made, m.out.len());
        let (mut h, mut focus) = (-1, 1);
        match st[0] {
            0 => popup_message(&mut m, LABELS[w[0] as usize % 5], w[1], w[2], w[3], color, w[4] != 0, w[5] != 0),
            1 => popup_message(&mut m, LABELS[w[0] as usize % 5], w[1], w[2], w[3], w[6], w[4] != 0, w[5] != 0),
            2 => clear_popup_message_data(&mut m),
            3 => {
                let key = [0x1C, 0x01, 0x1E][w[1] as usize % 3];
                h = handled(in_game_popup_message_input(&mut m, 0, if w[0] != 0 { 7 } else { gwm::CHAR }, key, w[2] as u8));
            }
            _ => {
                let msg = [gwm::CREATE, gwm::DESTROY, gwm::INPUT_FOCUS, ggm::GBM_SELECTED, 7][w[0] as usize % 5];
                let control = m.name_to_key(NAMES[w[1] as usize % 3]);
                let mut f = false;
                h = handled(in_game_popup_message_system(&mut m, msg, (msg == ggm::GBM_SELECTED).then_some(control), (msg == gwm::INPUT_FOCUS).then_some((w[2] != 0, &mut f))));
                focus = i32::from(f);
            }
        }
        if m.p.undefined.is_some() {
            m.p = before.0;
            m.keys = before.1;
            m.made = before.2;
            m.out.truncate(before.3);
            break;
        }
        m.out.extend([100, i32::from(m.p.popup_message_data.is_some())]);
        if let Some(d) = &m.p.popup_message_data {
            m.out.extend([d.x, d.y, d.width, d.text_color, i32::from(d.pause), i32::from(d.pause_music), i32::from(d.layout.is_some()), fnv(&d.message)]);
        }
        let idx = |w: Option<WinId>| w.map_or(-1, |w| w as i32);
        m.out.extend([i32::from(m.p.pause), m.p.parent_id, m.p.static_text_message_id, m.p.button_ok_id, idx(m.p.parent), idx(m.p.static_text_message), idx(m.p.button_ok), h, focus]);
        ran += 1;
    }
    (m.out, ran)
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 200_000];
    let n = unsafe { cpp::ref_popup_message_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let (w, h) = match g.below(4) {
        0 => (800, 600),
        1 => (390, 844),
        _ => (1 + g.below(2000) as i32, 1 + g.below(2000) as i32),
    };
    let present = if g.below(20) == 0 { 7 & !(1 << g.below(3)) } else { 7 };
    let font = if g.below(4) == 0 { 0 } else { 6 + g.below(19) as i32 };
    let mut v = vec![w, h, g.u32() as i32, present, font, 20 + g.below(60) as i32, 10 + g.below(30) as i32, 0];
    // (input and system messages only while the layout is up, as the window manager would give them)
    let mut up = false;
    for _ in 0..1 + g.below(40) {
        let mut s = [0i32; 8];
        let kind = match g.below(10) {
            0..=2 => 0,
            3 => 1,
            4 => 2,
            5..=7 if up => 3,
            8..=9 if up => 4,
            _ => 0,
        };
        s[0] = kind;
        match kind {
            0 | 1 => {
                s[1] = g.below(5) as i32;
                s[2] = g.below(161) as i32 - 30;
                s[3] = g.below(161) as i32 - 30;
                s[4] = g.below(411) as i32 - 10;
                s[5] = i32::from(g.below(2) == 0);
                s[6] = i32::from(g.below(2) == 0);
                s[7] = g.u32() as i32;
                up = true;
            }
            2 => up = false,
            3 => {
                s[1] = i32::from(g.below(6) == 0);
                s[2] = g.below(3) as i32;
                s[3] = g.below(4) as i32;
            }
            _ => {
                s[1] = g.below(5) as i32;
                s[2] = if g.below(4) == 0 { 1 + g.below(2) as i32 } else { 2 };
                s[3] = g.below(2) as i32;
            }
        }
        v.extend(s);
    }
    v
}

#[test]
fn the_popup_message_matches_upstream() {
    let cases: u64 = std::env::var("POPUP_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x151_0004);
    let mut whole = Check::new("the popup message");
    let (mut steps, mut stopped, mut messages, mut oks) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let mut input = gen_case(&mut g);
        let all = (input.len() - 8) / 8;
        let (r, ran) = rust_seq(&input);
        if ran < all {
            stopped += 1;
            input.truncate(8 + 8 * ran);
        }
        steps += ran as u64;
        messages += input[8..].chunks(8).filter(|s| s[0] <= 1).count() as u64;
        oks += r.windows(2).filter(|w| w[0] == 14 || (w[0] == 5 && w[1] == 1)).count() as u64;
        let c = cpp_seq(&input);
        if r != c && !planted {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(12);
            println!("sequence {i} first difference at word {k}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 8).min(r.len())], &c[k0.min(c.len())..(k + 8).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}: {input:?}"));
    }
    println!("{cases} sequences, {steps} steps, {messages} messages, {stopped} stopped where upstream is undefined (a missing window); about {oks} clear messages or clears");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
