//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameWindow.cpp (W3DGameWindow::getTextSize,
//!   setTextLoc)
//!
//! A W3D window draws its text with a `Render2DSentenceClass` (`m_textRenderer`); the sentence is rebuilt when the
//! text position changes (`m_newTextPos`).

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
        (want_width.then_some(extents.0 as i32), want_height.then_some(extents.1 as i32))
    }

    /// `W3DGameWindow::setTextLoc(x, y)` (W3DGameWindow.cpp:598): the position changes only where it differs, and
    /// each change flags `m_newTextPos`.
    pub fn set_text_loc(&mut self, x: i32, y: i32) {
        if self.text_pos.0 != x {
            self.text_pos.0 = x;
            self.new_text_pos = true;
        }
        if self.text_pos.1 != y {
            self.text_pos.1 = y;
            self.new_text_pos = true;
        }
    }
}
