//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarResizer.cpp (ControlBarResizer::findResizerWindow,
//!   newResizerWindow, sizeWindowsDefault, sizeWindowsAlt)
//!
//! The resizer remembers the default position and size of the control bar's windows and puts them back, or scales the
//! alternate (`ResizerWindow` INI) layout to the display. `m_altPos` / `m_altSize` are filled by the INI parser
//! (`parseResizerWindow`, not in this sweep's list).

use crate::client::window_manager::{GameWindowManager, GuiHost};

/// `class ResizerWindow`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResizerWindow {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_defaultPos`, `m_defaultSize`.
    pub default_pos: (i32, i32),
    pub default_size: (i32, i32),
    /// `m_altPos`, `m_altSize`.
    pub alt_pos: (i32, i32),
    pub alt_size: (i32, i32),
}

/// `class ControlBarResizer`'s list.
#[derive(Clone, Debug, Default)]
pub struct ControlBarResizer {
    /// `m_resizerWindowsList`, in order.
    pub resizer_windows_list: Vec<ResizerWindow>,
}

/// `TheWindowManager->winGetWindowFromId(NULL, TheNameKeyGenerator->nameToKey(name))`.
fn window_by_name(wm: &GameWindowManager, name: &[u8]) -> Option<usize> {
    wm.tree.find_by_name(name)
}

impl ControlBarResizer {
    /// `findResizerWindow(name)` (ControlBarResizer.cpp:111): the first with that name, by its place.
    pub fn find_resizer_window(&self, name: &[u8]) -> Option<usize> {
        self.resizer_windows_list.iter().position(|r| r.name.as_slice() == name)
    }

    /// `newResizerWindow(name)` (:134): the window of that name must already be loaded; its current position and size
    /// are the defaults. A window that is not found is a `DEBUG_ASSERTCRASH` and gives `None`. Added at the back.
    pub fn new_resizer_window(&mut self, wm: &GameWindowManager, name: &[u8]) -> Option<usize> {
        let mut new_rwin = ResizerWindow { name: name.to_vec(), ..ResizerWindow::default() };
        let win = window_by_name(wm, name)?;
        let w = wm.win(win);
        new_rwin.default_pos = (w.region.0, w.region.1);
        new_rwin.default_size = w.size;
        self.resizer_windows_list.push(new_rwin);
        Some(self.resizer_windows_list.len() - 1)
    }

    /// `sizeWindowsDefault()` (:154): each window found goes back to its default position and size.
    pub fn size_windows_default<H: GuiHost>(&self, wm: &mut GameWindowManager, h: &mut H) {
        for r in &self.resizer_windows_list {
            let Some(win) = window_by_name(wm, &r.name) else { continue };
            wm.win_set_position(win, r.default_pos.0, r.default_pos.1);
            wm.win_set_size(h, win, r.default_size.0, r.default_size.1);
            // DEBUG_LOG(("sizeWindowsDefault:%s pos X:%d pos Y: %d size X:%d sizeY: %d", ..))
        }
    }

    /// `sizeWindowsAlt()` (:179): each window found goes to its alternate position scaled by the display's size over
    /// 800 x 600 (`Real` products truncated to `Int` by `winSetPosition`); the size likewise when either alternate
    /// extent is positive.
    pub fn size_windows_alt<H: GuiHost>(&self, wm: &mut GameWindowManager, h: &mut H) {
        let x = wm.display.0 as f32 / 800.0;
        let y = wm.display.1 as f32 / 600.0;
        for r in &self.resizer_windows_list {
            let Some(win) = window_by_name(wm, &r.name) else { continue };
            wm.win_set_position(win, (r.alt_pos.0 as f32 * x) as i32, (r.alt_pos.1 as f32 * y) as i32);
            if r.alt_size.0 > 0 || r.alt_size.1 > 0 {
                wm.win_set_size(h, win, (r.alt_size.0 as f32 * x) as i32, (r.alt_size.1 as f32 * y) as i32);
            }
            // DEBUG_LOG(("sizeWindowsAlt:%s ..", ..))
        }
    }
}
