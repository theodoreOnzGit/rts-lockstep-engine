//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDebugDisplay.cpp
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DDebugDisplay.h
//!
//! A debug overlay: it draws each text line through a `DisplayString` (white, with a black drop shadow).

use rts_lockstep_engine::client::gadget_list_box::DisplayString;
use rts_lockstep_engine::common::ini::game_make_color;

/// The draw call `DisplayString::draw(x, y, color, dropColor)`, which the display string manager implements.
pub trait DrawDisplayString {
    fn draw(&mut self, s: &DisplayString, x: i32, y: i32, color: i32, drop_color: i32);
}

/// `class W3DDebugDisplay`.
#[derive(Clone, Debug, Default)]
pub struct W3DDebugDisplay {
    /// `m_font` (the font's name; None for `NULL`).
    pub font: Option<Vec<u8>>,
    pub font_width: i32,
    pub font_height: i32,
    /// `m_displayString` (None for `NULL`).
    pub display_string: Option<DisplayString>,
}

impl W3DDebugDisplay {
    /// `W3DDebugDisplay::W3DDebugDisplay` (W3DDebugDisplay.cpp:78): `m_displayString(NULL)`.
    pub fn new() -> Self {
        W3DDebugDisplay::default()
    }

    /// `W3DDebugDisplay::~W3DDebugDisplay` (W3DDebugDisplay.cpp:86): `freeDisplayString`.
    pub fn destroy(&mut self) {
        self.display_string = None;
    }

    /// `W3DDebugDisplay::init` (W3DDebugDisplay.cpp:99): `newDisplayString()`.
    pub fn init(&mut self) {
        self.display_string = Some(DisplayString::default());
    }

    /// `W3DDebugDisplay::drawText` (W3DDebugDisplay.cpp:109): the text at character cell (`x`, `y`).
    pub fn draw_text(&mut self, x: i32, y: i32, text: &[u8], out: &mut dyn DrawDisplayString) {
        if self.font.is_none() || self.display_string.is_none() {
            return;
        }
        let text_color = game_make_color(255, 255, 255, 255);
        let drop_color = game_make_color(0, 0, 0, 255);
        let ds = self.display_string.as_mut().unwrap();
        ds.text = text.iter().map(|&b| b as u16).collect(); // UnicodeString::translate(AsciiString)
        out.draw(ds, x * self.font_width, 13 + y * self.font_height, text_color, drop_color);
    }

    /// `W3DDebugDisplay::setFont` (W3DDebugDisplay.cpp:128).
    pub fn set_font(&mut self, font: Option<Vec<u8>>) {
        self.font = font.clone();
        if let Some(ds) = self.display_string.as_mut() {
            ds.font = None; // m_displayString->setFont(m_font): the font handle is the caller's (`GameFont`)
        }
    }

    /// `setFontWidth`, `setFontHeight` (W3DDebugDisplay.h).
    pub fn set_font_width(&mut self, width: i32) {
        self.font_width = width;
    }
    pub fn set_font_height(&mut self, height: i32) {
        self.font_height = height;
    }
}
