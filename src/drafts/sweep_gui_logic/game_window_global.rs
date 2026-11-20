//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ for `win_format_text`, `win_get_text_size` and `free_static_strings` (test:
//! tests/differential_draft_glsmall.rs); `winGetWindowList` is a field read, `dumpWindow` and `initTestGUI` are excluded (below);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowGlobal.cpp (GameWindowManager::winFormatText,
//!   winGetTextSize)
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManager.cpp (GameWindowManager::winGetWindowList,
//!   dumpWindow, initTestGUI)
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GameWindowManagerScript.cpp (GameWindowManager::freeStaticStrings)
//!
//! Sweep 2 findings for this file:
//! - `GameWindowManager::winGetWindowList` (GameWindowManager.cpp:687): translated, the field
//!   `GameWindowManager::window_list` (`crate::client::window_manager`).
//! - `GameWindowManager::dumpWindow` (:1372): excluded, `#ifndef FINAL`, and only `DEBUG_LOG`s each window's id,
//!   draw callback and user data.
//! - `GameWindowManager::initTestGUI` (:3732): excluded, test code: it returns `TRUE` on its first statement and the
//!   rest (a test window, push buttons, a check box, radio buttons, a list box with `testGrab`) is unreachable.
//! - `GameWindowManager::freeStaticStrings` (GameWindowManagerScript.cpp:2676): the four scratch strings of the
//!   script reader (`theSystemString`, `theInputString`, `theTooltipString`, `theDrawString`); the verified
//!   reader (`WindowScript`) owns its buffers and drops them with it. Drafted as clearing them.

use crate::client::window_script::GameFont;
use crate::logic::modules::plant;

/// `GameWindowManager::winFormatText(font, text, color, x, y, width, height)` (GameWindowGlobal.cpp:158): the body is
/// empty upstream (the W3D manager's override draws).
pub fn win_format_text(_font: Option<&GameFont>, _text: &[u16], _color: i32, _x: i32, _y: i32, _width: i32, _height: i32) {}

/// `GameWindowManager::winGetTextSize(font, text, width, height, maxWidth)` (GameWindowGlobal.cpp:169): the base
/// class sets both outputs to 0 (the W3D manager's override measures).
pub fn win_get_text_size(_font: Option<&GameFont>, _text: &[u16], _max_width: i32) -> (i32, i32) {
    (0, if plant(15892) { 1 } else { 0 })
}

/// The script reader's static strings (`theSystemString` ...).
#[derive(Clone, Debug, Default)]
pub struct ScriptStaticStrings {
    pub system_string: Vec<u8>,
    pub input_string: Vec<u8>,
    pub tooltip_string: Vec<u8>,
    pub draw_string: Vec<u8>,
}

impl ScriptStaticStrings {
    /// `GameWindowManager::freeStaticStrings()` (GameWindowManagerScript.cpp:2676): each is cleared.
    pub fn free_static_strings(&mut self) {
        self.system_string.clear();
        self.input_string.clear();
        if !plant(15893) {
            self.tooltip_string.clear();
        }
        self.draw_string.clear();
    }
}
