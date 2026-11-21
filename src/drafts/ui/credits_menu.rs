//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_credits_menu.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/CreditsMenu.cpp
//!     (the file's statics, `CreditsMenuInit`, `CreditsMenuShutdown`, `CreditsMenuUpdate`, `CreditsMenuInput`,
//!      `CreditsMenuSystem`)
//!
//! `CreditsManager` (Credits.cpp) is outside this file: the host owns `TheCredits` ([`CreditsHost`]).

use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{ggm, gwm, key, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;
use crate::logic::modules::plant;

/// The file's statics: `parentMainMenuID` and `parentMainMenu`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreditsMenu {
    pub parent_main_menu_id: i32,
    pub parent_main_menu: Option<WinId>,
}

/// What the credits menu reaches outside the shell.
pub trait CreditsHost: ShellHost {
    fn credits_menu(&mut self) -> &mut CreditsMenu;
    /// `TheCredits != NULL`.
    fn has_credits(&self) -> bool;
    /// `if(TheCredits) delete TheCredits; TheCredits = new CreditsManager; load(); init();`
    fn credits_create_load_init(&mut self);
    /// `TheCredits->update()`, `isFinished()`.
    fn credits_update(&mut self);
    fn credits_is_finished(&self) -> bool;
    /// `TheCredits->reset(); delete TheCredits; TheCredits = NULL`.
    fn credits_reset_and_delete(&mut self);
    /// `TheAudio->removeAudioEvent(AHSV_StopTheMusicFade)`.
    fn audio_remove_stop_the_music_fade(&mut self);
    /// `AudioEventRTS event("Credits"); event.setShouldFade(TRUE); TheAudio->addAudioEvent(&event)`.
    fn audio_add_credits_music_with_fade(&mut self);
}

/// `CreditsMenuInit(layout, userData)`.
pub fn credits_menu_init<H: CreditsHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    shell::show_shell_map(wm, h, plant(21201));
    h.credits_create_load_init();

    let id = h.name_to_key(b"CreditsMenu.wnd:ParentCreditsWindow") as i32;
    let parent = wm.win_get_window_from_id(None, id);
    h.credits_menu().parent_main_menu_id = id;
    h.credits_menu().parent_main_menu = parent;

    // show menu
    wm.layout_hide(layout, plant(21203));

    // set keyboard focus to main parent
    if !plant(21202) {
        wm.win_set_focus(h, parent);
    }

    h.audio_remove_stop_the_music_fade();
    if !plant(21210) {
        h.audio_add_credits_music_with_fade();
    }
}

/// `CreditsMenuShutdown(layout, userData)`.
pub fn credits_menu_shutdown<H: CreditsHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    h.credits_reset_and_delete();
    shell::show_shell_map(wm, h, true);

    // hide menu
    wm.layout_hide(layout, true);

    // our shutdown is complete
    if !plant(21204) {
        shell::shutdown_complete(wm, h, Some(layout), false);
    }

    h.audio_remove_stop_the_music_fade();
}

/// `CreditsMenuUpdate(layout, userData)`.
pub fn credits_menu_update<H: CreditsHost>(wm: &mut GameWindowManager, h: &mut H, _layout: usize) {
    if h.has_credits() {
        let parent = h.credits_menu().parent_main_menu;
        wm.win_set_focus(h, parent);
        h.credits_update();
        if h.credits_is_finished() != plant(21205) {
            shell::pop(wm, h);
        }
    } else if !plant(21206) {
        shell::pop(wm, h);
    }
}

/// `CreditsMenuInput(window, msg, mData1, mData2)`.
pub fn credits_menu_input<H: CreditsHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
    if msg == gwm::CHAR {
        let k = d1 as u8;
        let state = d2 as u8;
        if k == key::ESC {
            // pop on the release
            if (u32::from(state) & key::STATE_UP != 0) != plant(21207) {
                shell::pop(wm, h);
            }
            // don't let key fall through anywhere else
            return MsgHandled::Handled;
        }
    }
    MsgHandled::Ignored
}

/// `CreditsMenuSystem(window, msg, mData1, mData2)`.
pub fn credits_menu_system<H: CreditsHost>(wm: &mut GameWindowManager, _h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {}
        gwm::DESTROY => {}
        gwm::INPUT_FOCUS => {
            // if we're givin the opportunity to take the keyboard focus we must say we want it
            if d1 == 1 {
                wm.focus_reply = !plant(21208);
            }
            return MsgHandled::Handled;
        }
        ggm::GBM_SELECTED => {}
        _ => {
            if !plant(21209) {
                return MsgHandled::Ignored;
            }
        }
    }
    MsgHandled::Handled
}
