//! UNTESTED DRAFT (#202, #205): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/GUI/GUICallbacks/W3DMOTD.cpp

use rts_lockstep_engine::client::window_manager::{data_win, ggm, gwm, GameWindowManager, MsgHandled, WindowMsgData};
use rts_lockstep_engine::client::window_script::WinId;

/// `NAMEKEY_INVALID`.
pub const NAMEKEY_INVALID: i32 = -1;

/// The one static of the file, `closeButtonID`, and `TheNameKeyGenerator->nameToKey`.
#[derive(Clone, Copy, Debug)]
pub struct MotdState {
    /// `static NameKeyType closeButtonID`.
    pub close_button_id: i32,
}

impl Default for MotdState {
    fn default() -> Self {
        MotdState { close_button_id: NAMEKEY_INVALID }
    }
}

/// `MOTDSystem(window, msg, mData1, mData2)` (W3DMOTD.cpp:75): the message of the day's window system.
/// `name_to_key`: `TheNameKeyGenerator->nameToKey(name)`.
pub fn motd_system(wm: &mut GameWindowManager, state: &mut MotdState, name_to_key: &mut dyn FnMut(&[u8]) -> i32, window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {
            // load id's needed
            state.close_button_id = name_to_key(b"MOTD.wnd:CloseMOTD");
        }
        gwm::DESTROY => {}
        ggm::GBM_SELECTED => {
            if let Some(control) = data_win(d1) {
                let control_id = wm.tree.windows[control].id;
                if control_id == state.close_button_id {
                    let hidden = wm.tree.windows[window].is_hidden();
                    wm.win_hide(window, !hidden); // window->winHide(!window->winIsHidden())
                }
            }
            // upstream dereferences a NULL control: undefined, not modelled here
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
