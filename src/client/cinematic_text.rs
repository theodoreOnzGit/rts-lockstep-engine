// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/Display.h (setCinematicText, setCinematicFont, setCinematicTextFrames;
//     m_cinematicText, m_cinematicFont, m_cinematicTextFrames)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Display.cpp (Display::Display's cinematic members)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (W3DDisplay::draw's cinematic text,
//     lines 1926-1953)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GlobalLanguage.cpp (GlobalLanguage::adjustFontSize, the default
//     m_resolutionFontSizeAdjustment)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! GitHub #146: the cinematic text, a line a script shows over the letterbox (`TheDisplay`'s `m_cinematicText`).
//!
//! # As upstream does it
//!
//! - **The calls.** The script action `DISPLAY_CINEMATIC_TEXT` (`ScriptActions::doDisplayCinematicText`,
//!   ScriptActions.cpp:2563, translated in `logic::world::script_actions`) fetches the label's text and gives it to
//!   `TheDisplay->setCinematicText`, parses the font's name, size and bold from its `"Name - Size: N [Bold]"` parameter
//!   and gives `TheFontLibrary->getFont(name, TheGlobalLanguageData->adjustFontSize(size), bold)` to `setCinematicFont`,
//!   and gives `LOGICFRAMES_PER_SECOND * seconds` to `setCinematicTextFrames`. They reach the client as the logic's
//!   `ClientEvent::CinematicText` events; [`CinematicText::apply`] makes them (and `adjustFontSize`, [`adjust_font_size`],
//!   which the World leaves to the client: it reads the display's resolution).
//! - **The draw** (`W3DDisplay::draw`, W3DDisplay.cpp:1927, once a drawn frame): right after the letterbox
//!   (`renderLetterBox`), so over it, while the text is not empty and the frame count is not 0: a new display string
//!   wrapped at the display's width less 20 and centred line by line, white, with a transparent black drop colour, in the
//!   cinematic font, at `x = (width - the string's width) / 2` (or 20 if the string is wider than the display) and
//!   `y = height * .9` ([`CinematicText::draw`]); then the count goes down by one.
//!
//! # Upstream quirks kept
//!
//! - The count goes down once a **drawn** frame, not a logic frame. Upstream draws once per `GameEngine::update` (30 a
//!   second, the frame limiter's rate), so the two agree there; the client's renderer runs faster, and calls
//!   [`CinematicText::draw`] once per client frame (30 a second) instead.
//! - `y` is `getHeight() * .9` in `double`, truncated (`Int height = ...`): 759 on an 844-pixel display.
//! - The display's size is `UnsignedInt`: the string's width is compared, and subtracted, as unsigned.
//! - Nothing clears the text: `Display::reset` leaves it (only the constructor sets it empty), so a count left over
//!   carries into the next game. The count is what stops the draw.
//! - The font's name is the one the action's parse made (`"rial - Size:20al - Size:20 - Size:20"` from `"Arial - Size:20"`,
//!   see `logic::world::script_actions`), which no font has: Windows' font mapper then picks its default face at that
//!   size. The client draws in its own font at that size.
//!
//! # Where this differs from upstream
//!
//! - Upstream makes a new display string every frame and never frees it (a leak): the draw is returned
//!   ([`CinematicDraw`]) for the client to show, nothing is allocated.
//! - The string's width (`DisplayString::getWidth`, the font's measure of the wrapped text) is the caller's: the client's
//!   text layout centres each line itself, which is what `setWordWrapCentered(TRUE)` and the centred `x` give.

use crate::common::base_type::real_to_int_floor;
use crate::logic::client_event::CinematicTextCall;
use crate::logic::modules::plant;

/// `GlobalLanguage::GlobalLanguage`'s `m_resolutionFontSizeAdjustment` (GlobalLanguage.cpp:119), until `Language.ini`'s
/// `ResolutionFontAdjustment` sets another.
pub const DEFAULT_RESOLUTION_FONT_SIZE_ADJUSTMENT: f32 = 0.7;

/// `GlobalLanguage::adjustFontSize(theFontSize)` (GlobalLanguage.cpp:192): the size scaled by the display's width over 800
/// (`TheGlobalData->m_xResolution`), damped by the language's adjustment, the factor kept within 1 .. 2, floored. (Two
/// `if`s as upstream: `clamp` would differ on a NaN factor.)
#[allow(clippy::manual_clamp)]
pub fn adjust_font_size(the_font_size: i32, x_resolution: i32, resolution_font_size_adjustment: f32) -> i32 {
    let mut adjust_factor = x_resolution as f32 / 800.0f32;
    adjust_factor = 1.0f32 + (adjust_factor - 1.0f32) * resolution_font_size_adjustment;
    if adjust_factor < 1.0f32 {
        adjust_factor = 1.0f32;
    }
    if adjust_factor > 2.0f32 {
        adjust_factor = 2.0f32;
    }
    real_to_int_floor(the_font_size as f32 * adjust_factor)
}

/// The cinematic font (`m_cinematicFont`, a `GameFont`): its name, point size and bold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CinematicFont {
    pub name: Vec<u8>,
    pub point_size: i32,
    pub bold: bool,
}

/// What one frame's cinematic text draws: the display string's settings and `draw(x, y, color, backColor)`'s arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CinematicDraw {
    /// `setWordWrap(TheDisplay->getWidth() - 20)`.
    pub word_wrap: i32,
    /// `setWordWrapCentered(TRUE)`.
    pub word_wrap_centered: bool,
    /// `setText(text)`, `text.translate(m_cinematicText)`: each byte sign-extended to a unit.
    pub text: Vec<u16>,
    /// `setFont(m_cinematicFont)` (`None`: `NULL`, no script set one: the display string's default font).
    pub font: Option<CinematicFont>,
    /// The `x` and `y` given to `draw`.
    pub x: i32,
    pub y: i32,
    /// `GameMakeColor(255, 255, 255, 255)` and `GameMakeColor(0, 0, 0, 0)`, `0xAARRGGBB`.
    pub color: u32,
    pub back_color: u32,
}

/// `TheDisplay`'s cinematic text members (Display.h:194).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CinematicText {
    /// `m_cinematicText` (`Display::Display`: `AsciiString::TheEmptyString`).
    pub text: Vec<u8>,
    /// `m_cinematicFont` (`NULL`).
    pub font: Option<CinematicFont>,
    /// `m_cinematicTextFrames` (0).
    pub frames: i32,
}

impl CinematicText {
    /// `setCinematicText(string)`.
    pub fn set_cinematic_text(&mut self, text: &[u8]) {
        self.text = text.to_vec();
    }

    /// `setCinematicFont(font)`.
    pub fn set_cinematic_font(&mut self, font: Option<CinematicFont>) {
        self.font = font;
    }

    /// `setCinematicTextFrames(frames)`.
    pub fn set_cinematic_text_frames(&mut self, frames: i32) {
        self.frames = frames;
    }

    /// One of the logic's `ClientEvent::CinematicText` calls made on the display: the font through `adjustFontSize` at the
    /// display's width (`m_xResolution`) and the language's adjustment, then `getFont`'s font of that name, size and bold.
    pub fn apply(&mut self, call: &CinematicTextCall, x_resolution: i32, resolution_font_size_adjustment: f32) {
        match call {
            CinematicTextCall::Text(t) => self.set_cinematic_text(t),
            CinematicTextCall::Font { name, size, bold } => {
                let point_size = adjust_font_size(*size, x_resolution, resolution_font_size_adjustment);
                self.set_cinematic_font(Some(CinematicFont { name: name.clone(), point_size, bold: *bold }));
            }
            CinematicTextCall::Frames(f) => self.set_cinematic_text_frames(*f),
        }
    }

    /// `W3DDisplay::draw`'s cinematic text (W3DDisplay.cpp:1926): on a display `display_width` by `display_height`, with
    /// `string_width` the display string's `getWidth()` once its wrap, text and font are set; `None` when nothing is
    /// drawn. Counts the frame down.
    pub fn draw(&mut self, display_width: u32, display_height: u32, string_width: impl FnOnce(&CinematicDraw) -> i32) -> Option<CinematicDraw> {
        let shown = !self.text.is_empty() || plant(9390);
        if !(shown && (self.frames != 0 || plant(9386))) {
            return None;
        }
        // "set word wrap if neccessary"
        let word_wrap = if plant(9389) { display_width as i32 } else { display_width.wrapping_sub(20) as i32 };
        let mut d = CinematicDraw {
            word_wrap,
            word_wrap_centered: true,
            text: self.text.iter().map(|&c| (c as i8) as i16 as u16).collect(),
            font: self.font.clone(),
            x: 0,
            y: 0,
            color: 0xFFFF_FFFF,
            back_color: 0x0000_0000,
        };
        d.y = if plant(9387) { petir::Real::round(f64::from(display_height) * 0.9f64) as i32 } else { (f64::from(display_height) * 0.9f64) as i32 };
        let w = string_width(&d);
        d.x = if (w as u32) > display_width && !plant(9388) { 20 } else { (display_width.wrapping_sub(w as u32) / 2) as i32 };
        if !plant(9385) {
            self.frames = self.frames.wrapping_sub(1);
        }
        Some(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #146: the draw, predicted from W3DDisplay.cpp before the run: nothing with no text or no frames; at 390x844 a
    /// string 200 wide is at x 95, y 759 (844 * .9 = 759.6, truncated), wrapped at 370; one wider than the display is at x
    /// 20; each draw takes a frame; a byte 0xE9 becomes the unit 0xFFE9 (sign-extended).
    #[test]
    fn the_cinematic_text_draws_as_upstream() {
        let mut c = CinematicText::default();
        assert_eq!(c.draw(390, 844, |_| 0), None);
        c.apply(&CinematicTextCall::Text(b"Hi \xE9".to_vec()), 390, DEFAULT_RESOLUTION_FONT_SIZE_ADJUSTMENT);
        assert_eq!(c.draw(390, 844, |_| 0), None, "no frames");
        c.apply(&CinematicTextCall::Frames(2), 390, DEFAULT_RESOLUTION_FONT_SIZE_ADJUSTMENT);
        c.apply(&CinematicTextCall::Font { name: b"rial".to_vec(), size: 20, bold: true }, 1600, DEFAULT_RESOLUTION_FONT_SIZE_ADJUSTMENT);
        let d = c.draw(390, 844, |d| {
            assert_eq!(d.word_wrap, 370);
            200
        });
        let d = d.expect("drawn");
        assert_eq!((d.x, d.y, d.text.clone()), (95, 759, vec![72, 105, 32, 0xFFE9]));
        // 1600 / 800 = 2, damped by 0.7: 1.7, 20 * 1.7 = 34
        assert_eq!(d.font.map(|f| (f.point_size, f.bold)), Some((34, true)));
        assert_eq!(c.draw(390, 844, |_| 500).map(|d| d.x), Some(20));
        assert_eq!(c.frames, 0);
        assert_eq!(c.draw(390, 844, |_| 0), None);
        // a phone's width: the factor is at least 1
        assert_eq!(adjust_font_size(20, 390, DEFAULT_RESOLUTION_FONT_SIZE_ADJUSTMENT), 20);
        assert_eq!(adjust_font_size(20, 4000, DEFAULT_RESOLUTION_FONT_SIZE_ADJUSTMENT), 40);
    }
}
