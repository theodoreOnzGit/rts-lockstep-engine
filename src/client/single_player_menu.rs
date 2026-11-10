// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SinglePlayerMenu.cpp (the file's statics,
//     shutdownComplete, SinglePlayerMenuInit, SinglePlayerMenuShutdown, SinglePlayerMenuUpdate, SinglePlayerMenuInput,
//     SinglePlayerMenuSystem)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   SinglePlayerMenu.cpp: Colin Day, 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The single player menu (GitHub #78, item 3): `Menus/SinglePlayerMenu.wnd`'s callbacks.
//!
//! # As upstream reads it
//!
//! **Init** shows the shell map and the layout, gives `SinglePlayerMenuParent` the focus, and slides three buttons in
//! (`registerWithAnimateManager`: *New* from the left after 1 ms, *Load* from the left after 200 ms, *Back* from the
//! right after 1 ms). **System**: the window's `GWM_CREATE` takes the three buttons' keys; `GBM_SELECTED`, unless a
//! button was pushed: *New* pushes `Menus/MapSelectMenu.wnd`, *Load* does nothing, *Back* pops the menu. **Input**:
//! Escape's release (unless a button was pushed) presses *Back*. **Shutdown**: immediate, done at once; else the
//! animations reversed, and the **Update** completes it when they are done (the layout hidden, the shell told).
//!
//! **Not reachable in Zero Hour:** nothing pushes `Menus/SinglePlayerMenu.wnd` (the main menu shows its own single
//! player panel). It is translated as the lexicon names it; the tests push it themselves.
//!
//! # Upstream quirks that are kept
//!
//! - `buttonPushed` stays set after *New* or *Back* until the next init.
//! - Init's `buttonPushed` and `isShuttingDown` are reset, but the system's keys are set only by `GWM_CREATE`.
//! - The shutdown marks itself shutting down before it reads the user data (a `NULL` there is recorded as undefined).
//!
//! # Where this differs from upstream
//!
//! - `Menus/MapSelectMenu.wnd` (MapSelectMenu.cpp, the single player map list) is not translated: the push loads the
//!   file if it is there and its callbacks are recorded as untranslated ([`crate::client::shell_gui::ShellGui`]).
//!
//! # Representation
//!
//! [`SinglePlayerMenu`]: the file's statics, with the system callback's function statics.

use crate::client::animate_window::anim;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// What the single player menu needs besides the shell.
pub trait SinglePlayerHost: ShellHost {
    /// SinglePlayerMenu.cpp's statics.
    fn single_player_menu(&mut self) -> &mut SinglePlayerMenu;
}

/// SinglePlayerMenu.cpp's statics and `SinglePlayerMenuSystem`'s function statics (`NAMEKEY_INVALID` is 0).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SinglePlayerMenu {
    pub button_pushed: bool,
    pub is_shutting_down: bool,
    pub button_new: i32,
    pub button_load: i32,
    pub button_back: i32,
}

fn key_of<H: ShellHost>(h: &mut H, name: &str) -> i32 {
    h.name_to_key(format!("SinglePlayerMenu.wnd:{name}").as_bytes()) as i32
}

/// The file's `shutdownComplete(layout)`.
fn menu_shutdown_complete<H: SinglePlayerHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.single_player_menu().is_shutting_down = false;
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `SinglePlayerMenuInit(layout, userData)`.
pub fn single_player_menu_init<H: SinglePlayerHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    shell::show_shell_map(wm, h, true);
    let m = h.single_player_menu();
    m.button_pushed = false;
    m.is_shutting_down = false;
    wm.layout_hide(layout, false);
    let parent_id = key_of(h, "SinglePlayerMenuParent");
    let parent = wm.win_get_window_from_id(None, parent_id);
    wm.win_set_focus(h, parent);
    for (name, anim_type, delay) in [("ButtonNew", anim::WIN_ANIMATION_SLIDE_LEFT, 1), ("ButtonLoad", anim::WIN_ANIMATION_SLIDE_LEFT, 200), ("ButtonBack", anim::WIN_ANIMATION_SLIDE_RIGHT, 1)] {
        let id = key_of(h, name);
        let w = wm.win_get_window_from_id(None, id);
        shell::register_with_animate_manager(wm, h, w, anim_type, true, delay);
    }
}

/// `SinglePlayerMenuShutdown(layout, userData)`: `immediate` is `*(Bool *)userData`.
pub fn single_player_menu_shutdown<H: SinglePlayerHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize, immediate: Option<bool>) {
    h.single_player_menu().is_shutting_down = true;
    let Some(pop_immediate) = immediate else {
        wm.set_undefined("SinglePlayerMenuShutdown: no user data (*(Bool *)NULL)");
        return;
    };
    if pop_immediate {
        menu_shutdown_complete(wm, h, layout);
        return;
    }
    shell::reverse_animate_window(wm, h);
}

/// `SinglePlayerMenuUpdate(layout, userData)`.
pub fn single_player_menu_update<H: SinglePlayerHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.single_player_menu().is_shutting_down && shell::is_anim_finished(wm, h) {
        menu_shutdown_complete(wm, h, layout);
    }
}

/// `SinglePlayerMenuInput(window, msg, mData1, mData2)`.
pub fn single_player_menu_input<H: SinglePlayerHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg != gwm::CHAR || h.single_player_menu().button_pushed {
        return MsgHandled::Ignored;
    }
    if (d1 & 0xFF) as u8 != key::ESC {
        return MsgHandled::Ignored;
    }
    if (d2 & 0xFF) & key::STATE_UP != 0 {
        let id = key_of(h, "ButtonBack");
        let button = wm.win_get_window_from_id(Some(window), id);
        wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(button), id as u32);
    }
    MsgHandled::Handled
}

/// `SinglePlayerMenuSystem(window, msg, mData1, mData2)`.
pub fn single_player_menu_system<H: SinglePlayerHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {
            let (n, l, b) = (key_of(h, "ButtonNew"), key_of(h, "ButtonLoad"), key_of(h, "ButtonBack"));
            let m = h.single_player_menu();
            m.button_new = n;
            m.button_load = l;
            m.button_back = b;
        }
        gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let id = wm.tree.windows[control].id;
            let m = h.single_player_menu().clone();
            if m.button_pushed {
                return MsgHandled::Handled;
            }
            if id == m.button_new {
                shell::push(wm, h, b"Menus/MapSelectMenu.wnd", false);
                h.single_player_menu().button_pushed = true;
            } else if id == m.button_load {
            } else if id == m.button_back {
                shell::pop(wm, h);
                h.single_player_menu().button_pushed = true;
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::save_load_menu::tests::{booted, click, named, settle, tick};

    /// #78: the single player menu, pushed by the test (nothing in Zero Hour pushes it), predicted from
    /// SinglePlayerMenu.cpp before the run:
    ///
    /// - the push runs the main menu's shutdown, then (once it completes) the menu's init: its layout shown, the three
    ///   buttons registered with the animate manager (with `m_animateWindows`), the system's keys set by `GWM_CREATE`;
    /// - *New* pushes `Menus/MapSelectMenu.wnd` (pending, the menu's shutdown asked) and sets `buttonPushed`; the next
    ///   *Back* is ignored;
    /// - pushed again (after the test pops the pending push away), Escape's release presses *Back*: `buttonPushed`,
    ///   the shutdown reverses the animations, and once they are done the update completes it: the menu popped, the
    ///   main menu the top again.
    ///
    /// **Result (2026-11-10):** as predicted.
    #[test]
    fn the_single_player_menu_pushes_and_pops() {
        let (mut wm, mut h) = booted();
        shell::push(&mut wm, &mut h, b"Menus/SinglePlayerMenu.wnd", false);
        settle(&mut wm, &mut h);
        settle(&mut wm, &mut h);
        let top = h.shell.top().expect("top");
        assert_eq!(wm.layouts[top].filename, b"Menus/SinglePlayerMenu.wnd");
        assert!(!wm.layouts[top].hidden);
        let f = "SinglePlayerMenu.wnd";
        let k = h.name_to_key(b"SinglePlayerMenu.wnd:ButtonNew") as i32;
        assert_eq!(h.single_player.button_new, k);
        assert!(h.single_player.button_back != 0 && h.single_player.button_load != 0);
        let new = named(&wm, f, "ButtonNew");
        click(&mut wm, &mut h, new);
        assert!(h.single_player.button_pushed && h.shell.pending_push);
        assert_eq!(h.shell.pending_push_name, b"Menus/MapSelectMenu.wnd");
        let back = named(&wm, f, "ButtonBack");
        click(&mut wm, &mut h, back);
        assert!(!h.shell.pending_pop, "buttonPushed: Back ignored");
        // (the map select menu is not translated: the test drops the pending push and pops the menu at once)
        h.shell.pending_push = false;
        h.shell.pending_push_name.clear();
        shell::pop_immediate(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 1);
        shell::push(&mut wm, &mut h, b"Menus/SinglePlayerMenu.wnd", true);
        settle(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 2);
        assert!(!h.single_player.button_pushed);
        let p = wm.layouts[h.shell.top().expect("top")].windows[0];
        wm.win_set_focus(&mut h, Some(p));
        wm.win_process_key(&mut h, key::ESC, key::STATE_UP);
        assert!(h.single_player.button_pushed && h.single_player.is_shutting_down && h.shell.pending_pop);
        settle(&mut wm, &mut h);
        tick(&mut wm, &mut h);
        assert_eq!(h.shell.get_screen_count(), 1);
        assert_eq!(wm.layouts[h.shell.top().expect("top")].filename, b"Menus/MainMenu.wnd");
        assert!(wm.undefined.is_none(), "{:?}", wm.undefined);
        assert!(!h.untranslated.iter().any(|f| f.starts_with(b"SinglePlayer")), "{:?}", h.untranslated);
        // a shutdown without its user data: upstream's `*(Bool *)NULL`
        single_player_menu_shutdown(&mut wm, &mut h, 0, None);
        assert!(wm.undefined.is_some());
    }
}
