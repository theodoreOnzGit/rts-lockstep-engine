//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_selection_xlat.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/SelectionXlat.cpp
//!     (`SelectionTranslator::selectFriends`, `killThemKillThemAll`)
//!
//! Translated already (#213): `setDragSelecting` and `setLeftMouseButton` are plain stores of
//! `crate::client::selection_xlat::SelectionTranslator::drag_selecting` / `left_mouse_button_is_down` (public fields).
//! `CanSelectDrawable` is `crate::client::selection_xlat::can_select_drawable`.
//!
//! `m_displayedMaxWarning` and `m_selectCountMap` (selected drawables per template) are not in the verified
//! `SelectionTranslator`; they are [`SelectFriendsState`]. The `TheInGameUI` and `TheGameText` calls are
//! [`SelectFriendsHost`].

use std::collections::BTreeMap;

/// What `selectFriends` / `killThemKillThemAll` reach.
pub trait SelectFriendsHost {
    /// `CanSelectDrawable(draw, dragSelecting)`.
    fn can_select_drawable(&self, draw: u32, drag_selecting: bool) -> bool;
    /// `TheInGameUI->getMaxSelectCount()`.
    fn max_select_count(&self) -> i32;
    /// `TheInGameUI->getSelectCount()`.
    fn select_count(&self) -> i32;
    /// `TheInGameUI->message(TheGameText->fetch("GUI:MaxSelectionSize") formatted with the max)`.
    fn message_max_selection_size(&mut self, max: i32);
    /// `TheInGameUI->selectDrawable(draw)`.
    fn select_drawable(&mut self, draw: u32);
    /// `draw->getTemplate()` (a key for the per-template count).
    fn drawable_template(&self, draw: u32) -> Vec<u8>;
    /// `draw->getObject()->getID()`; `None` when the drawable has no object.
    fn drawable_object_id(&self, draw: u32) -> Option<u32>;
}

/// The members `SelectionTranslator` keeps for these two.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectFriendsState {
    /// `m_displayedMaxWarning`.
    pub displayed_max_warning: bool,
    /// `m_selectCountMap`.
    pub select_count_map: BTreeMap<Vec<u8>, i32>,
}

impl SelectFriendsState {
    /// The shared "enforce an optional selection size limit" block: `true` when the limit stops the selection (the
    /// warning is shown once).
    fn limit_reached<H: SelectFriendsHost + ?Sized>(&mut self, host: &mut H) -> bool {
        let limited = if crate::logic::modules::plant(19861) { host.max_select_count() >= 0 } else { host.max_select_count() > 0 };
        let reached = if crate::logic::modules::plant(19862) { host.select_count() > host.max_select_count() } else { host.select_count() >= host.max_select_count() };
        if limited && reached {
            if !self.displayed_max_warning || crate::logic::modules::plant(19863) {
                self.displayed_max_warning = true;
                let max = host.max_select_count();
                host.message_max_selection_size(max);
            }
            return true;
        }
        false
    }

    /// `SelectionTranslator::selectFriends(draw, createTeamMsg, dragSelecting)` (SelectionXlat.cpp:301): select the
    /// drawable if `CanSelectDrawable`, within the optional size limit, counting it per template and appending its object
    /// id to the team message (`team_msg_object_ids` is the message's argument list, `None` when there is no message).
    pub fn select_friends<H: SelectFriendsHost + ?Sized>(&mut self, host: &mut H, draw: u32, team_msg_object_ids: Option<&mut Vec<u32>>, drag_selecting: bool) -> bool {
        if host.can_select_drawable(draw, if crate::logic::modules::plant(19864) { false } else { drag_selecting }) {
            // enforce an optional selection size limit
            if self.limit_reached(host) {
                return false;
            }
            host.select_drawable(draw);
            let key = host.drawable_template(draw);
            if !crate::logic::modules::plant(19865) || host.drawable_object_id(draw).is_some() {
                *self.select_count_map.entry(key).or_insert(0) += 1;
            }
            // add to message's argument list if an object is present
            if let (Some(obj), Some(msg)) = (host.drawable_object_id(draw), team_msg_object_ids) {
                if !crate::logic::modules::plant(19866) {
                    msg.push(obj);
                }
            }
            return true; // selected
        }
        false // not selected
    }

    /// `SelectionTranslator::killThemKillThemAll(draw, killThemAllMsg)` (SelectionXlat.cpp:337): for a drawable with an
    /// object, within the size limit, append the object's id to the message; nothing is selected.
    pub fn kill_them_kill_them_all<H: SelectFriendsHost + ?Sized>(&mut self, host: &mut H, draw: Option<u32>, kill_them_all_msg_object_ids: Option<&mut Vec<u32>>) -> bool {
        if let Some(draw) = draw {
            if let Some(obj) = host.drawable_object_id(draw).or(if crate::logic::modules::plant(19867) { Some(0) } else { None }) {
                // enforce an optional selection size limit
                if self.limit_reached(host) {
                    return false;
                }
                // add to message's argument list if an object is present
                if let Some(msg) = kill_them_all_msg_object_ids {
                    msg.push(obj);
                }
                return true; // selected
            }
        }
        false
    }
}
