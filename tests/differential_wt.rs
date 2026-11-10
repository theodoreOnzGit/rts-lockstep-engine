// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of the window transitions and the animate window manager (#77:
//! `client::window_transitions`, `client::animate_window`) against upstream's own GameWindowTransitions.cpp,
//! GameWindowTransitionsStyles.cpp, AnimateWindowManager.cpp and ProcessAnimateWindow.cpp (commit 0a05454d), copied
//! verbatim by `build.rs` into a program, `wt_ref`, with the window manager's and the gadgets' files and
//! `differential/wrap_wt.cpp` (which is `wrap_wm.cpp` again, and lists every stand-in).
//!
//! # Method
//!
//! Each case is a random window tree, a random synthetic `WindowTransitions.ini`, and a random sequence of
//! operations, made as the Rust runs it (so that the targets are windows that exist). The tree: two to seven windows
//! (user windows, push buttons, static texts, some hidden, some children), most of them named from a small pool (so
//! that several windows share a name, and the grow windows `<name>Medium` / `<name>Small`,
//! `MainMenu.wnd:WinGrowMarker` and `ControlBar.wnd:ButtonGeneral` sometimes exist), images set into the enabled and
//! disabled draw data 0, 5 and 6 (`Img*` names, 5x10, `*Mid` 7x10), static texts given numbers or words, some
//! centred. The INI: one to five groups (`FireOnce` yes, no or absent), each of one to four windows of a random
//! style (the fifteen, in any case), a name from the pool or one no window has, and a frame delay of 0 to 6 or none;
//! now and then a name repeated (in another case: `getNewGroup` gives `NULL`) or a second load. `COUNTUP` names only
//! static texts (on another window its `GGM_SET_LABEL` passes a pointer the C++'s test system prints). The operations:
//! `setGroup` (immediate or not; a group, an unknown name, or the empty one), `reverse`, `remove` (skipping the
//! pending one or not), the manager's `update` (most often: `processDestroyList` and the handler's update), the
//! handler's `draw`, a frame (the update, then the draw), `isFinished`, `reset`; hiding and destroying windows, new texts and images, the control bar's
//! arrow image set or cleared; and the animate manager's `registerGameWindow` (each of the eight types, needing to
//! finish or not, 0 to 1000 ms, 0 to 300 ms of delay), `reverseAnimateWindow`, `update` (1 to 25 times in a row),
//! `resetToRestPosition`,
//! `reset`, with the clock moved on by 0 to 120 ms.
//!
//! The Rust runs the case first; where it meets upstream's undefined behaviour ([`GameWindowManager::undefined`])
//! the case stops before that operation and the C++ runs only the ones before it (the reasons are counted). After
//! each operation both write the messages and sounds the host recorded, the draws (`TheDisplay`'s rectangles and
//! images with their colours and clip region, the typed text's string, place, colours, wrap, centring, font and clip),
//! and a dump: every window (status, position, size, family, text and text colours, its enabled and disabled image
//! 0, a static text's text and centring), the manager, the handler (current, pending and drawn groups, finished), each
//! group (fire-once, direction, frame) and window (name, delays, style, window, and its transition's frame length,
//! finished, forward, window), and the animate manager (needs update, reversed, empty) with every animation (window,
//! type, current, start, end and rest positions, the velocity's bits, the times, the delay, finished). They must be
//! identical.
//!
//! The floats: the styles' alphas (`ScreenFade`, `FullFade`, `ControlBarArrow`) and the slides' velocities are
//! `float` products, compiled to SSE (`-msse2 -mfpmath=sse -ffp-contract=off`) as Rust's `f32` is; the spiral's `cos`
//! and `sin` of a float are petir's on both sides (`wrap_wt.cpp` defines `cosf` / `sinf` as the `libm` crate's, and
//! the float overloads MSVC's `<math.h>` has, which g++'s glibc header lacks).
//!
//! # Predictions (written before the first run)
//!
//! The first run of the harness: a few mismatches from the harness itself (the draw log's formats, the static text's
//! font in the typed text). Then in the translation: 2 to 4 causes, most likely in `ButtonFlashTransition`'s backward
//! draw-state table, `TextTypeTransition`'s frame-length quirks, the handler's `update` order with fire-once and
//! reversed groups, and the slides' stop conditions. The floats: 0 bit mismatches (products and a petir sine on both
//! sides). Undefined (skipped): `reverse` or `remove` of an unknown group with none current (often), `WINFADE` frames
//! without a window, the medium / small scale without a window, a transition or an animation holding a destroyed
//! window, `drawTypeText` on a window that is not a static text, `MAINMENUSCALEUP` drawn without its grow window.
//!
//! # Results (2026-11-10, `WT_CASES=2000 cargo test --release --features differential --test differential_wt`)
//!
//! - First run (50 cases): 46 cases differed, all from the harness: a `TransitionGroup`'s `m_directionMultiplier`
//!   (set only by `init`) read as heap garbage in the C++. The shim's `NEW` zero-fills, as the game's allocator does,
//!   but g++'s `-flifetime-dse` removes that memset (a store before the constructor starts); `wt_ref` is built with
//!   `-fno-lifetime-dse` (valgrind found it). Not predicted (an issue drafted: the other differential builds).
//! - Then 200 cases: **1 mismatch, in the translation**: `registerGameWindow` calls `setMaxDuration(ms)` on the
//!   registered type's process only (the base class's is empty); the port set the timed slide's duration at every
//!   registration, so a later slide of another type changed an earlier timed slide's speed. Fixed. Predicted 2 to 4
//!   causes in the transitions; the one cause was in the animate manager.
//! - Then **2000 cases, 93,488 operations compared, 0 mismatches**; 4,803 filled and 3,109 open rectangles, 1,302
//!   images and 1,015 typed texts drawn and compared; the floats (the alphas, the velocities' bits, the spiral's petir
//!   sine and cosine) **0 bit mismatches** (predicted 0: held).
//! - Stopped where upstream is undefined (1,369 cases, at some operation): `TextTypeTransition` without a window 247,
//!   `MainMenuScaleUpTransition` 194 (its draw without the grow window, its init without a window),
//!   `MainMenuMediumScaleUpTransition::reverse` without a grow window 179, the handler's `reverse` / `remove` of an
//!   unknown group 167, the medium / small scale's init without a window 165, a field into the duplicate name's `NULL`
//!   group 97, `CountUpTransition` without a window 82, `TextOnFrameTransition` 69, a freed window held by a transition
//!   53 or an animation 48 (a float past `Int` 4 of them), `FadeTransition` 43, `FullFadeTransition` 18,
//!   `PushButtonImageDrawThree` without a window 7. Predicted list: held, with more kinds than listed.
//! - Planted bugs (`L9_PLANT`, 2000 cases each; predictions written before the first plant run): 4711 (`BUTTONFLASH`'s
//!   backward frame 4 draws frame 3's state) predicted 20 to 60, **0** on the first run: the cases rarely drew the
//!   right frame; the generator then got a `frame` operation (an update and a draw, as a game frame) and caught **5**,
//!   final **4** (missed, low); 4712 (a finished fire-once group not dropped) predicted 300 to 600, **36**, then
//!   **25**, final **27** (missed: few fire-once groups finish, most hold a window that is missing or a style that never finishes);
//!   4713 (the slides' slow-down threshold one pixel nearer) predicted 10 to 40, **1**, then **0** with the `frame`
//!   operation (missed: a slide needs some 20 updates to reach its threshold); `aupd` was given a count (1 to 25
//!   updates in a row) and caught **61**; 4714 (`ScreenFade`'s step 1/30 for 1/29) predicted 150 to 400, **148**,
//!   **203**, final **168** (held at the last two runs, just under on the first). The main run after each generator
//!   change: 0 mismatches.

#![cfg(feature = "differential")]

use rts_lockstep_engine::client::animate_window::AnimateWindowManager;
use rts_lockstep_engine::client::gadget::static_text_set_text;
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::*;
use rts_lockstep_engine::client::window_transitions as wt;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn range(&mut self, a: i32, b: i32) -> i32 {
        a + self.below((b - a + 1) as u64) as i32
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len() as u64) as usize]
    }
}

/// The host: wrap_wm.cpp's `TestSystem` / `TestInput` and stand-ins (and wrap_wt.cpp's), recording what they record.
#[derive(Default)]
struct Host {
    log: String,
    keys: BTreeMap<Vec<u8>, i32>,
    arrow: Option<Vec<u8>>,
    now: u32,
    awm: AnimateWindowManager,
}

impl GuiHost for Host {
    fn system(&mut self, _wm: &mut GameWindowManager, _func: &[u8], win: WinId, msg: u32, d1: u32, _d2: u32) -> MsgHandled {
        let _ = writeln!(self.log, "sys {win} {msg} {d1}");
        MsgHandled::Handled
    }
    fn input(&mut self, _wm: &mut GameWindowManager, _func: &[u8], win: WinId, msg: u32, d1: u32, _d2: u32) -> MsgHandled {
        if msg != gwm::MOUSE_POS {
            let _ = writeln!(self.log, "inp {win} {msg} {d1}");
        }
        MsgHandled::Ignored
    }
    fn add_audio_event(&mut self, name: &[u8]) {
        let _ = writeln!(self.log, "audio {}", String::from_utf8_lossy(name));
    }
    fn font_height(&self, font: Option<&GameFont>) -> i32 {
        font.map_or(12, |f| f.point_size)
    }
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        Some(if image.ends_with(b"Mid") { (7, 10) } else { (5, 10) })
    }
    fn has_image(&self, image: &[u8]) -> bool {
        image.len() >= 3 && image[..3].eq_ignore_ascii_case(b"img") || image == b"Gradient"
    }
    fn control_bar_arrow_image(&self) -> Option<Vec<u8>> {
        self.arrow.clone()
    }
}

fn text(o: &mut String, t: &[u16]) {
    o.push_str(" [");
    for (i, c) in t.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(o, "{c}");
    }
    o.push(']');
}

fn idx(w: Option<WinId>) -> i64 {
    w.map_or(-1, |w| w as i64)
}

fn draw_data(o: &mut String, set: &DrawDataSet) {
    for d in set {
        let name = d.image.as_ref().map_or("-".to_string(), |n| String::from_utf8_lossy(n).into_owned());
        let _ = write!(o, " {name}/{}/{}", d.color, d.border_color);
    }
}

fn clip(o: &mut String, c: Option<(i32, i32, i32, i32)>) {
    match c {
        Some((a, b, x, y)) => {
            let _ = write!(o, " clip {a} {b} {x} {y}");
        }
        None => o.push_str(" clip -"),
    }
}

/// wrap_wt.cpp's draw log.
fn draws(o: &mut String, items: &[DrawItem]) {
    for d in items {
        match d {
            DrawItem::FillRect { x, y, w, h, color } => {
                let _ = writeln!(o, "fill {x} {y} {w} {h} {}", *color as u32);
            }
            DrawItem::OpenRect { x, y, w, h, color } => {
                let _ = writeln!(o, "open {x} {y} {w} {h} 1 {}", *color as u32);
            }
            DrawItem::Image { image, x0, y0, x1, y1, color, clip: c, .. } => {
                let _ = write!(o, "img {} {x0} {y0} {x1} {y1} {color} 0", String::from_utf8_lossy(image));
                clip(o, *c);
                o.push('\n');
            }
            DrawItem::Text { text: t, x, y, color, drop_color, font, word_wrap, wrap_centered, clip: c, .. } => {
                o.push_str("text");
                text(o, t);
                let _ = write!(o, " {x} {y} {} {} wrap {} cen {} font {}", *color as u32, *drop_color as u32, word_wrap.unwrap_or(0), u8::from(*wrap_centered), font.as_ref().map_or(-1, |f| f.point_size));
                clip(o, *c);
                o.push('\n');
            }
            other => panic!("a draw the transitions never make: {other:?}"),
        }
    }
}

/// The dump, in wrap_wm.cpp's and wrap_wt.cpp's format; the draw data of the windows from `dd_from` on.
fn dump(wm: &GameWindowManager, h: &Host, dd_from: usize) -> String {
    let mut o = String::new();
    for (i, w) in wm.tree.windows.iter().enumerate() {
        if wm.ext[i].freed {
            let _ = writeln!(o, "w{i} freed");
            continue;
        }
        let _ = write!(o, "w{i} st {} state {} style {} at {} {} size {} {} parent {} owner {} kids", w.status, w.inst.state, w.inst.style, w.region.0, w.region.1, w.size.0, w.size.1, idx(w.parent), idx(wm.ext[i].owner));
        for c in &w.children {
            let _ = write!(o, " {c}");
        }
        text(&mut o, wm.win_get_text(i));
        let t = &w.inst;
        let _ = writeln!(o, " tx {}/{} {}/{} {}/{}", t.enabled_text.color, t.enabled_text.border_color, t.disabled_text.color, t.disabled_text.border_color, t.hilite_text.color, t.hilite_text.border_color);
        if i >= dd_from {
            o.push_str(" dd");
            draw_data(&mut o, &t.enabled_draw_data);
            draw_data(&mut o, &t.disabled_draw_data);
            draw_data(&mut o, &t.hilite_draw_data);
            o.push('\n');
        }
        assert!(matches!(wm.ext[i].user, UserData::None | UserData::PushButton(_) | UserData::StaticText(..)), "a gadget wrap_wm.cpp dumps more of");
    }
    let _ = write!(o, "mgr focus {} cap {} grab {} rgn {} lone {} modal", idx(wm.keyboard_focus), idx(wm.mouse_captor), idx(wm.grab_window), idx(wm.curr_mouse_rgn), idx(wm.lone_window));
    for m in &wm.modal {
        let _ = write!(o, " {m}");
    }
    o.push_str(" list");
    for w in &wm.window_list {
        let _ = write!(o, " {w}");
    }
    o.push_str(" tip");
    text(&mut o, if wm.mouse.is_empty { &[] } else { &wm.mouse.display_string });
    let _ = writeln!(o, " ime {}", idx(wm.ime.get_window()));
    // wrap_wt.cpp's
    for (i, w) in wm.tree.windows.iter().enumerate() {
        if wm.ext[i].freed {
            continue;
        }
        let name = |n: &Option<Vec<u8>>| n.as_ref().map_or("-".to_string(), |n| String::from_utf8_lossy(n).into_owned());
        let _ = write!(o, "i{i} {} {}", name(&w.inst.enabled_draw_data[0].image), name(&w.inst.disabled_draw_data[0].image));
        if let UserData::StaticText(td, t) = &wm.ext[i].user {
            if w.inst.style & gws::STATIC_TEXT != 0 {
                o.push_str(" st");
                text(&mut o, t);
                let _ = write!(o, " {}", u8::from(td.centered));
            }
        }
        o.push('\n');
    }
    let th = &wm.transitions;
    let g = |x: Option<usize>| x.map_or(-1, |x| x as i64);
    let _ = writeln!(o, "th cur {} pend {} draw {} sec {} fin {}", g(th.current), g(th.pending), g(th.draw_group), g(th.secondary_draw_group), u8::from(th.is_finished()));
    for grp in &th.groups {
        let _ = writeln!(o, "tg {} once {} dir {} frame {}", String::from_utf8_lossy(&grp.name), u8::from(grp.fire_once), grp.direction_multiplier, grp.current_frame);
        for t in &grp.windows {
            let _ = write!(o, " tw {} delay {} cur {} style {} win {}", String::from_utf8_lossy(&t.win_name), t.frame_delay, t.current_frame_delay, t.style, idx(t.win));
            if let Some(x) = &t.transition {
                let _ = write!(o, " len {} fin {} fwd {} xw {}", x.frame_length, u8::from(x.is_finished), u8::from(x.is_forward), idx(x.win));
            }
            o.push('\n');
        }
    }
    let a = &h.awm;
    let _ = writeln!(o, "aw nu {} rev {} empty {}", u8::from(a.needs_update), u8::from(a.reverse), u8::from(a.is_empty()));
    for (list, l) in [("must", &a.must_finish_list), ("plain", &a.win_list)] {
        for e in l {
            let _ = writeln!(
                o,
                "ae {list} win {} type {} cur {} {} start {} {} end {} {} rest {} {} vel {} {} t {} {} delay {} fin {}",
                idx(e.win),
                e.anim_type,
                e.cur_pos.0,
                e.cur_pos.1,
                e.start_pos.0,
                e.start_pos.1,
                e.end_pos.0,
                e.end_pos.1,
                e.rest_pos.0,
                e.rest_pos.1,
                e.vel.0.to_bits(),
                e.vel.1.to_bits(),
                e.start_time,
                e.end_time,
                e.delay,
                u8::from(e.is_finished)
            );
        }
    }
    o
}

fn units(t: &str) -> Vec<u16> {
    t.chars().filter(|&c| c != '_').map(|c| c as u16).collect()
}

fn run_op(wm: &mut GameWindowManager, h: &mut Host, op: &str) {
    let mut it = op.split(' ');
    let c = it.next().expect("op");
    let mut v: Vec<&str> = it.collect();
    let n = |v: &mut Vec<&str>| -> i32 { v.remove(0).parse().expect("number") };
    let w = |i: i32| -> Option<WinId> { (i >= 0).then_some(i as usize) };
    let name = |s: &str| -> Vec<u8> { if s == "_" { Vec::new() } else { s.as_bytes().to_vec() } };
    match c {
        "W" => {
            let parent = w(n(&mut v));
            let ty = n(&mut v);
            let (x, y, ww, hh) = (n(&mut v), n(&mut v), n(&mut v), n(&mut v));
            let status = n(&mut v) as u32;
            let style = n(&mut v) as u32;
            let visual = n(&mut v) != 0;
            let inst = WinInstanceData { style, ..WinInstanceData::new() };
            match ty {
                0 => {
                    let u = wm.win_create(h, parent, status, x, y, ww, hh, SystemFunc::Host(b"TestSystem".to_vec()), Some(inst));
                    wm.ext[u].input = InputFunc::Host(b"TestInput".to_vec());
                    wm.ext[u].owner = parent;
                }
                1 => {
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::None, visual);
                }
                11 => {
                    wm.gogo_gadget_look(h, parent, status, x, y, ww, hh, inst, GadgetData::Text(TextData::default()), visual);
                }
                _ => unreachable!(),
            }
        }
        "t" => {
            let t = n(&mut v) as u32;
            wm.time_ms = t;
            h.now = t;
        }
        "H" => {
            let x = n(&mut v) as usize;
            wm.win_hide(x, n(&mut v) != 0);
        }
        "Z" => {
            let x = w(n(&mut v));
            wm.win_destroy(x);
            wm.update(h);
        }
        "!" => {
            let word = v.remove(0);
            match word {
                "name" => {
                    let i = n(&mut v) as usize;
                    let s = v.remove(0).as_bytes().to_vec();
                    let k = h.keys.len() as i32 + 1;
                    let key = *h.keys.entry(s.clone()).or_insert(k);
                    wm.tree.windows[i].inst.decorated_name_string = s;
                    wm.tree.windows[i].id = key;
                }
                "img" => {
                    let (i, slot, k) = (n(&mut v) as usize, n(&mut v), n(&mut v) as usize);
                    let s = v.remove(0);
                    let im = (s != "-" && h.has_image(s.as_bytes())).then(|| s.as_bytes().to_vec());
                    let inst = &mut wm.tree.windows[i].inst;
                    if slot == 0 {
                        inst.enabled_draw_data[k].image = im;
                    } else {
                        inst.disabled_draw_data[k].image = im;
                    }
                }
                "stxt" => {
                    let i = n(&mut v) as usize;
                    let t = units(v.remove(0));
                    static_text_set_text(wm, h, Some(i), &t);
                }
                "ctr" => {
                    let i = n(&mut v) as usize;
                    let b = n(&mut v) != 0;
                    if let UserData::StaticText(td, _) = &mut wm.ext[i].user {
                        td.centered = b;
                    }
                }
                "ini" => {
                    let t: String = v.join(" ").replace('|', "\n");
                    wt::load_window_transitions(wm, wt::WINDOW_TRANSITIONS_INI, t.as_bytes()).expect("the synthetic INI reads");
                }
                "set" => {
                    let s = name(v.remove(0));
                    let imm = n(&mut v) != 0;
                    wt::set_group(wm, h, &s, imm);
                }
                "rev" => {
                    let s = name(v.remove(0));
                    wt::reverse(wm, h, &s);
                }
                "rem" => {
                    let s = name(v.remove(0));
                    let skip = n(&mut v) != 0;
                    wt::remove(wm, h, &s, skip);
                }
                "upd" => wm.update(h),
                "draw" | "frame" => {
                    if word == "frame" {
                        wm.update(h);
                    }
                    let mut out = Vec::new();
                    wt::draw(wm, h, &mut out);
                    let mut s = String::new();
                    draws(&mut s, &out);
                    h.log.push_str(&s);
                }
                "fin" => {
                    let f = wt::is_finished(wm);
                    let _ = writeln!(h.log, "fin {}", u8::from(f));
                }
                "reset" => wm.transitions.reset(),
                "arrow" => {
                    let s = v.remove(0);
                    h.arrow = (s != "-" && h.has_image(s.as_bytes())).then(|| s.as_bytes().to_vec());
                }
                "areg" => {
                    let (i, ty, needs, ms, delay) = (n(&mut v), n(&mut v), n(&mut v), n(&mut v), n(&mut v));
                    let now = h.now;
                    let mut a = std::mem::take(&mut h.awm);
                    a.register_game_window(wm, w(i), ty, needs != 0, ms as u32, delay as u32, now);
                    h.awm = a;
                }
                "arev" => {
                    let now = h.now;
                    h.awm.reverse_animate_window(wm, now);
                }
                "aupd" => {
                    let now = h.now;
                    for _ in 0..n(&mut v) {
                        h.awm.update(wm, now);
                        if wm.undefined.is_some() {
                            break;
                        }
                    }
                }
                "arest" => h.awm.reset_to_rest_position(wm),
                "areset" => h.awm.reset(wm),
                "afin" => {
                    let _ = writeln!(h.log, "afin {} {}", u8::from(h.awm.is_finished()), u8::from(h.awm.is_reversed()));
                }
                other => panic!("unknown operation {other}"),
            }
        }
        other => panic!("unknown operation {other}"),
    }
}

/// The names windows get: the pool for user windows and buttons, and for static texts.
const NAMES: &[&str] = &["A", "B", "C", "AMedium", "ASmall", "BMedium", "BSmall", "MainMenu.wnd:WinGrowMarker", "ControlBar.wnd:ButtonGeneral"];
const TEXT_NAMES: &[&str] = &["S0", "S1", "S2"];
const STYLES: &[&str] = &[
    "FLASH", "BUTTONFLASH", "WINFADE", "WINSCALEUP", "MAINMENUSCALEUP", "TYPETEXT", "SCREENFADE", "COUNTUP", "FULLFADE", "TEXTONFRAME", "MAINMENUMEDIUMSCALEUP",
    "MAINMENUSMALLSCALEDOWN", "CONTROLBARARROW", "SCORESCALEUP", "REVERSESOUND",
];
const GROUPS: &[&str] = &["G0", "G1", "G2", "G3", "G4"];
const IMAGES: &[&str] = &["ImgL", "ImgMid", "ImgR", "ImgX", "-", "Nope"];
const TEXTS: &[&str] = &["5", "12", "0", "-3", "250", "4000", "29", "99999", "HELLO_WORLD", "_", "AB", "A_LONG_TEXT_THAT_IS_LONGER_THAN_THIRTY_UNITS", "7X"];

/// The window tree and the INI: the case's first operations.
fn gen_setup(r: &mut Rng) -> Vec<String> {
    let mut ops = Vec::new();
    let count = r.range(2, 7);
    let mut texts = Vec::new();
    for i in 0..count {
        let ty = *r.pick(&[0, 0, 1, 11, 11]);
        let parent = if i > 0 && r.chance(1, 3) { r.range(0, i - 1) } else { -1 };
        let status = 8 | if r.chance(1, 5) { 16 } else { 0 } | if r.chance(1, 6) { win_status::WRAP_CENTERED } else { 0 };
        let style = match ty {
            1 => gws::PUSH_BUTTON,
            11 => gws::STATIC_TEXT,
            _ => gws::USER_WINDOW,
        };
        let (x, y) = (r.range(-20, 700), r.range(-20, 500));
        let (ww, hh) = (r.range(0, 300), r.range(0, 200));
        ops.push(format!("W {parent} {ty} {x} {y} {ww} {hh} {status} {style} 0"));
        if ty == 11 {
            texts.push(i);
        }
    }
    let mut named: Vec<&str> = Vec::new();
    let mut text_named: Vec<&str> = Vec::new();
    for i in 0..count {
        let is_text = texts.contains(&i);
        if r.chance(5, 6) {
            let n = if is_text { *r.pick(TEXT_NAMES) } else { *r.pick(NAMES) };
            ops.push(format!("! name {i} {n}"));
            if is_text {
                text_named.push(n);
            } else {
                named.push(n);
            }
        }
        for _ in 0..r.range(0, 3) {
            let k = *r.pick(&[0, 0, 5, 6]);
            ops.push(format!("! img {i} {} {k} {}", r.below(2), r.pick(IMAGES)));
        }
        if is_text {
            ops.push(format!("! stxt {i} {}", r.pick(TEXTS)));
            if r.chance(1, 3) {
                ops.push(format!("! ctr {i} 1"));
            }
        }
    }
    ops.push(gen_ini(r, &named, &text_named));
    ops
}

/// A synthetic `WindowTransitions.ini` on one line (`|` for the line ends).
fn gen_ini(r: &mut Rng, named: &[&str], text_named: &[&str]) -> String {
    let mut s = String::from("! ini ; synthetic|");
    let groups = r.range(1, 5) as usize;
    for g in 0..groups {
        let mut name = GROUPS[g].to_string();
        if r.chance(1, 25) {
            name = GROUPS[r.below(g as u64 + 1) as usize].to_lowercase();
        }
        s += &format!("WindowTransition {name}|");
        match r.below(3) {
            0 => s += "  FireOnce = Yes|",
            1 => s += "  FireOnce = No|",
            _ => {}
        }
        for _ in 0..r.range(1, 4) {
            let style = if r.chance(1, 6) { "TYPETEXT" } else { *r.pick(STYLES) };
            // mostly a name a window has (a static text's for the text styles), now and then any of the pools' or one
            // no window has
            let texty = matches!(style, "COUNTUP" | "TYPETEXT");
            let win = if style == "COUNTUP" {
                if text_named.is_empty() || r.chance(1, 10) { *r.pick(TEXT_NAMES) } else { *r.pick(text_named) }
            } else if r.chance(1, 20) {
                "Nowhere"
            } else if r.chance(1, 10) {
                *r.pick(NAMES)
            } else if texty && !text_named.is_empty() || named.is_empty() && !text_named.is_empty() {
                *r.pick(text_named)
            } else if !named.is_empty() {
                *r.pick(named)
            } else {
                *r.pick(NAMES)
            };
            s += "  Window|";
            if r.chance(15, 16) {
                s += &format!("    WinName = {win}|");
            }
            let st = if r.chance(1, 6) { style.to_lowercase() } else { style.to_string() };
            if r.chance(15, 16) {
                s += &format!("    Style = {st}|");
            }
            if r.chance(5, 6) {
                s += &format!("    FrameDelay = {}|", r.range(0, 6));
            }
            s += "  End|";
        }
        s += "End|";
    }
    s
}

fn live(wm: &GameWindowManager) -> Vec<WinId> {
    (0..wm.tree.windows.len()).filter(|&i| !wm.ext[i].freed && wm.tree.windows[i].status & win_status::DESTROYED == 0).collect()
}

/// A group's name for `setGroup` / `reverse` / `remove`: mostly one the INI made (in its case or lower case), now and
/// then one it did not, or the empty name.
fn group_name(r: &mut Rng, wm: &GameWindowManager) -> String {
    let made: Vec<String> = wm.transitions.groups.iter().map(|g| String::from_utf8_lossy(&g.name).into_owned()).collect();
    match r.below(40) {
        0 => "Unknown".into(),
        1 => "_".into(),
        2 => r.pick(GROUPS).to_string(),
        3..=6 if !made.is_empty() => r.pick(&made).to_lowercase(),
        _ if !made.is_empty() => r.pick(&made).clone(),
        _ => r.pick(GROUPS).to_string(),
    }
}

fn gen_op(r: &mut Rng, wm: &GameWindowManager, h: &Host) -> String {
    let wins = live(wm);
    let any = |r: &mut Rng| -> usize { *r.pick(&wins) };
    match r.below(100) {
        0..=14 => "! upd".into(),
        15..=39 => "! frame".into(),
        40..=46 => "! draw".into(),
        47..=54 => format!("! set {} {}", group_name(r, wm), u8::from(r.chance(1, 4))),
        55..=59 => format!("! rev {}", group_name(r, wm)),
        60..=62 => format!("! rem {} {}", group_name(r, wm), r.below(2)),
        63..=64 => "! fin".into(),
        65 => "! reset".into(),
        66..=68 if !wins.is_empty() => format!("H {} {}", any(r), r.below(2)),
        69 if !wins.is_empty() && r.chance(1, 2) => format!("Z {}", any(r)),
        70..=71 => format!("! arrow {}", r.pick(IMAGES)),
        72..=73 if !wins.is_empty() => {
            let i = any(r);
            if matches!(wm.ext[i].user, UserData::StaticText(..)) && r.chance(1, 2) {
                return format!("! stxt {i} {}", r.pick(TEXTS));
            }
            format!("! img {i} {} {} {}", r.below(2), *r.pick(&[0, 0, 5, 6]), r.pick(IMAGES))
        }
        74..=78 if !wins.is_empty() => {
            let ty = r.range(1, 8);
            let ms = *r.pick(&[0, 500, 500, 1000, 250]);
            format!("! areg {} {ty} {} {ms} {}", any(r), r.below(2), *r.pick(&[0, 0, 100, 300]))
        }
        79..=80 => "! arev".into(),
        81..=88 => format!("! aupd {}", *r.pick(&[1, 1, 2, 5, 10, 25])),
        89 => "! arest".into(),
        90 => "! areset".into(),
        91 => "! afin".into(),
        _ => format!("t {}", h.now + r.below(121) as u32),
    }
}

struct Case {
    ops: Vec<String>,
    dumps: Vec<String>,
    undefined: Option<&'static str>,
    undefined_op: String,
}

fn run_rust(r: &mut Rng, len: u64) -> Case {
    let mut wm = GameWindowManager::new((800, 600));
    let mut h = Host { awm: AnimateWindowManager::new(800), ..Default::default() };
    let mut ops = gen_setup(r);
    let mut dumps = Vec::new();
    let mut i = 0;
    let total = ops.len() as u64 + len;
    while (i as u64) < total {
        if i >= ops.len() {
            let op = gen_op(r, &wm, &h);
            ops.push(op);
        }
        let before = wm.tree.windows.len();
        run_op(&mut wm, &mut h, &ops[i]);
        let dd_from = if ops[i].starts_with("W ") { before } else { usize::MAX };
        if let Some(u) = wm.undefined {
            let at = ops[i].split(' ').take(2).collect::<Vec<_>>().join(" ");
            ops.truncate(i);
            return Case { ops, dumps, undefined: Some(u), undefined_op: at };
        }
        dumps.push(format!("#{i}\n{}{}", std::mem::take(&mut h.log), dump(&wm, &h, dd_from)));
        i += 1;
    }
    Case { ops, dumps, undefined: None, undefined_op: String::new() }
}

fn run_cpp(ops: &[String]) -> (String, Option<i32>) {
    let mut child = std::process::Command::new(env!("WT_REF_EXE")).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().expect("run wt_ref");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        for op in ops {
            let _ = writeln!(stdin, "{op}");
        }
    }
    let mut stdout = child.stdout.take().expect("stdout");
    let reader = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut v);
        v
    });
    let start = std::time::Instant::now();
    let code = loop {
        if let Some(st) = child.try_wait().expect("wait wt_ref") {
            break st.code();
        }
        if start.elapsed() > std::time::Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    (String::from_utf8_lossy(&reader.join().expect("reader")).into_owned(), code)
}

#[test]
fn window_transitions_and_animate_windows_match_upstream() {
    let cases: u64 = std::env::var("WT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(2000);
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut ops_compared, mut equal_cases, mut mismatched_cases, mut crashed) = (0u64, 0u64, 0u64, 0u64);
    let mut undefined: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_op: BTreeMap<String, u64> = BTreeMap::new();
    let mut styles_drawn: BTreeMap<String, u64> = BTreeMap::new();
    let mut first_bad: Option<String> = None;
    for case in 0..cases {
        let len = 20 + r.below(81);
        let c = run_rust(&mut r, len);
        if let Some(u) = c.undefined {
            *undefined.entry(format!("{u} (at `{}`)", c.undefined_op)).or_default() += 1;
        }
        let (out, code) = run_cpp(&c.ops);
        let mut cpp: Vec<String> = Vec::new();
        for part in out.split_inclusive('\n') {
            if part.starts_with('#') {
                cpp.push(String::new());
            }
            if let Some(last) = cpp.last_mut() {
                last.push_str(part);
            }
        }
        let mut bad = None;
        for (i, d) in c.dumps.iter().enumerate() {
            match cpp.get(i) {
                Some(x) if x == d => {
                    ops_compared += 1;
                    let mut t = c.ops[i].split(' ');
                    let k = t.next().unwrap_or("").to_string();
                    let k = if k == "!" { t.next().unwrap_or("").to_string() } else { k };
                    *by_op.entry(k).or_default() += 1;
                    for line in d.lines() {
                        for kind in ["fill", "open", "img", "text"] {
                            if line.starts_with(kind) {
                                *styles_drawn.entry(kind.to_string()).or_default() += 1;
                            }
                        }
                    }
                }
                Some(x) => {
                    bad = Some(format!("case {case}, op {i} `{}`:\n--- rust\n{d}--- c++\n{x}", c.ops[i]));
                    break;
                }
                None => {
                    crashed += 1;
                    bad = Some(format!("case {case}, op {i} `{}`: the C++ stopped (exit {code:?}); rust:\n{d}", c.ops[i]));
                    break;
                }
            }
        }
        match bad {
            None => equal_cases += 1,
            Some(b) => {
                mismatched_cases += 1;
                if first_bad.is_none() {
                    let ops: String = c.ops.iter().map(|o| format!("{o}\n")).collect();
                    first_bad = Some(format!("{b}\nops:\n{ops}"));
                }
            }
        }
    }
    eprintln!("wt differential: {cases} cases, {ops_compared} operations compared, {equal_cases} cases identical, {mismatched_cases} with a mismatch ({crashed} where the C++ stopped)");
    eprintln!("  compared, by operation: {by_op:?}");
    eprintln!("  draws compared, by kind: {styles_drawn:?}");
    for (u, n) in &undefined {
        eprintln!("  stopped where upstream is undefined: {n} x {u}");
    }
    if let Some(b) = &first_bad {
        eprintln!("first mismatch: {b}");
    }
    assert_eq!(mismatched_cases, 0, "{}", first_bad.unwrap_or_default());
}
