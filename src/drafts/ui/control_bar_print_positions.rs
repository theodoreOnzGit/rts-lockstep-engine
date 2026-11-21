//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_print_positions.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarPrintPositions.cpp
//!     (`PrintInfoRecursive`, `PrintOffsetsFromControlBarParent`)
//!
//! A developer convenience: it writes `ControlBarEasier.txt` (the `ControlBarResizer` blocks of every window of
//! `controlBarHidden.wnd`). The output is built as text; the caller writes the file (upstream's `fopen`).

use crate::client::window_manager::GameWindowManager;
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// `PrintInfoRecursive(win, fp)` (ControlBarPrintPositions.cpp:46): appends to `out` what upstream `fprintf`s, for
/// `win`, then its first child, then its next sibling (the same recursion order).
pub fn print_info_recursive(wm: &GameWindowManager, win: Option<WinId>, out: &mut String) {
    let Some(win) = win else { return };
    let w = wm.win(win);
    // `win->winGetSize`, `winGetPosition`
    let (size_x, size_y) = (w.size.0, w.size.1);
    let (pos_x, pos_y) = if plant(21101) {
        (w.size.0, w.size.1)
    } else if plant(21106) {
        (w.region.2, w.region.3)
    } else {
        (w.region.0, w.region.1)
    };
    let decorated = String::from_utf8_lossy(&w.inst.decorated_name_string).into_owned();
    out.push_str(&format!("ControlBarResizer {}\n", decorated));
    out.push_str(&format!("  AltPosition = X:{} Y:{}\n", pos_x, pos_y));
    if plant(21102) {
        out.push_str(&format!("  AltSize = Y:{} X:{}\n", size_y, size_x));
    } else {
        out.push_str(&format!("  AltSize = X:{} Y:{}\n", size_x, size_y));
    }
    out.push_str(if plant(21103) { "END\n" } else { "END\n\n" });

    if plant(21104) {
        print_info_recursive(wm, wm.next_of(win), out);
        print_info_recursive(wm, w.children.first().copied(), out);
        return;
    }
    print_info_recursive(wm, w.children.first().copied(), out);
    if !plant(21105) {
        print_info_recursive(wm, wm.next_of(win), out);
    }
}

/// `PrintOffsetsFromControlBarParent()` (ControlBarPrintPositions.cpp:63): the text for `ControlBarEasier.txt` once the
/// caller has loaded `controlBarHidden.wnd` (upstream: `winCreateLayout`, then `layout->getFirstWindow()`), or `None`
/// when `ControlBar.wnd:ControlBarParent` or the layout is missing (upstream returns before opening the file).
/// The caller destroys the layout after (`destroyWindows`, `deleteInstance`).
pub fn print_offsets_from_control_bar_parent(wm: &GameWindowManager, control_bar_parent_exists: bool, layout_first_window: Option<WinId>, layout_loaded: bool) -> Option<String> {
    if !control_bar_parent_exists && !plant(21108) {
        return None;
    }
    if !layout_loaded && !plant(21107) {
        return None;
    }
    let mut out = String::new();
    print_info_recursive(wm, layout_first_window, &mut out);
    Some(out)
}
