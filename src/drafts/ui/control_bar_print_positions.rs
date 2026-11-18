//! UNTESTED DRAFT (#202, #203, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarPrintPositions.cpp
//!     (`PrintInfoRecursive`, `PrintOffsetsFromControlBarParent`)
//!
//! A developer convenience: it writes `ControlBarEasier.txt` (the `ControlBarResizer` blocks of every window of
//! `controlBarHidden.wnd`). The output is built as text; the caller writes the file (upstream's `fopen`).

use crate::client::window_manager::GameWindowManager;
use crate::client::window_script::WinId;

/// `PrintInfoRecursive(win, fp)` (ControlBarPrintPositions.cpp:46): appends to `out` what upstream `fprintf`s, for
/// `win`, then its first child, then its next sibling (the same recursion order).
pub fn print_info_recursive(wm: &GameWindowManager, win: Option<WinId>, out: &mut String) {
    let Some(win) = win else { return };
    let w = wm.win(win);
    // `win->winGetSize`, `winGetPosition`
    let (size_x, size_y) = (w.size.0, w.size.1);
    let (pos_x, pos_y) = (w.region.0, w.region.1);
    let decorated = String::from_utf8_lossy(&w.inst.decorated_name_string).into_owned();
    out.push_str(&format!("ControlBarResizer {}\n", decorated));
    out.push_str(&format!("  AltPosition = X:{} Y:{}\n", pos_x, pos_y));
    out.push_str(&format!("  AltSize = X:{} Y:{}\n", size_x, size_y));
    out.push_str("END\n\n");

    print_info_recursive(wm, w.children.first().copied(), out);
    print_info_recursive(wm, wm.next_of(win), out);
}

/// `PrintOffsetsFromControlBarParent()` (ControlBarPrintPositions.cpp:63): the text for `ControlBarEasier.txt` once the
/// caller has loaded `controlBarHidden.wnd` (upstream: `winCreateLayout`, then `layout->getFirstWindow()`), or `None`
/// when `ControlBar.wnd:ControlBarParent` or the layout is missing (upstream returns before opening the file).
/// The caller destroys the layout after (`destroyWindows`, `deleteInstance`).
pub fn print_offsets_from_control_bar_parent(wm: &GameWindowManager, control_bar_parent_exists: bool, layout_first_window: Option<WinId>, layout_loaded: bool) -> Option<String> {
    if !control_bar_parent_exists {
        return None;
    }
    if !layout_loaded {
        return None;
    }
    let mut out = String::new();
    print_info_recursive(wm, layout_first_window, &mut out);
    Some(out)
}
