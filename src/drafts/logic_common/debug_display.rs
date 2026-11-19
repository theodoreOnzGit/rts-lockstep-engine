//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_debug_display.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameClient/DebugDisplay.h
//!   GeneralsMD/Code/GameEngine/Source/GameClient/System/DebugDisplay.cpp
//!
//! The text console the debug overlays print into: a cursor, margins and a colour, with `printf` splitting at newlines and handing
//! each line to the device's `drawText`. `drawText` is the device's ([`DebugDisplayDevice`]); the C varargs `printf` takes the
//! already-formatted text.

use crate::logic::modules::plant;

/// `DebugDisplayInterface::Color` (DebugDisplay.h): `WHITE, BLACK, YELLOW, RED, GREEN, BLUE, NUM_COLORS`. The .cpp names only `WHITE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugDisplayColor {
    /// `WHITE`.
    White,
    /// `BLACK`.
    Black,
    /// `YELLOW`.
    Yellow,
    /// `RED`.
    Red,
    /// `GREEN`.
    Green,
    /// `BLUE`.
    Blue,
    /// `NUM_COLORS`.
    NumColors,
}

/// `DebugDisplay::drawText(x, y, text)`, the pure virtual the device implements.
pub trait DebugDisplayDevice {
    /// Draw `text` at character cell (`x`, `y`).
    fn draw_text(&mut self, x: i32, y: i32, text: &[u8]);
}

/// `class DebugDisplay`'s data.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugDisplay {
    /// `m_textColor`.
    pub text_color: DebugDisplayColor,
    /// `m_xPos`.
    pub x_pos: i32,
    /// `m_yPos`.
    pub y_pos: i32,
    /// `m_width`.
    pub width: i32,
    /// `m_height`.
    pub height: i32,
    /// `m_rightMargin`.
    pub right_margin: i32,
    /// `m_leftMargin`.
    pub left_margin: i32,
}

impl DebugDisplay {
    /// `DebugDisplay::DebugDisplay()`: `m_width(0), m_height(0)` then `reset()`.
    pub fn new() -> DebugDisplay {
        let mut d = DebugDisplay { text_color: DebugDisplayColor::White, x_pos: 0, y_pos: 0, width: 0, height: 0, right_margin: 0, left_margin: 0 };
        d.reset();
        d
    }

    /// `DebugDisplay::reset()`: cursor home, white, `setRightMargin(0)`, `setLeftMargin(getWidth())`.
    pub fn reset(&mut self) {
        self.set_cursor_pos(0, 0);
        self.set_text_color(DebugDisplayColor::White);
        self.set_right_margin(0);
        let w = self.get_width();
        if !plant(15287) {
            self.set_left_margin(w);
        }
    }

    /// `setCursorPos(x, y)`.
    pub fn set_cursor_pos(&mut self, x: i32, y: i32) {
        if plant(15289) {
            self.x_pos = y;
            self.y_pos = x;
        } else {
            self.x_pos = x;
            self.y_pos = y;
        }
    }
    /// `getCursorXPos()`.
    pub fn get_cursor_x_pos(&self) -> i32 {
        self.x_pos
    }
    /// `getCursorYPos()`.
    pub fn get_cursor_y_pos(&self) -> i32 {
        self.y_pos
    }
    /// `getWidth()`.
    pub fn get_width(&self) -> i32 {
        self.width
    }
    /// `getHeight()`.
    pub fn get_height(&self) -> i32 {
        self.height
    }
    /// `setTextColor(color)`.
    pub fn set_text_color(&mut self, color: DebugDisplayColor) {
        self.text_color = color;
    }
    /// `setRightMargin(rightPos)`.
    pub fn set_right_margin(&mut self, right_pos: i32) {
        self.right_margin = right_pos;
    }
    /// `setLeftMargin(leftPos)`.
    pub fn set_left_margin(&mut self, left_pos: i32) {
        self.left_margin = left_pos;
    }

    /// `DebugDisplay::printf(format, ...)` on the formatted text (a 5 KiB static buffer upstream; the overflow `DEBUG_ASSERTCRASH` is
    /// compiled out of release). Walks the text: each `\n` ends a line, which is drawn at (`m_rightMargin + m_xPos`, `m_yPos`) only if
    /// non-empty, then `m_yPos++` and `m_xPos = 0`; text after the last newline is drawn and advances `m_xPos` by its length.
    pub fn printf<D: DebugDisplayDevice>(&mut self, dev: &mut D, text: &[u8]) {
        // find every line and print it
        let mut line_start = 0usize;
        let mut line_len: i32 = 0;
        for (i, &ch) in text.iter().enumerate() {
            if ch == 0 {
                break;
            }
            if ch == b'\n' {
                if line_len > 0 || plant(15281) {
                    dev.draw_text(self.right_margin + if plant(15285) { 0 } else { self.x_pos }, self.y_pos, &text[line_start..i]);
                    if !plant(15290) {
                        line_len = 0;
                    }
                }
                line_start = i + 1;
                if !plant(15284) {
                    self.y_pos += 1;
                }
                if !plant(15283) {
                    self.x_pos = 0;
                }
            } else {
                line_len += 1;
            }
        }
        if line_len > 0 {
            let end = text.iter().position(|&b| b == 0).unwrap_or(text.len());
            dev.draw_text(self.right_margin + self.x_pos, self.y_pos, &text[line_start..end]);
            if !plant(15282) {
                self.x_pos += line_len;
            }
        }
    }
}

impl Default for DebugDisplay {
    fn default() -> Self {
        DebugDisplay::new()
    }
}
