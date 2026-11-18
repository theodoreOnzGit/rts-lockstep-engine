//! UNTESTED DRAFT (#202, #208, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameClient/ParabolicEase.h
//!   GeneralsMD/Code/GameEngine/Source/GameClient/ParabolicEase.cpp
//!
//! "Ease in and out based on a parabolic function" (Robert Minsk, May 2003). Used by the camera. The arithmetic mixes `Real` (f32)
//! and `double` literals exactly as upstream does (`2.0*(param - m_in)` is a double): kept, because the rounding is part of
//! the camera's path. These are plain `f32`/`f64` operations (`+ - * /` only, no libm), identical in native and wasm.

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
    /// `ParabolicEase()` (the header's constructor sets both to 0).
    pub fn new() -> ParabolicEase {
        ParabolicEase { m_in: 0.0, m_out: 0.0 }
    }

    /// `ParabolicEase::setEaseTimes(easeInTime, easeOutTime)`: `m_in` is the ease-in time, `m_out` is `1 - easeOutTime`; each is clamped
    /// to `[0, 1]` (a `DEBUG_CRASH` upstream, compiled out of release), and an overlap (`m_in > m_out`) sets `m_in = m_out`.
    pub fn set_ease_times(&mut self, ease_in_time: f32, ease_out_time: f32) {
        self.m_in = ease_in_time;
        if self.m_in < 0.0 || self.m_in > 1.0 {
            // DEBUG_CRASH(("Ease-in out of range (in = %g)\n", m_in));
            self.m_in = clamp(self.m_in);
        }
        self.m_out = 1.0 - ease_out_time;
        if self.m_out < 0.0 || self.m_out > 1.0 {
            // DEBUG_CRASH(("Ease-out out of range (out = %g)\n", m_out));
            self.m_out = clamp(self.m_out);
        }
        if self.m_in > self.m_out {
            // DEBUG_CRASH(("Ease-in and ease-out overlap (in = %g, out = %g)\n", m_in, m_out));
            self.m_in = self.m_out;
        }
    }

    /// `ParabolicEase::operator()(param)`: the eased value for `param` in `[0, 1]` (clamped when outside). Upstream's `#if 0` branch (a
    /// velocity-normalised form) is dead and not translated; the live one is `v0 = 1 + out - in` and three pieces: the ease-in
    /// parabola, the constant-speed middle and the ease-out parabola. The `2.0*` terms are `double`, the result is narrowed to `Real`.
    pub fn eval(&self, mut param: f32) -> f32 {
        if param < 0.0 || param > 1.0 {
            // DEBUG_CRASH(("Ease-in/ease-out parameter out of range (param = %g)\n", param));
            param = clamp(param);
        }
        let v0: f32 = 1.0f32 + self.m_out - self.m_in;
        if param < self.m_in {
            // "param < in" avoids a division by zero at param = 0.0f and in = 0.0f
            param * param / (v0 * self.m_in)
        } else if param <= self.m_out {
            // "param <= out" avoid a division by zero when param = 1.0f and out = 1.0f
            ((self.m_in as f64 + 2.0 * ((param - self.m_in) as f64)) / (v0 as f64)) as f32
        } else {
            let a = self.m_in as f64 + 2.0 * ((self.m_out - self.m_in) as f64);
            let b = 2.0 * ((param - self.m_out) as f64) + (self.m_out * self.m_out) as f64 - (param * param) as f64;
            ((a + b / ((1.0f32 - self.m_out) as f64)) / (v0 as f64)) as f32
        }
    }
}

impl Default for ParabolicEase {
    fn default() -> Self {
        ParabolicEase::new()
    }
}
