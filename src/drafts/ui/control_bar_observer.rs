//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_observer.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarObserver.cpp
//!     (the file's statics, `ControlBar::initObserverControls`, `ControlBarObserverSystem`,
//!      `ControlBar::populateObserverList`, `populateObserverInfoWindow`)

use crate::client::window_manager::{data_win, ggm, gwm, GameWindowManager, GuiHost, MsgHandled, WindowMsgData};
use crate::client::window_script::{win_status, WinId};
use crate::logic::modules::plant;

/// `enum { MAX_BUTTONS = 8 }` (ControlBarObserver.cpp:55).
pub const MAX_BUTTONS: usize = 8;
/// `MAX_SLOTS` (NetworkDefs.h).
pub const MAX_SLOTS: usize = 8;
/// `MAX_PLAYER_COUNT` (GameCommon.h).
pub const MAX_PLAYER_COUNT: usize = 16;

/// The file's statics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlBarObserver {
    pub button_player_id: [i32; MAX_BUTTONS],
    pub static_text_player_id: [i32; MAX_BUTTONS],
    pub observer_player_info_window: Option<WinId>,
    pub observer_player_list_window: Option<WinId>,
    pub button_player: [Option<WinId>; MAX_BUTTONS],
    pub static_text_player: [Option<WinId>; MAX_BUTTONS],
    pub button_cancel_id: i32,
    pub win_flag: Option<WinId>,
    pub win_general_portrait: Option<WinId>,
    pub static_text_number_of_units: Option<WinId>,
    pub static_text_number_of_buildings: Option<WinId>,
    pub static_text_number_of_units_killed: Option<WinId>,
    pub static_text_number_of_units_lost: Option<WinId>,
    pub static_text_player_name: Option<WinId>,
}

impl Default for ControlBarObserver {
    /// All `NAMEKEY_INVALID` (0) and `NULL`.
    fn default() -> Self {
        ControlBarObserver {
            button_player_id: [0; MAX_BUTTONS],
            static_text_player_id: [0; MAX_BUTTONS],
            observer_player_info_window: None,
            observer_player_list_window: None,
            button_player: [None; MAX_BUTTONS],
            static_text_player: [None; MAX_BUTTONS],
            button_cancel_id: 0,
            win_flag: None,
            win_general_portrait: None,
            static_text_number_of_units: None,
            static_text_number_of_buildings: None,
            static_text_number_of_units_killed: None,
            static_text_number_of_units_lost: None,
            static_text_player_name: None,
        }
    }
}

/// What the observer bar reads from the game. A player is an index the host knows.
pub trait ObserverHost: GuiHost {
    fn observer(&mut self) -> &mut ControlBarObserver;
    fn name_to_key(&mut self, name: &[u8]) -> i32;
    /// `TheRecorder->isMultiplayer()`.
    fn is_multiplayer(&self) -> bool;
    /// `ThePlayerList->findPlayerWithNameKey(nameToKey("player<i>"))`.
    fn find_player_with_name_key(&mut self, name: &[u8]) -> Option<usize>;
    /// `ThePlayerList->getNthPlayer(i)`.
    fn get_nth_player(&self, i: usize) -> Option<usize>;
    fn is_player_observer(&self, p: usize) -> bool;
    /// `p->getPlayerType() == PLAYER_HUMAN`.
    fn is_player_human(&self, p: usize) -> bool;
    /// `GadgetButtonSetData(w, p)`, `GadgetButtonGetData(w)`.
    fn gadget_button_set_data_player(&mut self, wm: &mut GameWindowManager, w: WinId, p: usize);
    fn gadget_button_get_data_player(&self, wm: &GameWindowManager, w: WinId) -> Option<usize>;
    /// `GadgetButtonSetEnabledImage(w, p->getPlayerTemplate()->getEnabledImage())`.
    fn gadget_button_set_enabled_image_player(&mut self, wm: &mut GameWindowManager, w: WinId, p: usize);
    /// `w->winSetTooltip(p->getPlayerDisplayName())`.
    fn win_set_tooltip_player_name(&mut self, wm: &mut GameWindowManager, w: WinId, p: usize);
    /// `p->getPlayerColor()`.
    fn player_color(&self, p: usize) -> i32;
    /// `GameMakeColor(0, 0, 0, 255)`.
    fn make_color(&self, r: u8, g: u8, b: u8, a: u8) -> i32;
    /// `w->winSetEnabledTextColors(color, border)`.
    fn win_set_enabled_text_colors(&mut self, wm: &mut GameWindowManager, w: WinId, color: i32, border: i32);
    fn gadget_static_text_set_text(&mut self, wm: &mut GameWindowManager, w: WinId, text: &[u16]);
    /// `p->getPlayerDisplayName()`.
    fn player_display_name(&self, p: usize) -> Vec<u16>;
    /// `TheGameInfo->getConstSlot(i)`: its team number, and whether it is an AI.
    fn slot_team_number(&self, i: usize) -> i32;
    fn slot_is_ai(&self, i: usize) -> bool;
    /// `TheGameText->fetch(label)`; the label is ASCII (`teamStr`).
    fn fetch_text(&self, label: &[u8]) -> Vec<u16>;
    /// `UnicodeString::format(TheGameText->fetch("CONTROLBAR:ObsPlayerLabel"), name, team)`.
    fn format_obs_player_label(&self, name: &[u16], team: &[u16]) -> Vec<u16>;
    /// `m_observerLookAtPlayer`.
    fn observer_look_at_player(&self) -> Option<usize>;
    fn set_observer_look_at_player(&mut self, p: Option<usize>);
    /// `m_observerLookAtPlayer->countObjects(mask, clearmask)` for the masks the function builds: `(set, clear)` as
    /// lists of `KINDOF_` names (`SCORE`, `STRUCTURE`, `SCORE_CREATE`, `SCORE_DESTROY`).
    fn count_objects(&self, p: usize, set: &[&str], clear: &[&str]) -> i32;
    /// `getScoreKeeper()->getTotalUnitsDestroyed()`, `getTotalUnitsLost()`.
    fn total_units_destroyed(&self, p: usize) -> i32;
    fn total_units_lost(&self, p: usize) -> i32;
    /// `winFlag->winSetEnabledImage(0, p->getPlayerTemplate()->getFlagWaterMarkImage())`.
    fn win_set_flag_image(&mut self, wm: &mut GameWindowManager, w: WinId, p: usize);
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `ControlBar::initObserverControls()` (ControlBarObserver.cpp:84).
pub fn init_observer_controls<H: ObserverHost>(wm: &mut GameWindowManager, h: &mut H) {
    let id = h.name_to_key(b"ControlBar.wnd:ObserverPlayerInfoWindow");
    let info = wm.win_get_window_from_id(None, id);
    let id = h.name_to_key(b"ControlBar.wnd:ObserverPlayerListWindow");
    let list = wm.win_get_window_from_id(None, id);
    h.observer().observer_player_info_window = info;
    h.observer().observer_player_list_window = list;

    for i in 0..MAX_BUTTONS {
        let button_id = h.name_to_key(format!("ControlBar.wnd:ButtonPlayer{i}").as_bytes());
        let button = wm.win_get_window_from_id(if plant(21241) { None } else { list }, button_id);
        let text_id = h.name_to_key(format!("ControlBar.wnd:StaticTextPlayer{i}").as_bytes());
        let text = wm.win_get_window_from_id(list, text_id);
        let o = h.observer();
        o.button_player_id[i] = button_id;
        o.button_player[i] = button;
        o.static_text_player_id[i] = text_id;
        o.static_text_player[i] = text;
    }

    let find = |h: &mut H, name: &[u8]| {
        let id = h.name_to_key(name);
        wm.win_get_window_from_id(None, id)
    };
    let units = find(h, b"ControlBar.wnd:StaticTextNumberOfUnits");
    let buildings = find(h, b"ControlBar.wnd:StaticTextNumberOfBuildings");
    let killed = find(h, b"ControlBar.wnd:StaticTextNumberOfUnitsKilled");
    let lost = find(h, b"ControlBar.wnd:StaticTextNumberOfUnitsLost");
    let name = find(h, b"ControlBar.wnd:StaticTextPlayerName");
    let flag = find(h, b"ControlBar.wnd:WinFlag");
    let portrait = find(h, b"ControlBar.wnd:WinGeneralPortrait");
    let cancel = h.name_to_key(b"ControlBar.wnd:ButtonCancel");
    let o = h.observer();
    o.static_text_number_of_units = units;
    o.static_text_number_of_buildings = buildings;
    o.static_text_number_of_units_killed = killed;
    o.static_text_number_of_units_lost = lost;
    o.static_text_player_name = name;
    o.win_flag = flag;
    o.win_general_portrait = portrait;
    o.button_cancel_id = cancel;
}

/// `ControlBarObserverSystem(window, msg, mData1, mData2)` (ControlBarObserver.cpp:121).
pub fn control_bar_observer_system<H: ObserverHost>(wm: &mut GameWindowManager, h: &mut H, _window: WinId, msg: u32, d1: WindowMsgData, _d2: WindowMsgData) -> MsgHandled {
    match msg {
        gwm::CREATE => {}
        ggm::GBM_MOUSE_ENTERING | ggm::GBM_MOUSE_LEAVING => {}
        ggm::GBM_SELECTED | ggm::GBM_SELECTED_RIGHT => {
            let Some(control) = data_win(d1) else { return MsgHandled::Handled };
            let control_id = wm.win(control).id;
            let o = h.observer().clone();
            if control_id == o.button_cancel_id {
                h.set_observer_look_at_player(None);
                if let Some(w) = o.observer_player_info_window {
                    wm.win_hide(w, !plant(21243));
                }
                if let Some(w) = o.observer_player_list_window {
                    wm.win_hide(w, plant(21243));
                }
                if !plant(21242) {
                    populate_observer_list(wm, h);
                }
            }
            for i in 0..MAX_BUTTONS {
                if control_id == o.button_player_id[i] {
                    if let Some(w) = o.observer_player_info_window {
                        wm.win_hide(w, false);
                    }
                    if let Some(w) = o.observer_player_list_window {
                        wm.win_hide(w, true);
                    }
                    let p = o.button_player[i].and_then(|b| h.gadget_button_get_data_player(wm, b));
                    h.set_observer_look_at_player(p);
                    if h.observer_look_at_player().is_some() && !plant(21244) {
                        populate_observer_info_window(wm, h);
                    }
                    return MsgHandled::Handled;
                }
            }
            // (the communicator button's block is commented out upstream)
        }
        _ => return MsgHandled::Ignored,
    }
    MsgHandled::Handled
}

/// The body both of `populateObserverList`'s loops share for player `p` into slot `current_button`.
fn fill_button<H: ObserverHost>(wm: &mut GameWindowManager, h: &mut H, p: usize, current_button: usize) {
    let o = h.observer().clone();
    let (Some(button), Some(text)) = (o.button_player[current_button], o.static_text_player[current_button]) else { return };
    h.gadget_button_set_data_player(wm, button, p);
    h.gadget_button_set_enabled_image_player(wm, button, p);
    h.win_set_tooltip_player_name(wm, button, p);
    wm.win_hide(button, false);
    wm.win_set_status(button, win_status::USE_OVERLAY_STATES);

    let player_color = h.player_color(p);
    let back_color = h.make_color(0, 0, 0, 255);
    h.win_set_enabled_text_colors(wm, text, player_color, back_color);
    wm.win_hide(text, false);
}

/// `ControlBar::populateObserverList()` (ControlBarObserver.cpp:197).
pub fn populate_observer_list<H: ObserverHost>(wm: &mut GameWindowManager, h: &mut H) {
    let mut current_button = 0usize;
    if h.is_multiplayer() {
        for i in 0..MAX_SLOTS {
            let name = format!("player{i}");
            let Some(p) = h.find_player_with_name_key(name.as_bytes()) else { continue };
            if h.is_player_observer(p) {
                continue;
            }
            // (DEBUG_ASSERTCRASH(currentButton < MAX_BUTTONS) is compiled out: a ninth player indexes past the arrays)
            if current_button >= MAX_BUTTONS {
                break;
            }
            fill_button(wm, h, p, current_button);

            // `TheGameInfo->getConstSlot(currentButton)`
            let team_number = h.slot_team_number(current_button);
            let mut team_str = format!("Team:{}", if plant(21245) { team_number } else { team_number + 1 });
            if h.slot_is_ai(current_button) && (team_number == -1 || (plant(21246) && team_number == 0)) {
                team_str = "Team:AI".to_string();
            }

            let display_name = h.player_display_name(p);
            let team_text = h.fetch_text(team_str.as_bytes());
            let text = h.format_obs_player_label(&display_name, &team_text);
            if let Some(w) = h.observer().static_text_player[current_button] {
                h.gadget_static_text_set_text(wm, w, &text);
            }

            current_button += 1;
        }
    } else {
        for i in 0..MAX_PLAYER_COUNT {
            if let Some(p) = h.get_nth_player(i) {
                if !h.is_player_observer(p) && h.is_player_human(p) {
                    if current_button >= MAX_BUTTONS {
                        break;
                    }
                    fill_button(wm, h, p, current_button);
                    let display_name = h.player_display_name(p);
                    if let Some(w) = h.observer().static_text_player[current_button] {
                        h.gadget_static_text_set_text(wm, w, &display_name);
                    }

                    current_button += 1;
                    if !plant(21247) {
                        break;
                    }
                }
            }
        }
    }
    while current_button < MAX_BUTTONS {
        let o = h.observer().clone();
        if let Some(w) = o.button_player[current_button] {
            wm.win_hide(w, true);
        }
        if let Some(w) = o.static_text_player[current_button] {
            wm.win_hide(w, true);
        }
        current_button += 1;
    }
}

/// `ControlBar::populateObserverInfoWindow()` (ControlBarObserver.cpp:290).
pub fn populate_observer_info_window<H: ObserverHost>(wm: &mut GameWindowManager, h: &mut H) {
    let o = h.observer().clone();
    // `ObserverPlayerInfoWindow->winIsHidden()`: the window's own bit (a missing window: upstream dereferences NULL; `isHidden(None)` is true)
    let info_hidden = o.observer_player_info_window.map_or(true, |w| if plant(21251) { wm.is_hidden(Some(w)) } else { wm.tree.windows[w].status & win_status::HIDDEN != 0 });
    if info_hidden != plant(21250) {
        return;
    }

    let Some(p) = h.observer_look_at_player() else {
        if let Some(w) = o.observer_player_info_window {
            wm.win_hide(w, true);
        }
        if let Some(w) = o.observer_player_list_window {
            wm.win_hide(w, false);
        }
        populate_observer_list(wm, h);
        return;
    };

    let units = h.count_objects(p, &["SCORE"], &["STRUCTURE"]);
    if let Some(w) = o.static_text_number_of_units {
        h.gadget_static_text_set_text(wm, w, &utf16(&units.to_string()));
    }

    let mut num_buildings = h.count_objects(p, &["SCORE", "STRUCTURE"], &[]);
    num_buildings += h.count_objects(p, &["SCORE_CREATE", "STRUCTURE"], &[]);
    if !plant(21248) {
        num_buildings += h.count_objects(p, &["SCORE_DESTROY", "STRUCTURE"], &[]);
    }
    if let Some(w) = o.static_text_number_of_buildings {
        h.gadget_static_text_set_text(wm, w, &utf16(&num_buildings.to_string()));
    }
    let killed = if plant(21249) { h.total_units_lost(p) } else { h.total_units_destroyed(p) };
    if let Some(w) = o.static_text_number_of_units_killed {
        h.gadget_static_text_set_text(wm, w, &utf16(&killed.to_string()));
    }
    let lost = h.total_units_lost(p);
    if let Some(w) = o.static_text_number_of_units_lost {
        h.gadget_static_text_set_text(wm, w, &utf16(&lost.to_string()));
    }
    let name = h.player_display_name(p);
    if let Some(w) = o.static_text_player_name {
        h.gadget_static_text_set_text(wm, w, &name);
        let color = h.player_color(p);
        let back = h.make_color(0, 0, 0, 255);
        h.win_set_enabled_text_colors(wm, w, color, back);
    }
    if let Some(w) = o.win_flag {
        h.win_set_flag_image(wm, w, p);
    }
    if let Some(w) = o.win_general_portrait {
        wm.win_hide(w, false);
    }
}
