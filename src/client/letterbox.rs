// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (LETTER_BOX_FADE_TIME,
//     W3DDisplay::renderLetterBox, isLetterBoxFading, isLetterBoxed, toggleLetterBox, enableLetterBox)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Display.cpp (Display::Display's and Display::reset's letterbox members)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Display.h (m_letterBoxFadeLevel, m_letterBoxEnabled,
//     m_letterBoxFadeStartTime)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #144: the script letterbox, the black bars a cinematic draws at the top and bottom of the display
//! (`TheDisplay`'s letterbox).
//!
//! # As upstream does it
//!
//! - **The calls.** A script's `doLetterBoxMode` (ScriptActions.cpp:3778/:3783, which reaches the client as
//!   `ScriptInputCall::EnableLetterBox`) calls `TheDisplay->enableLetterBox(enable)` ([`LetterBox::enable_letter_box`]):
//!   only a change of state counts; it starts the fade at `timeGetTime()` and tells the tactical view
//!   `setZoomLimited(!enable)`. The debug key `MSG_META_DEMO_TOGGLE_LETTERBOX` calls `toggleLetterBox`
//!   ([`LetterBox::toggle_letter_box`]), which flips the state without that check. `Display::reset` (the engine's reset,
//!   `clearGameData`) takes the letterbox off at once ([`LetterBox::reset`]).
//! - **The draw** (`W3DDisplay::draw`, once a rendered frame): after the views, the in-game UI (the windows) and the
//!   mouse, and before the cinematic text and the debug display, `renderLetterBox(now)` ([`LetterBox::render_letter_box`]):
//!   - enabled: the fade level, while it is not 1, is `(now - start) / LETTER_BOX_FADE_TIME` (an `UnsignedInt`
//!     difference over 1,000 ms), at most 1; two bars, `drawFillRect` in black with the fade as alpha
//!     (`(Int)(fade * 255) << 24`): the top one `(0, 0, width, (height - 9/16 width) / 2)` and the bottom one from
//!     `height - (height - 9/16 width) / 2`, `height` tall (it runs off the display's bottom), so the view between them
//!     is 16:9;
//!   - disabled while the fade is not 0: the fade is `1 - (now - start) / 1000`, at least 0, and only the **top** bar
//!     is drawn (the bottom one's line is commented out upstream);
//!   - disabled and faded out: nothing drawn (`m_letterBoxEnabled = FALSE` again).
//!
//! # Upstream quirks kept
//!
//! - The fade does not carry over: enabling it while it fades out starts again from 0, and disabling it while it fades
//!   in starts the fade-out from 1.
//! - The fade-out draws the top bar only.
//! - The bars' sizes are the float expressions converted to `Int` by `drawFillRect`'s parameters (truncation). On a
//!   display wider than 16:9 the height is negative: the rectangles are kept as upstream computes them
//!   ([`FillRect`]); the drawing makes nothing of a negative size.
//!
//! # Where this differs from upstream
//!
//! `SLIDE_LETTERBOX` (the bars sliding the tactical view's origin) is not defined upstream: not translated. The bars are
//! returned ([`FillRect`]), not drawn: the client draws them. The tactical view's `setZoomLimited` is returned for the
//! caller to make (`TheTacticalView` may be absent, as upstream checks).

use crate::logic::modules::plant;

/// `LETTER_BOX_FADE_TIME` (W3DDisplay.cpp:2007): 1,000 ms.
pub const LETTER_BOX_FADE_TIME: f32 = 1000.0;

/// One `drawFillRect(startX, startY, width, height, color)` call, its arguments as upstream passes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FillRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// `0xAARRGGBB`: black, the fade as alpha.
    pub color: u32,
}

/// `TheDisplay`'s letterbox members (Display.h:208).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LetterBox {
    /// `m_letterBoxFadeLevel`: 0 (none) to 1 (full).
    pub fade_level: f32,
    /// `m_letterBoxEnabled`.
    pub enabled: bool,
    /// `m_letterBoxFadeStartTime`, `timeGetTime()` milliseconds.
    pub fade_start_time: u32,
}

impl Default for LetterBox {
    /// `Display::Display()` (Display.cpp:56): fade 0, disabled, started at 0.
    fn default() -> Self {
        LetterBox { fade_level: 0.0, enabled: false, fade_start_time: 0 }
    }
}

impl LetterBox {
    /// `W3DDisplay::enableLetterBox(enable)` (W3DDisplay.cpp:2116) at `now` (`timeGetTime()`): a change of state starts
    /// the fade. Returns the tactical view's `setZoomLimited` argument when the state changed (`0` enabled, `1`
    /// disabled), `None` otherwise.
    pub fn enable_letter_box(&mut self, enable: bool, now: u32) -> Option<bool> {
        if enable {
            if !self.enabled || plant(9335) {
                // "letterbox mode not previously enabled"
                self.enabled = true;
                self.fade_start_time = now;
                return Some(false);
            }
        } else if self.enabled {
            // "letterbox mode no previously disabled"
            self.enabled = false;
            if !plant(9336) {
                self.fade_start_time = now;
            }
            return Some(true);
        }
        None
    }

    /// `W3DDisplay::toggleLetterBox()` (W3DDisplay.cpp:2104, the debug key's): the state flipped, the fade started.
    /// Returns the tactical view's `setZoomLimited(!m_letterBoxEnabled)` argument.
    pub fn toggle_letter_box(&mut self, now: u32) -> bool {
        self.enabled = !self.enabled;
        self.fade_start_time = now;
        if plant(9339) {
            return self.enabled;
        }
        !self.enabled
    }

    /// `Display::reset()`'s letterbox (Display.cpp:350): "Remove letterbox border that may have been enabled by a
    /// script": fade 0, disabled (the start time is kept).
    pub fn reset(&mut self) {
        if !plant(9338) {
            self.fade_level = 0.0;
        }
        self.enabled = false;
    }

    /// `W3DDisplay::isLetterBoxFading()` (W3DDisplay.cpp:2060).
    pub fn is_letter_box_fading(&self) -> bool {
        if self.enabled && self.fade_level != 1.0 {
            return true;
        }
        if !self.enabled && self.fade_level != 0.0 && !plant(9337) {
            return true;
        }
        false
    }

    /// `W3DDisplay::isLetterBoxed()` (W3DDisplay.cpp:2070).
    pub fn is_letter_boxed(&self) -> bool {
        self.enabled
    }

    /// `W3DDisplay::renderLetterBox(currentTime)` (W3DDisplay.cpp:2011) on a display `width` x `height` (`m_width`,
    /// `m_height`): the fade moved on, and the bars it draws (see the module docs), in order.
    pub fn render_letter_box(&mut self, current_time: u32, width: i32, height: i32) -> Vec<FillRect> {
        let mut out = Vec::new();
        // `(m_height-(9.0f/16.0f * m_width))*0.5f`, one rounding per operation, as the C++
        let bar = |h: i32, w: i32| -> f32 {
            let nine_sixteenths = 9.0f32 / 16.0f32;
            if plant(9334) {
                return (h as f32 - nine_sixteenths * w as f32) * 1.0;
            }
            (h as f32 - nine_sixteenths * w as f32) * 0.5
        };
        let color = |fade: f32| -> u32 {
            if plant(9340) {
                return ((fade * 255.0 + 0.5) as i32 as u32) << 24;
            }
            ((fade * 255.0) as i32 as u32) << 24
        };
        if self.enabled {
            if self.fade_level != 1.0 {
                self.fade_level = current_time.wrapping_sub(self.fade_start_time) as f32 / LETTER_BOX_FADE_TIME;
                if self.fade_level > 1.0 && !plant(9331) {
                    self.fade_level = 1.0;
                }
            }
            let lbcolor = color(self.fade_level);
            out.push(FillRect { x: 0, y: 0, width, height: bar(height, width) as i32, color: lbcolor });
            out.push(FillRect { x: 0, y: (height as f32 - bar(height, width)) as i32, width, height, color: lbcolor });
        } else if self.fade_level != 0.0 {
            // "letter box is disabled, but may still be fading out"
            self.fade_level = 1.0 - current_time.wrapping_sub(self.fade_start_time) as f32 / LETTER_BOX_FADE_TIME;
            if self.fade_level < 0.0 && !plant(9332) {
                self.fade_level = 0.0;
            }
            let lbcolor = color(self.fade_level);
            out.push(FillRect { x: 0, y: 0, width, height: bar(height, width) as i32, color: lbcolor });
            if plant(9333) {
                out.push(FillRect { x: 0, y: (height as f32 - bar(height, width)) as i32, width, height, color: lbcolor });
            }
        } else {
            // "box has finished fading out"
            self.enabled = false;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// *Predictions (written before the run):* on 800x600 the bars are 75 high (`(600 - 450) / 2`), the bottom one from
    /// y 525; enabled at 1,000 ms, half faded at 1,500 (alpha 127, `(Int)127.5`), full from 2,000 on (and the level
    /// stays 1, so a later clock wrap changes nothing); a second enable does not restart it; disabled at 3,000, the
    /// fade-out draws only the top bar, at alpha 191 at 3,250 (`(Int)191.25`), and nothing from 4,000 on; on a phone
    /// (390x844) each bar is 312 high (`(844 - 219.375) / 2 = 312.31`), the bottom one from 531; the reset takes it off
    /// at once.
    ///
    /// *Results (2026-11-11):* held.
    #[test]
    fn the_bars_fade_in_and_out_as_upstream_draws_them() {
        let mut lb = LetterBox::default();
        assert!(lb.render_letter_box(0, 800, 600).is_empty());
        assert_eq!(lb.enable_letter_box(true, 1000), Some(false));
        assert_eq!(lb.enable_letter_box(true, 1200), None);
        let r = lb.render_letter_box(1500, 800, 600);
        assert_eq!(r, vec![FillRect { x: 0, y: 0, width: 800, height: 75, color: 127 << 24 }, FillRect { x: 0, y: 525, width: 800, height: 600, color: 127 << 24 }]);
        assert!(lb.is_letter_box_fading() && lb.is_letter_boxed());
        let r = lb.render_letter_box(2100, 800, 600);
        assert_eq!(r[0].color, 0xFF00_0000);
        assert!(!lb.is_letter_box_fading());
        assert_eq!(lb.render_letter_box(5, 800, 600)[0].color, 0xFF00_0000);
        assert_eq!(lb.enable_letter_box(false, 3000), Some(true));
        let r = lb.render_letter_box(3250, 800, 600);
        assert_eq!(r, vec![FillRect { x: 0, y: 0, width: 800, height: 75, color: 191 << 24 }]);
        assert!(lb.render_letter_box(4000, 800, 600).is_empty() || lb.fade_level == 0.0);
        assert!(lb.render_letter_box(4100, 800, 600).is_empty());
        assert!(!lb.is_letter_box_fading());
        lb.enable_letter_box(true, 0);
        let r = lb.render_letter_box(1000, 390, 844);
        assert_eq!((r[0].height, r[1].y), (312, 531));
        lb.reset();
        assert!(!lb.is_letter_boxed() && lb.render_letter_box(1100, 390, 844).is_empty());
        assert!(!lb.toggle_letter_box(5) && lb.is_letter_boxed());
    }
}
