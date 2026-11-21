//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_wol_small.rs);
//! not yet wired into the game. (Excluded inputs: OK pressed with no list box, where upstream's `int selected` is unset.) Callbacks before `Init` (the cut's statics left over from the previous case) are not compared.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/WOLLocaleSelectPopup.cpp
//!     (the file's statics, `WOLLocaleSelectInit/Shutdown/Update/Input/System`)
//!
//! The GameSpy service is gone: `TheGameSpyInfo`, `TheGameSpyPSMessageQueue` (the persistent-storage queue) and
//! `GameSpyMiscPreferences` are the host's. The *OK* handler's body is translated step by step against that host.

use crate::logic::modules::plant;
use crate::client::gadget_list_box as lb;
use crate::client::shell::{self, ShellHost};
use crate::client::window_manager::{data_win, ggm, gwm, GameWindowManager, MsgHandled, WindowMsgData};
use crate::client::window_script::WinId;

/// `LOC_MIN`, `LOC_MAX` (GameSpy locale ids, PersistentStorageDefs.h `enum LocaleType`: `LOC_MIN = 1`, `LOC_MAX = 37`).
/// ~~`LOC_MAX` was 20~~ **Corrected 2026-11-21 (#215, group ui3a):** 37 (the draft's 20 left `WOL:Locale21` to `37` out of the list).
pub const LOC_MIN: i32 = 1;
pub const LOC_MAX: i32 = 37;

/// The file's statics (`NAMEKEY_INVALID` is 0, `NULL` is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WolLocaleSelect {
    pub parent_locale_select_id: i32,
    pub button_ok_id: i32,
    pub listbox_locale_id: i32,
    pub parent_locale_select: Option<WinId>,
    pub button_ok: Option<WinId>,
    pub listbox_locale: Option<WinId>,
}

/// The `PSPlayerStats` fields this screen touches.
#[derive(Clone, Debug, Default)]
pub struct LocaleStats {
    pub id: i32,
    pub locale: i32,
}

/// What the locale popup reaches in the GameSpy layer.
pub trait WolLocaleSelectHost: ShellHost {
    fn wol_locale_select(&mut self) -> &mut WolLocaleSelect;
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `GameSpyColor[GSCOLOR_DEFAULT]`.
    fn gamespy_color_default(&self) -> i32;
    /// `winSetModal(window)`.
    fn win_set_modal(&mut self, wm: &mut GameWindowManager, w: Option<WinId>);
    /// `PSRequest psReq` with `PSREQUEST_UPDATEPLAYERLOCALE`, filled from `TheGameSpyInfo` (email, nick, password,
    /// profile id) and queued with `TheGameSpyPSMessageQueue->addRequest`.
    fn ps_add_update_player_locale_request(&mut self, locale: i32);
    /// `GameSpyCloseOverlay(GSOVERLAY_LOCALESELECT)`.
    fn gamespy_close_overlay_locale_select(&mut self, wm: &mut GameWindowManager);
    /// `GameSpyMiscPreferences cPref; cPref.setLocale(locale); cPref.write()`.
    fn misc_prefs_set_locale_and_write(&mut self, locale: i32);
    /// `TheGameSpyInfo->getLocalProfileID()`.
    fn local_profile_id(&self) -> i32;
    /// `TheGameSpyPSMessageQueue->findPlayerStatsByID(id)`, `trackPlayerStats(stats)`.
    fn ps_find_player_stats_by_id(&self, id: i32) -> LocaleStats;
    fn ps_track_player_stats_locale(&mut self, stats: &LocaleStats);
    /// `TheGameSpyInfo->getCachedLocalPlayerStats()` / `setCachedLocalPlayerStats`: set the cached stats' locale.
    fn gamespy_set_cached_local_player_stats_locale(&mut self, locale: i32);
    /// `PSResponse` of `PSRESPONSE_PLAYERSTATS` for the local player, added with `addResponse`.
    fn ps_add_player_stats_response_for_local_player(&mut self);
    /// `CheckReOpenPlayerInfo()`.
    fn check_re_open_player_info(&mut self, wm: &mut GameWindowManager);
}

/// `WOLLocaleSelectInit(layout, userData)`.
pub fn wol_locale_select_init<H: WolLocaleSelectHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    let ids = [
        ShellHost::name_to_key(h, b"PopupLocaleSelect.wnd:ParentLocaleSelect") as i32,
        ShellHost::name_to_key(h, b"PopupLocaleSelect.wnd:ButtonOk") as i32,
        ShellHost::name_to_key(h, b"PopupLocaleSelect.wnd:ListBoxLocaleSelect") as i32,
    ];
    let parent = wm.win_get_window_from_id(None, ids[0]);
    let ok = wm.win_get_window_from_id(None, ids[1]);
    let list = wm.win_get_window_from_id(None, ids[2]);
    {
        let s = h.wol_locale_select();
        s.parent_locale_select_id = ids[0];
        s.button_ok_id = ids[1];
        s.listbox_locale_id = ids[2];
        s.parent_locale_select = parent;
        s.button_ok = ok;
        s.listbox_locale = list;
    }

    if let Some(l) = list {
        let color = if plant(20685) { 0 } else { h.gamespy_color_default() };
        for i in LOC_MIN..=LOC_MAX {
            let id = format!("WOL:Locale{i:02}"); // "%2.2d"
            let text = h.fetch_text(id.as_bytes());
            lb::add_entry_text(wm, h, l, &text, color, -1, -1, false);
        }
        lb::set_selected(wm, h, l, &[i32::from(plant(20686))]);
    }

    wm.layout_hide(layout, false);
    wm.win_set_focus(h, parent);
    if !plant(20691) {
        h.win_set_modal(wm, parent);
    }
}

/// `WOLLocaleSelectShutdown(layout, userData)`.
pub fn wol_locale_select_shutdown<H: WolLocaleSelectHost>(wm: &mut GameWindowManager, h: &mut H, layout: usize) {
    wm.layout_hide(layout, true);
    shell::shutdown_complete(wm, h, Some(layout), false);
}

/// `WOLLocaleSelectUpdate(layout, userData)`: empty.
pub fn wol_locale_select_update<H: WolLocaleSelectHost>(_wm: &mut GameWindowManager, _h: &mut H, _layout: usize) {}

/// `WOLLocaleSelectInput(window, msg, mData1, mData2)`: every key is swallowed.
pub fn wol_locale_select_input(msg: u32) -> MsgHandled {
    if msg == gwm::CHAR {
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// `WOLLocaleSelectSystem(window, msg, mData1, mData2)`.
pub fn wol_locale_select_system<H: WolLocaleSelectHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
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
            let control_id = wm.tree.windows[control].id;
            let s = h.wol_locale_select().clone();
            if control_id == s.button_ok_id {
                let selected = match s.listbox_locale {
                    Some(l) => lb::get_selected(wm, h, l).first().copied().unwrap_or(-1),
                    None => -1,
                };
                if selected < i32::from(plant(20692)) {
                    return MsgHandled::Handled; // can't select nothing!
                }

                let locale = if plant(20687) { selected } else { selected + LOC_MIN };
                if plant(20688) {
                    h.gamespy_close_overlay_locale_select(wm);
                    h.ps_add_update_player_locale_request(locale);
                } else {
                    h.ps_add_update_player_locale_request(locale);

                    h.gamespy_close_overlay_locale_select(wm);
                }

                h.misc_prefs_set_locale_and_write(locale);

                // update the local player's stats in the queue's cache
                let local_id = h.local_profile_id();
                let mut stats = h.ps_find_player_stats_by_id(local_id);
                stats.locale = locale;
                if (stats.id == local_id && !plant(20689)) || (stats.id == 0 && plant(20689)) {
                    h.ps_track_player_stats_locale(&stats);
                }

                if (stats.id == 0) != plant(20690) {
                    h.gamespy_set_cached_local_player_stats_locale(locale);
                } else {
                    h.ps_add_player_stats_response_for_local_player();
                }

                h.check_re_open_player_info(wm);
            }
        }
        ggm::GEM_EDIT_DONE => {}
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}
