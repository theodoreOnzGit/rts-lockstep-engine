//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 700,000 cases (setEaseTimes, both constructors, operator()) (test: tests/differential_draft_parabolic_ease.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameClient/ParabolicEase.h
//!   GeneralsMD/Code/GameEngine/Source/GameClient/ParabolicEase.cpp
//!
//! "Ease in and out based on a parabolic function" (Robert Minsk, May 2003). Used by the camera. The arithmetic mixes `Real` (f32)
//! and `double` literals exactly as upstream does (`2.0*(param - m_in)` is a double): kept, because the rounding is part of
//! the camera's path. These are plain `f32`/`f64` operations (`+ - * /` only, no libm), identical in native and wasm.

use crate::logic::modules::plant;

/// `namespace { clamp(val, min = 0, max = 1) }`.
fn clamp(mut val: f32) -> f32 {
    let min = 0.0f32;
    let max = 1.0f32;
    if val < min {
        val = min;
    } else if val > max {
        val = max;
    }
    val
}

/// `class ParabolicEase`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParabolicEase {
    /// `m_in`.
    pub m_in: f32,
    /// `m_out`.
    pub m_out: f32,
}

impl ParabolicEase {
    /// `ParabolicEase(easeInTime, easeOutTime)`: the header's constructor, `setEaseTimes(easeInTime, easeOutTime)`.
    pub fn with_times(ease_in_time: f32, ease_out_time: f32) -> ParabolicEase {
        let mut e = ParabolicEase { m_in: 0.0, m_out: 0.0 };
        e.set_ease_times(ease_in_time, ease_out_time);
        e
    }

    /// `ParabolicEase()`: the defaults are `(0.0f, 0.0f)`, so `m_in = 0` and `m_out = 1 - 0 = 1`.
    /// ~~(the header's constructor sets both to 0)~~ **Corrected 2026-11-18 (#215):** it calls `setEaseTimes`, so `m_out` is 1.
    pub fn new() -> ParabolicEase {
        if plant(11476) {
            ParabolicEase { m_in: 0.0, m_out: 0.0 }
        } else {
            ParabolicEase::with_times(0.0, 0.0)
        }
    }

    /// `ParabolicEase::setEaseTimes(easeInTime, easeOutTime)`: `m_in` is the ease-in time, `m_out` is `1 - easeOutTime`; each is clamped
    /// to `[0, 1]` (a `DEBUG_CRASH` upstream, compiled out of release), and an overlap (`m_in > m_out`) sets `m_in = m_out`.
    pub fn set_ease_times(&mut self, ease_in_time: f32, ease_out_time: f32) {
        self.m_in = ease_in_time;
        if self.m_in < 0.0 || self.m_in > 1.0 {
            // DEBUG_CRASH(("Ease-in out of range (in = %g)\n", m_in));
            self.m_in = clamp(self.m_in);
        }
        self.m_out = if plant(11468) { ease_out_time } else { 1.0 - ease_out_time };
        if self.m_out < 0.0 || self.m_out > 1.0 {
            // DEBUG_CRASH(("Ease-out out of range (out = %g)\n", m_out));
            self.m_out = clamp(self.m_out);
        }
        if self.m_in > self.m_out {
            // DEBUG_CRASH(("Ease-in and ease-out overlap (in = %g, out = %g)\n", m_in, m_out));
            if plant(11469) {
                self.m_out = self.m_in;
            } else {
                self.m_in = self.m_out;
            }
        }
    }

    /// `ParabolicEase::operator()(param)`: the eased value for `param` in `[0, 1]` (clamped when outside). Upstream's `#if 0` branch (a
    /// velocity-normalised form) is dead and not translated; the live one is `v0 = 1 + out - in` and three pieces: the ease-in
    /// parabola, the constant-speed middle and the ease-out parabola. The `2.0*` terms are `double`, the result is narrowed to `Real`.
    pub fn eval(&self, mut param: f32) -> f32 {
        if if plant(11470) { param < 0.0 } else { param < 0.0 || param > 1.0 } {
            // DEBUG_CRASH(("Ease-in/ease-out parameter out of range (param = %g)\n", param));
            param = clamp(param);
        }
        let v0: f32 = if plant(11471) { 1.0f32 + (self.m_out - self.m_in) } else { 1.0f32 + self.m_out - self.m_in };
        if (if plant(11472) { param <= self.m_in } else { param < self.m_in }) {
            // "param < in" avoids a division by zero at param = 0.0f and in = 0.0f
            param * param / (v0 * self.m_in)
        } else if (if plant(11475) { param < self.m_out } else { param <= self.m_out }) {
            // "param <= out" avoid a division by zero when param = 1.0f and out = 1.0f
            if plant(11473) { (self.m_in + 2.0 * (param - self.m_in)) / v0 } else { ((self.m_in as f64 + 2.0 * ((param - self.m_in) as f64)) / (v0 as f64)) as f32 }
        } else {
            let a = self.m_in as f64 + 2.0 * ((self.m_out - self.m_in) as f64);
            let b = if plant(11474) { 2.0 * ((param - self.m_out) as f64) + (self.m_out as f64) * (self.m_out as f64) - (param as f64) * (param as f64) } else { 2.0 * ((param - self.m_out) as f64) + (self.m_out * self.m_out) as f64 - (param * param) as f64 };
            ((a + b / ((1.0f32 - self.m_out) as f64)) / (v0 as f64)) as f32
        }
    }
}

impl Default for ParabolicEase {
    fn default() -> Self {
        ParabolicEase::new()
    }
}
