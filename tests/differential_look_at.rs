#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/LookAtXlat.cpp
//   GeneralsMD/Code/GameEngine/Include/GameClient/LookAtXlat.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::look_at_xlat::LookAtTranslator` (GitHub #119) against the C++.
//!
//! # Method
//!
//! The C++ side is LookAtXlat.cpp cut verbatim by build.rs from its translator pointer to the end, with the header's
//! class declaration (`differential/wrap_look_at.cpp` lists the cuts and the one edit, a `static` dropped from an
//! anonymous enum), compiled as the release build compiles it on stand-ins for the InGameUI, the mouse, the shell, the
//! stats collector, the game logic and client, the display, the global data and the message stream, and on a
//! **recorder of the tactical view's calls**. Each case is a random sequence of 1 to 40 steps on a new translator: a
//! message (the raw keys with the arrows and other bytes, states with and without `KEY_STATE_UP` and integers past a
//! byte; both right and middle buttons; positions near and past the display's edges, negative ones included; the wheel
//! from -4 to 4; `MSG_META_OPTIONS`; the frame tick; the 16 bookmark messages; any type below 1,200), or a change of the
//! world around it: the InGameUI's input, selection and scrolling (a script's stop), the shell, the RMB anchor option,
//! windowed, the replay camera and the game's kind, a stats collector, the logic and client frames (the client frame
//! may go back), the display's size (0 to 3 pixels among them), the cursor, `resetModes`, `setCurrentPos`, the scroll
//! factors and the view's angle and pitch. Both sides write every call that acts and, after each step, the
//! translator's members, `getRMBScrollAnchor` and `hasMouseMovedRecently`, as words (floats as bits); the words are
//! compared whole, and the float words of records that line up are also compared for their ULP distance.
//!
//! # Predictions (written before the first run, 2026-11-10)
//!
//! - 0 mismatches over 20,000 sequences. The floats are single `float` operations in upstream's order (the RMB
//!   minimum's `normalize` is `BaseType`'s `sqrt` in `double`, already translated), so **max ULP 0**.
//! - Each planted bug (9041 to 9048) is caught by at least one sequence; 9042 and 9048 the fewest, since they need a
//!   middle button held across `resetModes`, or a click that moves up by more than 5 pixels.
//!
//! **The first run missed (153 of 20,000 sequences):** the middle button's release reset the view in the Rust where the
//! C++ did not. Read upstream again: `PIXEL_OFFSET` is a `const UnsignedInt`, so `dy > PIXEL_OFFSET` is an unsigned
//! comparison and a move up of any size counts as moved. The translation had taken it as a signed `Int`; corrected,
//! and the module's quirk rewritten (it had said the opposite: that a move up never counts).
//!
//! # Results (2026-11-10, `LOOK_AT_CASES=20000`, the default)
//!
//! 20,000 sequences, 442,590 steps: **0 mismatches**; 247,290 float words compared (the scroll offsets, the angles, the
//! locations), **max ULP 0**. The sequences made 7,233 `scrollBy` calls and 12,760 replay cameras, and destroyed 24,982
//! bookmark messages. (After the one fix below: the first run missed.)
//!
//! Planted bugs: **8 of 8 caught**. 9048 the fewest, as predicted; 9042 was not among the few (a rotation left on is
//! seen at the next mouse move).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints `MEASURED` lines and fails on purpose):
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 9041 | `setScrolling` ignores the input gate (LookAtXlat.cpp:76) | 2,440 sequences |
//! | 9042 | `resetModes` leaves the rotation on | 3,541 |
//! | 9043 | the screen edge scroll does not start at the right edge | 830 |
//! | 9044 | the wheel does not fall into `MSG_META_OPTIONS`'s stop | 9,249 |
//! | 9045 | the RMB scroll's minimum is not added to y | 421 |
//! | 9046 | the right arrow does not scroll | 426 |
//! | 9047 | the replay camera is never appended | 6,200 |
//! | 9048 | the middle button's click test takes the absolute y movement (upstream compares it unsigned) | 32 |

mod support;

use rts_lockstep_engine::client::look_at_xlat::{LookAtGlobals, LookAtHost, LookAtTranslator};
use rts_lockstep_engine::client::selection_xlat::Disposition;
use rts_lockstep_engine::client::tactical_view::ViewLocation;
use rts_lockstep_engine::client::window_xlat::RawMessage;
use rts_lockstep_engine::common::base_type::{Coord2D, ICoord2D};
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::common::message_types::*;
use rts_lockstep_engine::differential as cpp;
use support::{ulp_distance, Check, Gen};

fn bits(f: f32) -> i32 {
    f.to_bits() as i32
}

/// The world around the translator, as the C++ stand-ins keep it, and the records of the calls.
struct Host {
    flags: i32,
    logic_frame: u32,
    client_frame: u32,
    width: u32,
    height: u32,
    locations: i32,
    angle: f32,
    pitch: f32,
    ui_scrolling: bool,
    cursor: i32,
    g: LookAtGlobals,
    stats: bool,
    out: Vec<i32>,
}

impl Host {
    fn flag(&self, bit: i32) -> bool {
        (self.flags >> bit) & 1 != 0
    }
    fn rec(&mut self, w: [i32; 8]) {
        self.out.extend(w);
    }
}

impl LookAtHost for Host {
    fn ui_input_enabled(&self) -> bool {
        self.flag(0)
    }
    fn ui_is_selecting(&self) -> bool {
        self.flag(1)
    }
    fn ui_set_scrolling(&mut self, b: bool) {
        self.ui_scrolling = b;
        self.rec([1, i32::from(b), 0, 0, 0, 0, 0, 0]);
    }
    fn ui_is_scrolling(&self) -> bool {
        self.ui_scrolling
    }
    fn ui_set_scroll_amount(&mut self, amt: Coord2D) {
        self.rec([2, bits(amt.x), bits(amt.y), 0, 0, 0, 0, 0]);
    }
    fn ui_should_move_rmb_scroll_anchor(&self) -> bool {
        self.flag(4)
    }
    fn ui_message_bookmark_set(&mut self, slot: i32) {
        self.rec([3, slot, 1, 0, 0, 0, 0, 0]);
    }
    fn mouse_cursor(&self) -> i32 {
        self.cursor
    }
    fn mouse_set_cursor(&mut self, cursor: i32) {
        self.rec([4, cursor, 0, 0, 0, 0, 0, 0]);
    }
    fn shell_active(&self) -> bool {
        self.flag(2)
    }
    fn stats_collector(&self) -> bool {
        self.stats
    }
    fn stats_start_scroll_time(&mut self) {
        self.rec([5, 0, 0, 0, 0, 0, 0, 0]);
    }
    fn stats_end_scroll_time(&mut self) {
        self.rec([6, 0, 0, 0, 0, 0, 0, 0]);
    }
    fn logic_frame(&self) -> u32 {
        self.logic_frame
    }
    fn client_frame(&self) -> u32 {
        self.client_frame
    }
    fn display_width(&self) -> u32 {
        self.width
    }
    fn display_height(&self) -> u32 {
        self.height
    }
    fn globals(&self) -> LookAtGlobals {
        self.g
    }
    fn in_single_player_game(&self) -> bool {
        self.flag(7)
    }
    fn in_skirmish_game(&self) -> bool {
        self.flag(8)
    }
    fn append_replay_camera(&mut self, l: &ViewLocation, cursor: i32, pos: ICoord2D) {
        self.rec([7, bits(l.pos.x), bits(l.pos.y), bits(l.pos.z), bits(l.angle), bits(l.pitch), bits(l.zoom), cursor]);
        self.rec([8, pos.x, pos.y, 0, 0, 0, 0, 0]);
    }
    fn view_set_mouse_lock(&mut self, b: bool) {
        self.rec([9, i32::from(b), 0, 0, 0, 0, 0, 0]);
    }
    fn view_set_angle_and_pitch_to_default(&mut self) {
        self.rec([10, 0, 0, 0, 0, 0, 0, 0]);
    }
    fn view_set_zoom_to_default(&mut self) {
        self.rec([11, 0, 0, 0, 0, 0, 0, 0]);
    }
    fn view_angle(&self) -> f32 {
        self.angle
    }
    fn view_set_angle(&mut self, a: f32) {
        self.angle = a;
        self.rec([12, bits(a), 0, 0, 0, 0, 0, 0]);
    }
    fn view_pitch(&self) -> f32 {
        self.pitch
    }
    fn view_set_pitch(&mut self, a: f32) {
        self.pitch = a;
        self.rec([13, bits(a), 0, 0, 0, 0, 0, 0]);
    }
    fn view_zoom_in(&mut self) {
        self.rec([14, 0, 0, 0, 0, 0, 0, 0]);
    }
    fn view_zoom_out(&mut self) {
        self.rec([15, 0, 0, 0, 0, 0, 0, 0]);
    }
    fn view_scroll_by(&mut self, o: &Coord2D) {
        self.rec([16, bits(o.x), bits(o.y), 0, 0, 0, 0, 0]);
    }
    fn view_get_location(&mut self) -> ViewLocation {
        let k = self.locations;
        self.locations += 1;
        let mut l = ViewLocation::default();
        l.init(k as f32 * 1.5f32, k as f32 * 2.25f32, 0.5, self.angle, self.pitch, 1.0f32 + k as f32 * 0.125f32);
        self.rec([17, 0, 0, 0, 0, 0, 0, 0]);
        l
    }
    fn view_set_location(&mut self, l: &ViewLocation) {
        self.rec([18, i32::from(l.valid), bits(l.pos.x), bits(l.pos.y), bits(l.pos.z), bits(l.angle), bits(l.pitch), bits(l.zoom)]);
    }
}

fn is_mouse_type(t: i32) -> bool {
    t >= MSG_RAW_MOUSE_BEGIN.0 as i32 && t <= MSG_RAW_MOUSE_END.0 as i32
}

/// The Rust side of a sequence (the wrapper's `ref_look_at_seq`).
fn rust_seq(steps: &[[i32; 8]]) -> Vec<i32> {
    let mut h = Host { flags: 0, logic_frame: 0, client_frame: 0, width: 800, height: 600, locations: 0, angle: 0.0, pitch: 0.0, ui_scrolling: false, cursor: 0, g: LookAtGlobals::default(), stats: false, out: Vec::new() };
    let mut x = LookAtTranslator::new();
    for w in steps {
        let mut disp = -1;
        match w[0] {
            0 => {
                let t = MessageType(w[1] as _);
                let m = RawMessage { t, pos: ICoord2D { x: w[2], y: w[3] }, delta: ICoord2D::default(), arg0: w[2], arg1: w[5] };
                let _ = is_mouse_type(w[1]);
                disp = match x.translate_game_message(&m, &mut h) {
                    Disposition::Keep => 0,
                    Disposition::Destroy => 1,
                };
            }
            1 => {
                h.flags = w[1];
                h.ui_scrolling = h.ui_scrolling && h.flag(3);
                h.g.windowed = h.flag(5);
                h.g.save_camera_in_replay = h.flag(6);
                h.stats = h.flag(9);
            }
            2 => {
                h.logic_frame = w[1] as u32;
                h.client_frame = w[2] as u32;
            }
            3 => {
                h.width = w[1] as u32;
                h.height = w[2] as u32;
            }
            4 => h.cursor = w[1],
            5 => x.reset_modes(),
            6 => x.set_current_pos(ICoord2D { x: w[1], y: w[2] }),
            7 => {
                h.g.horizontal_scroll_speed_factor = f32::from_bits(w[1] as u32);
                h.g.vertical_scroll_speed_factor = f32::from_bits(w[2] as u32);
                h.g.keyboard_scroll_factor = f32::from_bits(w[3] as u32);
            }
            8 => {
                h.angle = f32::from_bits(w[1] as u32);
                h.pitch = f32::from_bits(w[2] as u32);
            }
            _ => {}
        }
        let b = |v: bool| i32::from(v);
        h.rec([100, disp, b(x.is_scrolling), b(x.is_rotating), b(x.is_pitching), b(x.is_changing_fov), x.scroll_type, x.timestamp as i32]);
        h.rec([101, x.anchor.x, x.anchor.y, x.original_anchor.x, x.original_anchor.y, x.current_pos.x, x.current_pos.y, x.last_mouse_move_frame as i32]);
        let anchor = b(x.get_rmb_scroll_anchor().is_some());
        let moved = b(x.has_mouse_moved_recently(h.logic_frame));
        let d = x.scroll_dir;
        h.rec([102, b(d[0]), b(d[1]), b(d[2]), b(d[3]), x.prev_cursor, anchor, moved]);
    }
    h.out
}

fn cpp_seq(steps: &[[i32; 8]]) -> Vec<i32> {
    let input: Vec<i32> = steps.iter().flatten().copied().collect();
    let mut out = vec![0i32; 64 * 1024];
    let n = unsafe { cpp::ref_look_at_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!((n as usize) <= out.len(), "the C++ log overflowed its buffer");
    out.truncate(n as usize);
    out
}

/// The float words of each record tag (indices 1..8).
fn float_slots(tag: i32) -> &'static [usize] {
    match tag {
        2 | 16 => &[1, 2],
        7 => &[1, 2, 3, 4, 5, 6],
        12 | 13 => &[1],
        18 => &[2, 3, 4, 5, 6, 7],
        _ => &[],
    }
}

const ARROWS: [i32; 4] = [0xC8, 0xD0, 0xCB, 0xCD];

fn gen_message(g: &mut Gen, w: u32, hgt: u32) -> [i32; 8] {
    let pool: [MessageType; 9] = [
        MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN,
        MSG_RAW_MOUSE_RIGHT_BUTTON_UP,
        MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN,
        MSG_RAW_MOUSE_MIDDLE_BUTTON_UP,
        MSG_RAW_MOUSE_POSITION,
        MSG_RAW_MOUSE_WHEEL,
        MSG_META_OPTIONS,
        MSG_FRAME_TICK,
        MSG_RAW_MOUSE_POSITION,
    ];
    let t = match g.below(10) {
        0..=4 => pool[g.below(pool.len() as u32) as usize],
        5 => MSG_FRAME_TICK,
        6 => if g.below(2) == 0 { MSG_RAW_KEY_DOWN } else { MSG_RAW_KEY_UP },
        7 => MessageType(MSG_META_SAVE_VIEW1.0 + g.below(16) as i32),
        8 => MessageType(g.below(1200) as _),
        _ => if g.below(2) == 0 { MSG_RAW_KEY_DOWN } else { MSG_RAW_KEY_UP },
    };
    let coord = |g: &mut Gen, size: u32| -> i32 {
        match g.below(6) {
            0 => g.below(6) as i32 - 2,
            1 => size as i32 - 5 + g.below(9) as i32,
            2 => g.below(200_001) as i32 - 100_000,
            _ => g.below(size.max(1) + 1) as i32,
        }
    };
    let (a, b) = if t == MSG_RAW_KEY_DOWN || t == MSG_RAW_KEY_UP {
        let k = match g.below(4) {
            0..=1 => ARROWS[g.below(4) as usize],
            2 => g.below(256) as i32,
            _ => ARROWS[g.below(4) as usize] + 256 * (1 + g.below(4) as i32),
        };
        (k, 0)
    } else {
        (coord(g, w), coord(g, hgt))
    };
    let e = if t == MSG_RAW_KEY_DOWN || t == MSG_RAW_KEY_UP {
        match g.below(4) {
            0 => 1,
            1 => 2,
            2 => g.below(4) as i32 + 256 * g.below(3) as i32,
            _ => g.i32(),
        }
    } else {
        g.below(9) as i32 - 4
    };
    [0, t.0 as i32, a, b, 0, e, 0, 0]
}

/// A random sequence of steps.
fn gen_seq(g: &mut Gen) -> Vec<[i32; 8]> {
    let n = 1 + g.below(40) as usize;
    let (mut w, mut hgt) = (800u32, 600u32);
    let (mut lf, mut cf) = (0u32, 0u32);
    let mut steps = Vec::with_capacity(n + 2);
    // most sequences start with the input on and factors set
    if g.below(5) != 0 {
        steps.push([1, 1 | (g.below(1024) as i32 & !1 & !(1 << 1)), 0, 0, 0, 0, 0, 0]);
    }
    if g.below(5) != 0 {
        steps.push([7, bits(g.uniform(0.0, 3.0)), bits(g.uniform(0.0, 3.0)), bits(g.uniform(0.0, 1.0)), 0, 0, 0, 0]);
    }
    for _ in 0..n {
        let s = match g.below(20) {
            0..=11 => gen_message(g, w, hgt),
            12 => [1, g.below(1024) as i32 | if g.below(3) != 0 { 1 } else { 0 }, 0, 0, 0, 0, 0, 0],
            13 => {
                lf = lf.wrapping_add(g.below(60));
                cf = if g.below(8) == 0 { cf.saturating_sub(g.below(10)) } else { cf.wrapping_add(g.below(8)) };
                [2, lf as i32, cf as i32, 0, 0, 0, 0, 0]
            }
            14 => {
                (w, hgt) = if g.below(4) == 0 { (g.below(4), g.below(2000)) } else { (1 + g.below(2000), 1 + g.below(2000)) };
                [3, w as i32, hgt as i32, 0, 0, 0, 0, 0]
            }
            15 => [4, g.below(20) as i32 - 1, 0, 0, 0, 0, 0, 0],
            16 => [5, 0, 0, 0, 0, 0, 0, 0],
            17 => [6, g.below(2001) as i32 - 1000, g.below(2001) as i32 - 1000, 0, 0, 0, 0, 0],
            18 => [7, bits(g.uniform(0.0, 3.0)), bits(g.uniform(0.0, 3.0)), bits(g.uniform(0.0, 1.0)), 0, 0, 0, 0],
            _ => [8, bits(g.uniform(-10.0, 10.0)), bits(g.uniform(-1.0, 1.0)), 0, 0, 0, 0, 0],
        };
        steps.push(s);
    }
    steps
}

#[test]
fn look_at_translator_matches_upstream() {
    let cases: u64 = std::env::var("LOOK_AT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x119_1190);
    let mut whole = Check::new("LookAtTranslator sequences");
    let (mut max_ulp, mut floats, mut scrolls, mut destroyed, mut replays, mut steps_n) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let steps = gen_seq(&mut g);
        steps_n += steps.len() as u64;
        let r = rust_seq(&steps);
        let c = cpp_seq(&steps);
        for rec in c.chunks(8) {
            match rec[0] {
                16 => scrolls += 1,
                7 => replays += 1,
                100 if rec[1] == 1 => destroyed += 1,
                _ => {}
            }
        }
        // the floats of records that line up
        for (a, b) in r.chunks(8).zip(c.chunks(8)) {
            if a.len() == 8 && b.len() == 8 && a[0] == b[0] {
                for &k in float_slots(a[0]) {
                    floats += 1;
                    let (x, y) = (f32::from_bits(a[k] as u32), f32::from_bits(b[k] as u32));
                    if !x.is_nan() && !y.is_nan() {
                        max_ulp = max_ulp.max(ulp_distance(x, y));
                    }
                }
            }
        }
        if r != c {
            // the first record that differs, with the step it came from
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len())) / 8 * 8;
            let step = c[..k.min(c.len())].chunks(8).filter(|x| x[0] == 102).count();
            println!("sequence {i}: step {step} {:?}: rust {:?} c++ {:?}", steps.get(step), &r[k.min(r.len())..(k + 24).min(r.len())], &c[k.min(c.len())..(k + 24).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps_n} steps: {scrolls} scrollBy calls, {replays} replay cameras, {destroyed} messages destroyed; {floats} float words compared, max ULP {max_ulp}");
    if planted {
        whole.report();
    } else {
        whole.finish();
        assert_eq!(max_ulp, 0, "a float differs");
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}

