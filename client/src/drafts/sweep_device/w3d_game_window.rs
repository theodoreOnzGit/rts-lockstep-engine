//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/W3DGameWindow.cpp
//!
//! Sweep 2 (#213): `W3DGameWindow::getTextSize` and `setTextLoc`.

/// The text members of `W3DGameWindow`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct W3DGameWindowText {
    /// `m_textPos`.
    pub text_pos: (i32, i32),
    /// `m_newTextPos`.
    pub new_text_pos: bool,
}

impl W3DGameWindowText {
    /// `W3DGameWindow::getTextSize` (W3DGameWindow.cpp:584): the window text's extents from the text renderer
    /// (`Get_Text_Extents`, a `Vector2`), truncated to `Int` on assignment. Each output is only written when asked for.
    pub fn get_text_size(extents: (f32, f32), want_width: bool, want_height: bool) -> (Option<i32>, Option<i32>) {
        (if want_width { Some(extents.0 as i32) } else { None }, if want_height { Some(extents.1 as i32) } else { None })
    }

    /// `W3DGameWindow::setTextLoc` (W3DGameWindow.cpp:598): moves the text position, flagging that the text must be
    /// re-laid-out when either coordinate changes.
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
