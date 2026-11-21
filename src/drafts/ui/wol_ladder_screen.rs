//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_wol_small.rs);
//! not yet wired into the game. (Excluded inputs: a missing *Back* button with Escape or `GBM_SELECTED` of a NULL control, which upstream dereferences.) Callbacks before `Init` (the cut's statics left over from the previous case) are not compared.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLLadderScreen.cpp
//!     (the file's statics, `WOLLadderScreenInit/Shutdown/Update/Input/System`)
//!
//! The ladder page was an embedded web browser (`TheWebBrowser`, the in-game IE control), which is the host's.

use crate::logic::modules::plant;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, key, win_data, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WolLadderScreen {
    pub parent_window_id: i32,
    pub button_back_id: i32,
    pub window_ladder_id: i32,
    pub parent_window: Option<WinId>,
    pub button_back: Option<WinId>,
    pub window_ladder: Option<WinId>,
}

/// What the ladder screen needs of the host.
pub trait WolLadderScreenHost: ShellHost {
    fn wol_ladder_screen(&mut self) -> &mut WolLadderScreen;
    /// `TheWebBrowser != NULL`.
    fn has_web_browser(&self) -> bool;
    /// `TheWebBrowser->createBrowserWindow("MessageBoard", window)`.
    fn web_browser_create_window(&mut self, name: &[u8], window: Option<WinId>);
    /// `TheWebBrowser->closeBrowserWindow(window)`.
    fn web_browser_close_window(&mut self, window: Option<WinId>);
}

/// `WOLLadderScreenInit(layout, userData)`.
pub fn wol_ladder_screen_init<H: WolLadderScreenHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if !plant(20678) {
        shell::show_shell_map(wm, h, !plant(20679));
    }

    let ids = [
        ShellHost::name_to_key(h, b"WOLLadderScreen.wnd:LadderParent") as i32,
        ShellHost::name_to_key(h, b"WOLLadderScreen.wnd:ButtonBack") as i32,
        ShellHost::name_to_key(h, b"WOLLadderScreen.wnd:WindowLadder") as i32,
    ];
    let parent = wm.win_get_window_from_id(None, ids[0]);
    let back = wm.win_get_window_from_id(parent, ids[1]);
    let ladder = wm.win_get_window_from_id(parent, ids[2]);
    {
        let s = h.wol_ladder_screen();
        s.parent_window_id = ids[0];
        s.button_back_id = ids[1];
        s.window_ladder_id = ids[2];
        s.parent_window = parent;
        s.button_back = back;
        s.window_ladder = ladder;
    }

    if h.has_web_browser() || plant(20680) {
        h.web_browser_create_window(b"MessageBoard", ladder);
    }

    wm.layout_hide(layout, false);
    wm.win_set_focus(h, parent);
}

/// `WOLLadderScreenShutdown(layout, userData)`.
pub fn wol_ladder_screen_shutdown<H: WolLadderScreenHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    if h.has_web_browser() {
        let w = if plant(20681) { h.wol_ladder_screen().button_back } else { h.wol_ladder_screen().window_ladder };
        h.web_browser_close_window(w);
    }

    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `WOLLadderScreenUpdate(layout, userData)`: empty.
pub fn wol_ladder_screen_update<H: WolLadderScreenHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `WOLLadderScreenInput(window, msg, mData1, mData2)`: Escape's release presses *Back*.
pub fn wol_ladder_screen_input<H: WolLadderScreenHost>(wm: &mut GameWindowManager, h: &mut H, window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR && (d1 & 0xFF) as u8 == key::ESC {
        if (d2 & 0xFF) & key::STATE_UP != 0 {
            let s = h.wol_ladder_screen().clone();
            wm.win_send_system_msg(h, Some(window), ggm::GBM_SELECTED, win_data(s.button_back), if plant(20684) { 0 } else { s.button_back_id } as u32);
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `WOLLadderScreenSystem(window, msg, mData1, mData2)`.
pub fn wol_ladder_screen_system<H: WolLadderScreenHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE | gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            if d1 == 1 {
                wm.focus_reply = true;
            }
            return MsgHandled::Handled;
        }
        ggm::GBM_SELECTED => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            if wm.tree.windows[control].id == (if plant(20682) { h.wol_ladder_screen().parent_window_id } else { h.wol_ladder_screen().button_back_id }) {
                if !plant(20683) {
                    shell::pop(wm, h);
                }
            }
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
