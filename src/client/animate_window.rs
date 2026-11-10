// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/AnimateWindowManager.cpp (AnimateWindow, setAnimData,
//     clearWinList, AnimateWindowManager: init, reset, update, registerGameWindow, getProcessAnimate,
//     reverseAnimateWindow, resetToRestPosition)
//   GeneralsMD/Code/GameEngine/Include/GameClient/AnimateWindowManager.h (AnimTypes, the inline isFinished,
//     isReversed, isEmpty)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ProcessAnimateWindow.cpp (ProcessAnimateWindowSlideFromRight,
//     SlideFromLeft, SlideFromTop, SlideFromBottom, SlideFromBottomTimed, Spiral, SlideFromTopFast,
//     SlideFromRightFast: their constructors, initAnimateWindow, initReverseAnimateWindow, updateAnimateWindow,
//     reverseAnimateWindow, setMaxDuration)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The animate window manager (GitHub #77): the shell's `m_animateWindowManager`, which slides or spirals windows
//! into place, and back out (`Shell::registerWithAnimateManager`, `reverseAnimatewindow`, `isAnimFinished`).
//!
//! # As upstream reads it
//!
//! `registerGameWindow(win, type, needsToFinish, ms, delayMs)` makes an `AnimateWindow`: the type's process takes the
//! window's position as its **rest** position, moves it off screen (the display's width away to the right, left, top
//! or bottom; the fast slides from just off the edge; the spiral half the display's width out), and starts it at
//! `timeGetTime() + delay`. A window that needs to finish goes in the must-finish list (and the manager needs an
//! update), the others in the plain list. `update` moves each window by its velocity (whole pixels, truncated), which
//! slows by `0.67` (`0.77` for the fast right slide) once within the threshold of its rest and never drops below one
//! pixel; it stops at its rest. The spiral turns by `0.33` radians and closes by 5 pixels a step until its radius is
//! under half the window's smaller side. The timed bottom slide moves by the fraction of its duration elapsed. The
//! manager is finished once every must-finish window is.
//!
//! `reverseAnimateWindow` turns every window around (the delays mirrored against the longest one, the velocities
//! negated; the spiral from radius 0), and `update` then runs them back out, speeding up by `2 - 0.67` (`4 - 0.67`,
//! `3 - 0.77` for the fast ones) and at most the top speed. `resetToRestPosition` puts every window at its rest;
//! `reset` does that and forgets them.
//!
//! # Upstream quirks that are kept
//!
//! - The right, fast right and left slides stop without putting the window at its rest: it stays where its last
//!   step left it (on the rest once slowed to a pixel a step; short of it when its last step would jump past).
//! - `resetToRestPosition` sets the manager reversed and needing an update (`reset` then clears both).
//! - The plain list's windows are reversed with a maximum delay of 0: their start time is `now - delay` (unsigned).
//! - The timed slide's duration is the process's, set by every timed registration (the shell always passes 500 ms), and
//!   used by every timed window, those registered before included.
//! - The spiral's radius is set from the display's width when the manager is made, not when it is used.
//!
//! # Where this differs from upstream
//!
//! - `timeGetTime()` is the caller's `now` (the shell's clock); `TheDisplay->getWidth()` is the manager's
//!   [`GameWindowManager::display`].
//! - A window freed by `processDestroyList` that an animation still holds is upstream's dangling pointer: recorded in
//!   [`GameWindowManager::undefined`] (the update stops there), as are a float-to-`Int` conversion of a value that is
//!   not finite or past `Int` (the timed slide with a zero duration).
//! - The spiral's `cos` / `sin` of a `float` are MSVC's `float` overloads (`cosf`, `sinf`): petir's
//!   ([`crate::ww::wwmath`]), as all of the client's maths.
//!
//! # Representation
//!
//! [`AnimateWindowManager`] holds the two lists and the processes' parameters ([`Slide`] for the six slides, the
//! spiral's and the timed slide's); the processes are functions on the [`AnimType`].

use crate::client::window_manager::GameWindowManager;
use crate::client::window_script::WinId;

/// `AnimTypes`.
pub mod anim {
    pub const WIN_ANIMATION_NONE: i32 = 0;
    pub const WIN_ANIMATION_SLIDE_RIGHT: i32 = 1;
    pub const WIN_ANIMATION_SLIDE_RIGHT_FAST: i32 = 2;
    pub const WIN_ANIMATION_SLIDE_LEFT: i32 = 3;
    pub const WIN_ANIMATION_SLIDE_TOP: i32 = 4;
    pub const WIN_ANIMATION_SLIDE_BOTTOM: i32 = 5;
    pub const WIN_ANIMATION_SPIRAL: i32 = 6;
    pub const WIN_ANIMATION_SLIDE_BOTTOM_TIMED: i32 = 7;
    pub const WIN_ANIMATION_SLIDE_TOP_FAST: i32 = 8;
    pub const WIN_ANIMATION_COUNT: i32 = 9;
}

/// An `AnimTypes` value.
pub type AnimType = i32;

/// `AnimateWindow`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimateWindow {
    pub delay: u32,
    pub start_pos: (i32, i32),
    pub end_pos: (i32, i32),
    pub cur_pos: (i32, i32),
    pub rest_pos: (i32, i32),
    pub win: Option<WinId>,
    pub vel: (f32, f32),
    pub start_time: u32,
    pub end_time: u32,
    pub anim_type: AnimType,
    pub needs_to_finish: bool,
    pub is_finished: bool,
}

impl AnimateWindow {
    /// `setAnimData(startPos, endPos, curPos, restPos, vel, startTime, endTime)`.
    #[allow(clippy::too_many_arguments)]
    pub fn set_anim_data(&mut self, start: (i32, i32), end: (i32, i32), cur: (i32, i32), rest: (i32, i32), vel: (f32, f32), start_time: u32, end_time: u32) {
        self.start_pos = start;
        self.end_pos = end;
        self.cur_pos = cur;
        self.rest_pos = rest;
        self.vel = vel;
        self.start_time = start_time;
        self.end_time = end_time;
    }
}

/// A slide's parameters (`m_maxVel`, `m_slowDownThreshold`, `m_slowDownRatio`, `m_speedUpRatio`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slide {
    pub max_vel: (f32, f32),
    pub slow_down_threshold: i32,
    pub slow_down_ratio: f32,
    pub speed_up_ratio: f32,
}

impl Slide {
    fn new(max_vel: (f32, f32), threshold: i32, slow: f32, speed_up_base: f32) -> Self {
        Slide { max_vel, slow_down_threshold: threshold, slow_down_ratio: slow, speed_up_ratio: speed_up_base - slow }
    }
}

/// `AnimateWindowManager`.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimateWindowManager {
    /// `m_winList`.
    pub win_list: Vec<AnimateWindow>,
    /// `m_winMustFinishList`.
    pub must_finish_list: Vec<AnimateWindow>,
    pub needs_update: bool,
    pub reverse: bool,
    pub slide_from_right: Slide,
    pub slide_from_right_fast: Slide,
    pub slide_from_left: Slide,
    pub slide_from_top: Slide,
    pub slide_from_top_fast: Slide,
    pub slide_from_bottom: Slide,
    /// The spiral's `m_deltaTheta` and `m_maxR`.
    pub spiral_delta_theta: f32,
    pub spiral_max_r: i32,
    /// `ProcessAnimateWindowSlideFromBottomTimed::m_maxDuration`.
    pub max_duration: u32,
}

impl Default for AnimateWindowManager {
    /// A manager on a display of width 0 (see [`AnimateWindowManager::new`]).
    fn default() -> Self {
        Self::new(0)
    }
}

/// Which list an animation is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum List {
    MustFinish,
    Plain,
}

impl AnimateWindowManager {
    /// `AnimateWindowManager()`, with the processes made on a display `display_width` wide (the spiral's radius).
    pub fn new(display_width: i32) -> Self {
        AnimateWindowManager {
            win_list: Vec::new(),
            must_finish_list: Vec::new(),
            needs_update: false,
            reverse: false,
            slide_from_right: Slide::new((-40.0, 0.0), 80, 0.67, 2.0),
            slide_from_right_fast: Slide::new((-80.0, 0.0), 60, 0.77, 3.0),
            slide_from_left: Slide::new((40.0, 0.0), 80, 0.67, 2.0),
            slide_from_top: Slide::new((0.0, 40.0), 80, 0.67, 2.0),
            slide_from_top_fast: Slide::new((0.0, 60.0), 40, 0.67, 4.0),
            slide_from_bottom: Slide::new((0.0, -40.0), 80, 0.67, 2.0),
            spiral_delta_theta: 0.33,
            spiral_max_r: display_width / 2,
            max_duration: 1000,
        }
    }

    /// `isFinished()`.
    pub fn is_finished(&self) -> bool {
        !self.needs_update
    }
    /// `isReversed()`.
    pub fn is_reversed(&self) -> bool {
        self.reverse
    }
    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.win_list.is_empty() && self.must_finish_list.is_empty()
    }

    /// `init()`: both lists cleared.
    pub fn init(&mut self) {
        self.win_list.clear();
        self.must_finish_list.clear();
        self.needs_update = false;
        self.reverse = false;
    }

    /// `reset()`: every window at its rest, the lists cleared.
    pub fn reset(&mut self, wm: &mut GameWindowManager) {
        self.reset_to_rest_position(wm);
        self.init();
    }

    fn list(&mut self, l: List) -> &mut Vec<AnimateWindow> {
        match l {
            List::MustFinish => &mut self.must_finish_list,
            List::Plain => &mut self.win_list,
        }
    }

    /// `update()` at `now` (`timeGetTime()`).
    pub fn update(&mut self, wm: &mut GameWindowManager, now: u32) {
        if self.needs_update {
            self.needs_update = false;
            for i in 0..self.must_finish_list.len() {
                let done = self.step(wm, List::MustFinish, i, now);
                if wm.undefined.is_some() {
                    return;
                }
                if !done {
                    self.needs_update = true;
                }
            }
        }
        for i in 0..self.win_list.len() {
            self.step(wm, List::Plain, i, now);
            if wm.undefined.is_some() {
                return;
            }
        }
    }

    /// One window's `updateAnimateWindow` or (reversed) `reverseAnimateWindow`.
    fn step(&mut self, wm: &mut GameWindowManager, l: List, i: usize, now: u32) -> bool {
        let reverse = self.reverse;
        let me = self.clone_params();
        let a = &mut self.list(l)[i];
        if reverse {
            me.reverse_animate_window(wm, a, now)
        } else {
            me.update_animate_window(wm, a, now)
        }
    }

    /// The processes' parameters (the processes are stateless besides them).
    fn clone_params(&self) -> Processes {
        Processes {
            slides: [self.slide_from_right, self.slide_from_right_fast, self.slide_from_left, self.slide_from_top, self.slide_from_bottom, self.slide_from_top_fast],
            delta_theta: self.spiral_delta_theta,
            max_r: self.spiral_max_r,
            max_duration: self.max_duration,
            display_width: 0,
        }
    }

    /// `registerGameWindow(win, animType, needsToFinish, ms, delayMs)` at `now`.
    #[allow(clippy::too_many_arguments)]
    pub fn register_game_window(&mut self, wm: &mut GameWindowManager, win: Option<WinId>, anim_type: AnimType, needs_to_finish: bool, ms: u32, delay_ms: u32, now: u32) {
        let Some(win) = win else { return };
        if anim_type <= anim::WIN_ANIMATION_NONE || anim_type >= anim::WIN_ANIMATION_COUNT {
            return;
        }
        let mut a = AnimateWindow { win: Some(win), anim_type, needs_to_finish, delay: delay_ms, ..Default::default() };
        // `processAnim->setMaxDuration(ms)` on the type's process: only the timed slide's keeps it (the others' is the
        // base class's empty one)
        if anim_type == anim::WIN_ANIMATION_SLIDE_BOTTOM_TIMED {
            self.max_duration = ms;
        }
        let mut p = self.clone_params();
        p.display_width = wm.display.0;
        p.init_animate_window(wm, &mut a, now);
        if needs_to_finish {
            self.must_finish_list.push(a);
            self.needs_update = true;
        } else {
            self.win_list.push(a);
        }
    }

    /// `reverseAnimateWindow()` at `now`.
    pub fn reverse_animate_window(&mut self, wm: &mut GameWindowManager, now: u32) {
        self.reverse = true;
        self.needs_update = true;
        let max_delay = self.must_finish_list.iter().map(|a| a.delay).max().unwrap_or(0);
        let mut p = self.clone_params();
        p.display_width = wm.display.0;
        for a in &mut self.must_finish_list {
            p.init_reverse_animate_window(wm, a, max_delay, now);
            a.is_finished = false;
        }
        for a in &mut self.win_list {
            p.init_reverse_animate_window(wm, a, 0, now);
            a.is_finished = false;
        }
    }

    /// `resetToRestPosition()`.
    pub fn reset_to_rest_position(&mut self, wm: &mut GameWindowManager) {
        self.reverse = true;
        self.needs_update = true;
        for a in self.must_finish_list.iter().chain(self.win_list.iter()) {
            if let Some(w) = a.win.and_then(|w| live(wm, w)) {
                wm.win_set_position(w, a.rest_pos.0, a.rest_pos.1);
            }
        }
    }
}

/// The processes' parameters, by [`AnimType`].
struct Processes {
    /// Right, right fast, left, top, bottom, top fast.
    slides: [Slide; 6],
    delta_theta: f32,
    max_r: i32,
    max_duration: u32,
    display_width: i32,
}

/// A slide's geometry: the axis it moves on (0 x, 1 y), the sign of its forward travel, and whether its forward stop
/// puts the window at its rest.
fn slide_kind(t: AnimType) -> (usize, usize, i32, bool) {
    match t {
        anim::WIN_ANIMATION_SLIDE_RIGHT => (0, 0, -1, false),
        anim::WIN_ANIMATION_SLIDE_RIGHT_FAST => (1, 0, -1, false),
        anim::WIN_ANIMATION_SLIDE_LEFT => (2, 0, 1, false),
        anim::WIN_ANIMATION_SLIDE_TOP => (3, 1, 1, true),
        anim::WIN_ANIMATION_SLIDE_BOTTOM => (4, 1, -1, true),
        _ => (5, 1, 1, true),
    }
}

fn get(p: (i32, i32), axis: usize) -> i32 {
    if axis == 0 {
        p.0
    } else {
        p.1
    }
}
fn set(p: &mut (i32, i32), axis: usize, v: i32) {
    if axis == 0 {
        p.0 = v;
    } else {
        p.1 = v;
    }
}
fn getf(p: (f32, f32), axis: usize) -> f32 {
    if axis == 0 {
        p.0
    } else {
        p.1
    }
}
fn setf(p: &mut (f32, f32), axis: usize, v: f32) {
    if axis == 0 {
        p.0 = v;
    } else {
        p.1 = v;
    }
}

/// The animation's window, or `None` (recorded) when `processDestroyList` has freed it.
fn live(wm: &mut GameWindowManager, w: WinId) -> Option<WinId> {
    if wm.ext[w].freed {
        wm.set_undefined("AnimateWindowManager: a window freed by processDestroyList is still animated");
        return None;
    }
    Some(w)
}

/// `(Int)` of a `float` (undefined, recorded, when it is not finite or past `Int`).
fn to_int(wm: &mut GameWindowManager, v: f32) -> i32 {
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&v) {
        wm.set_undefined("AnimateWindowManager: a float converted to Int out of its range");
        return 0;
    }
    v as i32
}

impl Processes {
    /// `initAnimateWindow(animWin)` of the window's type.
    fn init_animate_window(&mut self, wm: &mut GameWindowManager, a: &mut AnimateWindow, now: u32) {
        let t = a.anim_type;
        if matches!(t, anim::WIN_ANIMATION_SLIDE_RIGHT | anim::WIN_ANIMATION_SLIDE_RIGHT_FAST) {
            a.is_finished = false;
        }
        let Some(win) = a.win.and_then(|w| live(wm, w)) else { return };
        let g = &wm.tree.windows[win];
        let rest = (g.region.0, g.region.1);
        let size = g.size;
        let end = rest;
        let width = self.display_width;
        let (start, vel, end_time) = match t {
            anim::WIN_ANIMATION_SPIRAL => {
                let vel = (0.0f32, self.max_r as f32);
                let x = to_int(wm, vel.1 * crate::ww::wwmath::cos(vel.0) + end.0 as f32);
                let y = to_int(wm, vel.1 * crate::ww::wwmath::sin(vel.0) + end.1 as f32);
                ((x, y), vel, 0)
            }
            anim::WIN_ANIMATION_SLIDE_BOTTOM_TIMED => {
                let start = (rest.0, rest.1.wrapping_add(width));
                wm.win_set_position(win, start.0, start.1);
                let d = a.delay;
                a.set_anim_data(start, end, start, rest, (0.0, 0.0), now.wrapping_add(d), now.wrapping_add(self.max_duration).wrapping_add(d));
                return;
            }
            _ => {
                let (k, axis, sign, _) = slide_kind(t);
                let s = self.slides[k];
                let travel = match t {
                    anim::WIN_ANIMATION_SLIDE_RIGHT_FAST => width.wrapping_sub(rest.0).wrapping_add(size.0),
                    _ => width,
                };
                let mut start = rest;
                if t == anim::WIN_ANIMATION_SLIDE_TOP_FAST {
                    start.1 = size.1.wrapping_neg();
                } else {
                    // forward travel towards the rest: the start is on the other side
                    let s0 = get(rest, axis).wrapping_sub(sign.wrapping_mul(travel));
                    set(&mut start, axis, s0);
                }
                let vel = if t == anim::WIN_ANIMATION_SLIDE_RIGHT || t == anim::WIN_ANIMATION_SLIDE_RIGHT_FAST { (s.max_vel.0, 0.0) } else { s.max_vel };
                (start, vel, 0)
            }
        };
        wm.win_set_position(win, start.0, start.1);
        a.set_anim_data(start, end, start, rest, vel, now.wrapping_add(a.delay), end_time);
    }

    /// `initReverseAnimateWindow(animWin, maxDelay)` of the window's type.
    fn init_reverse_animate_window(&mut self, wm: &mut GameWindowManager, a: &mut AnimateWindow, max_delay: u32, now: u32) {
        let t = a.anim_type;
        if t == anim::WIN_ANIMATION_SLIDE_BOTTOM_TIMED {
            let Some(win) = a.win.and_then(|w| live(wm, w)) else { return };
            let rest = a.rest_pos;
            let start = rest;
            let end = (rest.0, rest.1.wrapping_add(self.display_width));
            let cur = (rest.0, rest.1);
            wm.win_set_position(win, start.0, start.1);
            a.set_anim_data(start, end, cur, rest, (0.0, 0.0), now, now.wrapping_add(self.max_duration));
            return;
        }
        if a.delay > 0 {
            a.start_time = now.wrapping_add(max_delay.wrapping_sub(a.delay));
        }
        if t == anim::WIN_ANIMATION_SPIRAL {
            a.vel = (0.0, 0.0);
            return;
        }
        // (`vel.x *= -1`: the sign flipped, as negation does, bit for bit)
        a.vel = (-a.vel.0, -a.vel.1);
        if t == anim::WIN_ANIMATION_SLIDE_RIGHT {
            a.is_finished = false;
        }
        if t == anim::WIN_ANIMATION_SLIDE_RIGHT_FAST {
            a.is_finished = false;
            let Some(win) = a.win.and_then(|w| live(wm, w)) else { return };
            let y = wm.tree.windows[win].region.1;
            a.cur_pos.1 = y;
            a.end_pos.1 = y;
            a.start_pos.1 = y;
        }
    }

    /// `updateAnimateWindow(animWin)`: whether it is done.
    fn update_animate_window(&self, wm: &mut GameWindowManager, a: &mut AnimateWindow, now: u32) -> bool {
        if a.is_finished {
            return true;
        }
        if now < a.start_time {
            return false;
        }
        let Some(win) = a.win.and_then(|w| live(wm, w)) else { return true };
        match a.anim_type {
            anim::WIN_ANIMATION_SPIRAL => {
                let mut cur = a.cur_pos;
                let end = a.end_pos;
                let mut vel = a.vel;
                cur.0 = to_int(wm, vel.1 * crate::ww::wwmath::cos(vel.0) + end.0 as f32);
                cur.1 = to_int(wm, vel.1 * crate::ww::wwmath::sin(vel.0) + end.1 as f32);
                vel.0 += self.delta_theta;
                vel.1 -= 5.0;
                let size = wm.tree.windows[win].size;
                let m_max = (size.0 / 2).min(size.1 / 2);
                if vel.1 < m_max as f32 {
                    a.is_finished = true;
                    wm.win_set_position(win, a.rest_pos.0, a.rest_pos.1);
                    return true;
                }
                wm.win_set_position(win, cur.0, cur.1);
                a.cur_pos = cur;
                a.vel = vel;
                false
            }
            anim::WIN_ANIMATION_SLIDE_BOTTOM_TIMED => {
                let (start, mut cur, end) = (a.start_pos, a.cur_pos, a.end_pos);
                if now < a.start_time {
                    return false;
                }
                let percent_done = 1.0f32 - a.end_time.wrapping_sub(now) as f32 / self.max_duration as f32;
                if now >= a.end_time {
                    cur.1 = end.1;
                    a.is_finished = true;
                    wm.win_set_position(win, cur.0, cur.1);
                    return true;
                }
                cur.1 = to_int(wm, start.1 as f32 + percent_done * end.1.wrapping_sub(start.1) as f32);
                wm.win_set_position(win, cur.0, cur.1);
                a.cur_pos = cur;
                false
            }
            t => {
                let (k, axis, sign, set_on_finish) = slide_kind(t);
                let s = self.slides[k];
                let mut cur = a.cur_pos;
                let end = a.end_pos;
                let mut vel = a.vel;
                let moved = get(cur, axis).wrapping_add(getf(vel, axis) as i32);
                set(&mut cur, axis, moved);
                let passed = if sign < 0 { get(cur, axis) < get(end, axis) } else { get(cur, axis) > get(end, axis) };
                if passed {
                    set(&mut cur, axis, get(end, axis));
                    if set_on_finish {
                        wm.win_set_position(win, cur.0, cur.1);
                    }
                    a.is_finished = true;
                    return true;
                }
                wm.win_set_position(win, cur.0, cur.1);
                a.cur_pos = cur;
                let dist = if sign < 0 { get(cur, axis).wrapping_sub(get(end, axis)) } else { get(end, axis).wrapping_sub(get(cur, axis)) };
                let mut v = getf(vel, axis);
                if dist <= s.slow_down_threshold - i32::from(crate::logic::modules::plant(4713)) {
                    v *= s.slow_down_ratio;
                }
                if sign < 0 {
                    if v >= -1.0 {
                        v = -1.0;
                    }
                } else if v < 1.0 {
                    v = 1.0;
                }
                setf(&mut vel, axis, v);
                a.vel = vel;
                false
            }
        }
    }

    /// `reverseAnimateWindow(animWin)`: whether it is done.
    fn reverse_animate_window(&self, wm: &mut GameWindowManager, a: &mut AnimateWindow, now: u32) -> bool {
        match a.anim_type {
            anim::WIN_ANIMATION_SLIDE_BOTTOM_TIMED => self.update_animate_window(wm, a, now),
            anim::WIN_ANIMATION_SPIRAL => {
                if a.is_finished {
                    return true;
                }
                let Some(win) = a.win.and_then(|w| live(wm, w)) else { return true };
                let mut cur = a.cur_pos;
                let end = a.end_pos;
                let mut vel = a.vel;
                cur.0 = to_int(wm, vel.1 * crate::ww::wwmath::cos(vel.0) + end.0 as f32);
                cur.1 = to_int(wm, vel.1 * crate::ww::wwmath::sin(vel.0) + end.1 as f32);
                vel.0 -= self.delta_theta;
                vel.1 += 5.0;
                if vel.1 > self.max_r as f32 {
                    a.is_finished = true;
                    return true;
                }
                wm.win_set_position(win, cur.0, cur.1);
                a.cur_pos = cur;
                a.vel = vel;
                false
            }
            t => {
                if a.is_finished {
                    return true;
                }
                if now < a.start_time {
                    return false;
                }
                let Some(win) = a.win.and_then(|w| live(wm, w)) else { return true };
                let (k, axis, sign, _) = slide_kind(t);
                let s = self.slides[k];
                let mut cur = a.cur_pos;
                let start = a.start_pos;
                let mut vel = a.vel;
                let moved = get(cur, axis).wrapping_add(getf(vel, axis) as i32);
                set(&mut cur, axis, moved);
                let passed = if sign < 0 { get(cur, axis) > get(start, axis) } else { get(cur, axis) < get(start, axis) };
                if passed {
                    set(&mut cur, axis, get(start, axis));
                    a.is_finished = true;
                    wm.win_set_position(win, cur.0, cur.1);
                    return true;
                }
                wm.win_set_position(win, cur.0, cur.1);
                a.cur_pos = cur;
                let end = a.end_pos;
                let dist = if sign < 0 { get(cur, axis).wrapping_sub(get(end, axis)) } else { get(end, axis).wrapping_sub(get(cur, axis)) };
                let max = getf(s.max_vel, axis);
                let mut v = getf(vel, axis);
                if dist <= s.slow_down_threshold {
                    v *= s.speed_up_ratio;
                } else {
                    v = -max;
                }
                if sign < 0 {
                    if v > -max {
                        v = -max;
                    }
                } else if v < -max {
                    v = -max;
                }
                setf(&mut vel, axis, v);
                a.vel = vel;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::window_manager::{NoGuiHost, SystemFunc};
    use crate::client::window_script::{win_status, WinInstanceData};

    fn window(wm: &mut GameWindowManager, x: i32, y: i32, w: i32, h: i32) -> WinId {
        wm.win_create(&mut NoGuiHost, None, win_status::ENABLED, x, y, w, h, SystemFunc::Default, Some(WinInstanceData::new()))
    }
    fn pos(wm: &GameWindowManager, w: WinId) -> (i32, i32) {
        (wm.tree.windows[w].region.0, wm.tree.windows[w].region.1)
    }

    /// The slides, predicted from ProcessAnimateWindowSlideFromRight / Left / Top / Bottom and their fast kinds: the
    /// window starts the display's width (800) away (the fast right one `800 - x + width`, the fast top one at
    /// `-height`), comes in at 40 (80, 60) a step, slows by 0.67 (0.77) within 80 (60, 40) pixels, and stops: ~~the
    /// right, fast right and left ones one step short of the rest (no last `winSetPosition`), the others at it~~
    /// **missed, corrected after the run:** the right, fast right and left ones stop without a last `winSetPosition`,
    /// so they stay where their last step left them: the right and left ones have slowed to one pixel a step and land
    /// on the rest, but the fast right one (60 pixels out at 80 a step, from 860: 780 .. 140) jumps past it from 140
    /// and stays there; the others are put at the rest; the manager is then finished. Reversed, each goes back to its
    /// start exactly, and `reset` puts them at rest.
    #[test]
    fn the_slides_come_in_and_go_back() {
        let mut wm = GameWindowManager::new((800, 600));
        for t in [anim::WIN_ANIMATION_SLIDE_RIGHT, anim::WIN_ANIMATION_SLIDE_RIGHT_FAST, anim::WIN_ANIMATION_SLIDE_LEFT, anim::WIN_ANIMATION_SLIDE_TOP, anim::WIN_ANIMATION_SLIDE_BOTTOM, anim::WIN_ANIMATION_SLIDE_TOP_FAST] {
            let w = window(&mut wm, 100, 50, 60, 30);
            let mut m = AnimateWindowManager::new(800);
            m.register_game_window(&mut wm, Some(w), t, true, 500, 0, 0);
            let start = pos(&wm, w);
            let expect = match t {
                anim::WIN_ANIMATION_SLIDE_RIGHT => (900, 50),
                anim::WIN_ANIMATION_SLIDE_RIGHT_FAST => (100 + 800 - 100 + 60, 50),
                anim::WIN_ANIMATION_SLIDE_LEFT => (-700, 50),
                anim::WIN_ANIMATION_SLIDE_TOP => (100, -750),
                anim::WIN_ANIMATION_SLIDE_BOTTOM => (100, 850),
                _ => (100, -30),
            };
            assert_eq!(start, expect, "type {t}");
            assert!(!m.is_finished());
            let mut n = 0;
            while !m.is_finished() {
                m.update(&mut wm, 0);
                n += 1;
                assert!(n < 2000, "type {t} never stops");
            }
            let stop = if t == anim::WIN_ANIMATION_SLIDE_RIGHT_FAST { (140, 50) } else { (100, 50) };
            assert_eq!(pos(&wm, w), stop, "type {t}");
            m.reverse_animate_window(&mut wm, 0);
            assert!(m.is_reversed() && !m.is_finished());
            while !m.is_finished() {
                m.update(&mut wm, 0);
            }
            assert_eq!(pos(&wm, w), start, "type {t} back at its start");
            m.reset(&mut wm);
            assert_eq!(pos(&wm, w), (100, 50));
            assert!(m.is_empty() && m.is_finished() && !m.is_reversed());
        }
    }

    /// The spiral, the timed slide and the lists, predicted from ProcessAnimateWindowSpiral / SlideFromBottomTimed and
    /// AnimateWindowManager: the spiral starts 400 to the right (`maxR` = 800 / 2, angle 0) and ends at rest when its
    /// radius is under half the window's smaller side; reversed it runs from radius 0 outwards until past 400. The
    /// timed slide is halfway (`y + 400`) at half its 500 ms and at rest at 500; a delay waits. A window that need not
    /// finish is updated but does not hold the manager; an invalid type or no window registers nothing; a freed
    /// window is undefined.
    #[test]
    fn the_spiral_the_timed_slide_and_the_lists() {
        let mut wm = GameWindowManager::new((800, 600));
        let w = window(&mut wm, 100, 100, 40, 20);
        let mut m = AnimateWindowManager::new(800);
        m.register_game_window(&mut wm, Some(w), anim::WIN_ANIMATION_SPIRAL, true, 500, 0, 0);
        assert_eq!(pos(&wm, w), (500, 100));
        let mut n = 0;
        while !m.is_finished() {
            m.update(&mut wm, 0);
            n += 1;
        }
        assert_eq!(pos(&wm, w), (100, 100));
        assert_eq!(n, (400 - 10) / 5 + 1, "the radius closes by 5 to under 10");
        m.reverse_animate_window(&mut wm, 0);
        assert_eq!(m.must_finish_list[0].vel, (0.0, 0.0));
        while !m.is_finished() {
            m.update(&mut wm, 0);
        }
        assert!(m.must_finish_list[0].is_finished);
        let mut m = AnimateWindowManager::new(800);
        let t = window(&mut wm, 10, 20, 40, 20);
        m.register_game_window(&mut wm, Some(t), anim::WIN_ANIMATION_SLIDE_BOTTOM_TIMED, true, 500, 100, 1000);
        assert_eq!(pos(&wm, t), (10, 820));
        m.update(&mut wm, 1050);
        assert_eq!(pos(&wm, t), (10, 820), "the delay");
        m.update(&mut wm, 1350);
        assert_eq!(pos(&wm, t), (10, 420));
        m.update(&mut wm, 1600);
        assert_eq!(pos(&wm, t), (10, 20));
        assert!(m.is_finished());
        m.reverse_animate_window(&mut wm, 2000);
        m.update(&mut wm, 2250);
        assert_eq!(pos(&wm, t), (10, 420), "reversed: out again over the same time");
        m.update(&mut wm, 2500);
        assert!(m.is_finished());
        // the plain list
        let p = window(&mut wm, 0, 0, 10, 10);
        let mut m = AnimateWindowManager::new(800);
        m.register_game_window(&mut wm, Some(p), anim::WIN_ANIMATION_SLIDE_LEFT, false, 500, 0, 0);
        assert!(m.is_finished() && !m.is_empty());
        m.update(&mut wm, 0);
        assert_eq!(pos(&wm, p), (-760, 0));
        m.register_game_window(&mut wm, Some(p), anim::WIN_ANIMATION_NONE, true, 500, 0, 0);
        m.register_game_window(&mut wm, None, anim::WIN_ANIMATION_SPIRAL, true, 500, 0, 0);
        assert_eq!((m.win_list.len(), m.must_finish_list.len()), (1, 0));
        m.reset_to_rest_position(&mut wm);
        assert!(m.is_reversed() && !m.is_finished());
        assert_eq!(pos(&wm, p), (0, 0));
        m.init();
        assert!(m.is_empty());
        // the fast right slide takes its window's height when reversed
        let f = window(&mut wm, 50, 60, 10, 10);
        let mut m = AnimateWindowManager::new(800);
        m.register_game_window(&mut wm, Some(f), anim::WIN_ANIMATION_SLIDE_RIGHT_FAST, true, 500, 0, 0);
        wm.win_set_position(f, 300, 77);
        m.reverse_animate_window(&mut wm, 0);
        assert_eq!((m.must_finish_list[0].cur_pos.1, m.must_finish_list[0].start_pos.1, m.must_finish_list[0].end_pos.1), (77, 77, 77));
        // a freed window
        wm.win_destroy(Some(f));
        wm.update(&mut NoGuiHost);
        m.update(&mut wm, 0);
        assert!(wm.undefined.is_some());
    }
}
