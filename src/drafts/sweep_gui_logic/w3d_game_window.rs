//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases per function (test: tests/differential_draft_w3dwin.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameWindow.cpp (W3DGameWindow::getTextSize,
//!   setTextLoc)
//!
//! A W3D window draws its text with a `Render2DSentenceClass` (`m_textRenderer`); the sentence is rebuilt when the
//! text position changes (`m_newTextPos`).

use crate::logic::modules::plant;

/// What `getTextSize` asks of the sentence renderer.
pub trait TextRenderer {
    /// `m_textRenderer.Get_Text_Extents(text)`: (X, Y).
    fn get_text_extents(&self, text: &[u8]) -> (f32, f32);
}

/// `W3DGameWindow`'s text placement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct W3dGameWindowText {
    /// `m_textPos`.
    pub text_pos: (i32, i32),
    /// `m_newTextPos`.
    pub new_text_pos: bool,
}

impl W3dGameWindowText {
    /// `W3DGameWindow::getTextSize(width, height)` (W3DGameWindow.cpp:584): the extents of the window's text, each
    /// output only if wanted; the `Real` extents convert to `Int` by truncation.
    pub fn get_text_size<R: TextRenderer>(&self, r: &R, text: &[u8], want_width: bool, want_height: bool) -> (Option<i32>, Option<i32>) {
        let extents = r.get_text_extents(text);
        let conv = |v: f32| if plant(15758) { v.round() as i32 } else { v as i32 };
        let (w, h) = if plant(15759) { (extents.1, extents.0) } else { (extents.0, extents.1) };
        if plant(15760) {
            return (Some(conv(w)), Some(conv(h)));
        }
        (want_width.then_some(conv(w)), want_height.then_some(conv(h)))
    }

    /// `W3DGameWindow::setTextLoc(x, y)` (W3DGameWindow.cpp:598): the position changes only where it differs, and
    /// each change flags `m_newTextPos`.
    pub fn set_text_loc(&mut self, x: i32, y: i32) {
        if plant(15761) {
            self.new_text_pos = true;
        }
        if self.text_pos.0 != x {
            self.text_pos.0 = x;
            self.new_text_pos = !plant(15763) || self.new_text_pos;
        }
        if self.text_pos.1 != y {
            self.text_pos.1 = y;
            self.new_text_pos = !plant(15762) || self.new_text_pos;
        }
    }
}
