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

//! (#155) The popup message held without a window manager (`client::in_game_popup_message::WindowlessPopup`) and shown
//! when a shell comes (`show_held_popup_message`), against upstream's InGameUI, which always has its window manager.
//!
//! # Method
//!
//! The C++ side is `tests/differential_popup_message.rs`'s, unchanged (`differential/wrap_popup_message.cpp`: InGameUI.cpp's
//! `popupMessage`s and `clearPopupMessageData` and InGamePopupMessage.cpp's callbacks, cut verbatim, on recording
//! stand-ins), run over the whole sequence with its window manager from the start. The Rust side runs the first `k`
//! steps on [`WindowlessPopup`] (no window manager: messages and clears only, as the logic sends them before a shell),
//! then hands the held message to a window manager (the same model host as `differential_popup_message.rs`) with
//! [`WindowlessPopup::take_held`] and [`show_held_popup_message`], and runs the rest there; one sequence in five never
//! gets a window manager. Each sequence: a display (800x600, 390x844 or any size), a `PopupMessageColor`, the static
//! text's font and the OK button's size (all three windows present); steps as `differential_popup_message.rs`'s (five
//! labels, x and y from -30 to 130 %, widths from -10 to 400, pausing or not, colours; clears; while a window is up,
//! Enter, Escape and the system messages).
//!
//! Compared:
//! - while windowless, each step's calls on the game and the history (`setGamePaused`, `UpdateDiplomacyBriefingText`)
//!   with the C++'s, in order, and the held data after each step (place, width, colour, pause, the message), not the
//!   window (none here, the C++'s layout) nor the window statics;
//! - at the hand-over, the window calls it makes (`winCreateLayout`, `runInit` and every call of the init) with the
//!   C++'s calls since it last made the layout (the window it has for the same message), the calls on the game it
//!   makes (none: the pause was made when the message came), and the data and the statics after it with the C++'s;
//! - after it, every call and every step's state, as `differential_popup_message.rs` compares them (the statics once the
//!   Rust side has run an init).
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches over 20,000 sequences: the windowless host makes the same calls as upstream up to `winCreateLayout`,
//!   and the hand-over's init reads only the data, which is the same.
//! - Plants all caught: 9484 (a windowless pause not made) by any pausing message before the shell; 9485 (a windowless
//!   label not given to the history) by any message before it; 9486 (the hand-over's init not run) by any hand-over of a
//!   held message; 9487 (the windowless display's width and height swapped) by any message before the shell on a display
//!   that is not square, where x or y is not 0; 9488 (the hand-over unpauses) by the hand-over of a pausing message.
//!
//! # Results (2026-11-12, `POPUP_CASES=20000`, the default)
//!
//! - First run: 20,000 sequences, 411,361 steps (246,734 of them without a window manager); 15,929 handed over, 13,231
//!   of them with a message held; 4,071 never handed over. **0 mismatches**, as predicted.
//! - Plants, sequences that differ of 20,000: 9484 17,212; 9485 18,592; 9486 13,231 (every held hand-over); 9487
//!   18,527; 9488 6,573. **5 of 5 caught.** Misses in the counts: I expected about half the hand-overs to hold a
//!   message (83 % did) and 9484 in about half the sequences (86 %: most sequences have a pausing message before the
//!   shell).
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 9484 | a windowless pausing message does not pause |
//! | 9485 | a windowless message's label is not given to the history |
//! | 9486 | the hand-over makes the layout but does not run its init |
//! | 9487 | the windowless display's width and height are swapped |
//! | 9488 | the hand-over unpauses a pausing message |

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

fn fetch(label: &[u8]) -> Vec<u16> {
    label.iter().map(|&b| (b as i8) as i16 as u16).collect()
}

fn label_index(label: &[u8]) -> i32 {
    LABELS.iter().rposition(|l| *l == label).map_or(-1, |i| i as i32)
}

/// The window manager's model (as `differential_popup_message.rs`'s).
struct Model {
    p: InGamePopupMessage,
    keys: Vec<Vec<u8>>,
    made: bool,
    size: (i32, i32),
    font_size: i32,
    ok: (i32, i32),
    out: Vec<i32>,
}

impl PopupMessageHost for Model {
    fn popup(&mut self) -> &mut InGamePopupMessage {
        &mut self.p
    }
    fn fetch(&mut self, label: &[u8]) -> Vec<u16> {
        fetch(label)
    }
    fn update_diplomacy_briefing_text(&mut self, label: &[u8]) {
        self.out.extend([2, label_index(label)]);
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
        (0..3).find(|&i| self.name_to_key(NAMES[i]) == id)
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

/// One record of the wrapper's words: a call (its words), or a step's state block.
#[derive(Clone, Debug, PartialEq)]
enum Rec {
    Call(Vec<i32>),
    /// [`data words or empty`, statics (9 words: pause, the three ids, the three windows, handled, focus)].
    State(Option<Vec<i32>>, Vec<i32>),
}

/// The wrapper's words as records.
fn parse(words: &[i32]) -> Vec<Rec> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < words.len() {
        let n = match words[k] {
            1 | 11 | 13 => 3,
            2 | 9 | 10 | 12 => 2,
            3 | 4 | 5 | 14 => 1,
            6 | 7 | 8 | 15 => 4,
            100 => {
                let has = words[k + 1] != 0;
                let d = if has { Some(words[k + 2..k + 10].to_vec()) } else { None };
                let at = k + 2 + if has { 8 } else { 0 };
                out.push(Rec::State(d, words[at..at + 9].to_vec()));
                k = at + 9;
                continue;
            }
            w => panic!("unknown word {w} at {k}"),
        };
        out.push(Rec::Call(words[k..k + n].to_vec()));
        k += n;
    }
    out
}

/// A record as compared while there is no window manager: calls on the game and the history only; the state without
/// the layout flag and the statics.
fn windowless_view(r: &Rec) -> Option<Rec> {
    match r {
        Rec::Call(c) if c[0] == 1 || c[0] == 2 => Some(r.clone()),
        Rec::Call(_) => None,
        Rec::State(d, _) => Some(Rec::State(d.as_ref().map(|d| d[..6].iter().chain(&d[7..]).copied().collect()), Vec::new())),
    }
}

fn data_words(p: &InGamePopupMessage) -> Option<Vec<i32>> {
    p.popup_message_data.as_ref().map(|d| vec![d.x, d.y, d.width, d.text_color, i32::from(d.pause), i32::from(d.pause_music), i32::from(d.layout.is_some()), fnv(&d.message)])
}

fn statics(p: &InGamePopupMessage, h: i32, focus: i32) -> Vec<i32> {
    let idx = |w: Option<WinId>| w.map_or(-1, |w| w as i32);
    vec![i32::from(p.pause), p.parent_id, p.static_text_message_id, p.button_ok_id, idx(p.parent), idx(p.static_text_message), idx(p.button_ok), h, focus]
}

fn handled(m: MsgHandled) -> i32 {
    match m {
        MsgHandled::Ignored => 0,
        MsgHandled::Handled => 1,
    }
}

/// The comparison of one sequence: `None` when it holds, else what differs first.
fn compare(input: &[i32], handover: Option<usize>, c: &[Rec]) -> Option<String> {
    let cfg = &input[..8];
    let color = cfg[2];
    let steps: Vec<&[i32]> = input[8..].chunks(8).collect();
    // the C++'s records by step (each step's ends with its state)
    let mut by_step: Vec<Vec<Rec>> = vec![Vec::new()];
    for r in c {
        by_step.last_mut().expect("step").push(r.clone());
        if matches!(r, Rec::State(..)) {
            by_step.push(Vec::new());
        }
    }
    by_step.pop();
    if by_step.len() != steps.len() {
        return Some(format!("the C++ ran {} steps of {}", by_step.len(), steps.len()));
    }
    let k = handover.unwrap_or(steps.len()).min(steps.len());
    let mut w = WindowlessPopup { display: (cfg[0], cfg[1]), ..Default::default() };
    for (i, st) in steps[..k].iter().enumerate() {
        let a = &st[1..];
        match st[0] {
            0 => w.popup_message(fetch, LABELS[a[0] as usize % 5], a[1], a[2], a[3], color, a[4] != 0, a[5] != 0),
            1 => w.popup_message(fetch, LABELS[a[0] as usize % 5], a[1], a[2], a[3], a[6], a[4] != 0, a[5] != 0),
            _ => w.clear_popup_message_data(),
        }
        let mut r: Vec<Rec> = std::mem::take(&mut w.calls)
            .into_iter()
            .map(|c| match c {
                WindowlessCall::SetGamePaused { paused, pause_music } => Rec::Call(vec![1, i32::from(paused), i32::from(pause_music)]),
                WindowlessCall::Briefing(l) => Rec::Call(vec![2, label_index(&l)]),
            })
            .collect();
        r.push(Rec::State(data_words(&w.popup), Vec::new()));
        let r: Vec<Rec> = r.iter().filter_map(windowless_view).collect();
        let cv: Vec<Rec> = by_step[i].iter().filter_map(windowless_view).collect();
        if r != cv {
            return Some(format!("windowless step {i}: rust {r:?} c++ {cv:?}"));
        }
    }
    let Some(_) = handover else { return None };
    // the hand-over: the window the C++ made for the held message (its calls since its last winCreateLayout)
    let mut m = Model { p: InGamePopupMessage::default(), keys: Vec::new(), made: false, size: (cfg[0], cfg[1]), font_size: cfg[4], ok: (cfg[5], cfg[6]), out: Vec::new() };
    let mut known = false;
    if let Some(held) = w.take_held() {
        let made: Vec<Rec> = w.calls.drain(..).map(|c| Rec::Call(format!("{c:?}").bytes().map(i32::from).collect())).collect();
        show_held_popup_message(&mut m, held);
        known = true;
        let mut r = made;
        r.extend(parse(&m.out));
        m.out.clear();
        r.push(Rec::State(data_words(&m.p), statics(&m.p, -1, 1)));
        let flat: Vec<Rec> = by_step[..k].iter().flatten().cloned().collect();
        let Some(at) = flat.iter().rposition(|x| *x == Rec::Call(vec![3])) else { return Some("held, but the C++ never made a layout".into()) };
        let mut cv: Vec<Rec> = flat[at..].iter().filter(|x| !matches!(x, Rec::Call(c) if c[0] == 1 || c[0] == 2) && !matches!(x, Rec::State(..))).cloned().collect();
        cv.push(flat.iter().rev().find(|x| matches!(x, Rec::State(..))).cloned().expect("a state"));
        if r != cv {
            return Some(format!("hand-over after step {k}: rust {r:?} c++ {cv:?}"));
        }
    }
    // after it: everything
    for (i, st) in steps.iter().enumerate().skip(k) {
        let a = &st[1..];
        let (mut h, mut focus) = (-1, 1);
        match st[0] {
            0 => popup_message(&mut m, LABELS[a[0] as usize % 5], a[1], a[2], a[3], color, a[4] != 0, a[5] != 0),
            1 => popup_message(&mut m, LABELS[a[0] as usize % 5], a[1], a[2], a[3], a[6], a[4] != 0, a[5] != 0),
            2 => clear_popup_message_data(&mut m),
            3 => {
                let key = [0x1C, 0x01, 0x1E][a[1] as usize % 3];
                h = handled(in_game_popup_message_input(&mut m, 0, if a[0] != 0 { 7 } else { gwm::CHAR }, key, a[2] as u8));
            }
            _ => {
                let msg = [gwm::CREATE, gwm::DESTROY, gwm::INPUT_FOCUS, ggm::GBM_SELECTED, 7][a[0] as usize % 5];
                let control = m.name_to_key(NAMES[a[1] as usize % 3]);
                let mut f = false;
                h = handled(in_game_popup_message_system(&mut m, msg, (msg == ggm::GBM_SELECTED).then_some(control), (msg == gwm::INPUT_FOCUS).then_some((a[2] != 0, &mut f))));
                focus = i32::from(f);
            }
        }
        known |= st[0] <= 1;
        let mut r = parse(&m.out);
        m.out.clear();
        r.push(Rec::State(data_words(&m.p), statics(&m.p, h, focus)));
        let mut cv = by_step[i].clone();
        if !known {
            // (the C++'s statics are its last init's, which the Rust side never ran: not compared until it runs one)
            for x in r.iter_mut().chain(cv.iter_mut()) {
                if let Rec::State(_, s) = x {
                    s.clear();
                }
            }
        }
        if r != cv {
            return Some(format!("step {i} (after the hand-over at {k}): rust {r:?} c++ {cv:?}"));
        }
    }
    None
}

fn gen_case(g: &mut Gen) -> (Vec<i32>, Option<usize>) {
    let (w, h) = match g.below(4) {
        0 => (800, 600),
        1 => (390, 844),
        _ => (1 + g.below(2000) as i32, 1 + g.below(2000) as i32),
    };
    let font = if g.below(4) == 0 { 0 } else { 6 + g.below(19) as i32 };
    let mut v = vec![w, h, g.u32() as i32, 7, font, 20 + g.below(60) as i32, 10 + g.below(30) as i32, 0];
    let n = 1 + g.below(40) as usize;
    let handover = if g.below(5) == 0 { None } else { Some(g.below(n as u32 + 1) as usize) };
    let k = handover.unwrap_or(n);
    // (input and system messages only while a window is up: after the hand-over, with a message up)
    let mut up = false;
    for i in 0..n {
        let windowed = i >= k;
        let mut s = [0i32; 8];
        let kind = match g.below(10) {
            0..=2 => 0,
            3 => 1,
            4 => 2,
            5..=7 if up && windowed => 3,
            8..=9 if up && windowed => 4,
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
                // (an OK of a pausing message, or a clear by the input, takes the window down; the model keeps `up`
                // conservatively: a step on no window is still upstream's to answer)
            }
        }
        v.extend(s);
    }
    (v, handover)
}

#[test]
fn the_windowless_popup_and_its_hand_over_match_upstream() {
    let cases: u64 = std::env::var("POPUP_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x155_0004);
    let mut whole = Check::new("the windowless popup and its hand-over");
    let (mut steps, mut windowless_steps, mut handed, mut held, mut never) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut shown = 0;
    for i in 0..cases {
        let (input, handover) = gen_case(&mut g);
        let n = (input.len() - 8) / 8;
        steps += n as u64;
        windowless_steps += handover.unwrap_or(n) as u64;
        match handover {
            Some(_) => handed += 1,
            None => never += 1,
        }
        let mut out = vec![0i32; 200_000];
        let len = unsafe { cpp::ref_popup_message_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
        out.truncate(len as usize);
        let c = parse(&out);
        // (a message held at the hand-over: the C++'s state before it has the data)
        if let Some(k) = handover {
            let mut seen = 0;
            for r in &c {
                if let Rec::State(d, _) = r {
                    seen += 1;
                    if seen == k && d.is_some() {
                        held += 1;
                    }
                }
            }
        }
        let diff = compare(&input, handover, &c);
        if let Some(d) = &diff {
            if shown < 3 && !planted {
                println!("sequence {i}: {d}");
                shown += 1;
            }
        }
        whole.eq(diff.is_none(), true, || format!("sequence {i}: {input:?} handover {handover:?}: {}", diff.clone().unwrap_or_default()));
    }
    println!("{cases} sequences, {steps} steps ({windowless_steps} without a window manager); {handed} handed over ({held} with a message held), {never} never");
    if planted {
        whole.finish_planted();
    } else {
        whole.finish();
    }
}
