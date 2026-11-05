// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (selectDrawable, deselectDrawable, deselectAllDrawables,
//   getAllSelectedDrawables, getSelectCount, getMaxSelectCount, incrementSelectCount, decrementSelectCount,
//   isAnySelectedKindOf, the force-attack and prefer-selection modes, setSelecting)
//   GeneralsMD/Code/GameEngine/Include/GameClient/InGameUI.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TheInGameUI`'s selection model (C2a, GitHub #6): which drawables the local player has selected, kept on the client as
//! upstream keeps it. The logic learns of a selection only through the command messages the translators append
//! (`MSG_CREATE_SELECTED_GROUP`, `MSG_DESTROY_SELECTED_GROUP`, ...), so nothing here changes the game.
//!
//! - A drawable's `isSelected` bit is the model's [`InGameUiSelection::is_selected`] (upstream keeps it on the drawable).
//! - `m_selectedDrawables` is a `std::list` with each new selection pushed at the **front**: the order is kept, as callers
//!   iterate it (the first selected drawable is the list's front, the last one selected).
//! - **Not translated here:** `evaluateSoloNexus` (the angry mob's lone selection), the control bar's `onDrawableSelected`
//!   (C5), and the drawing of the selection. Each is a call on [`SelectionUiHost`], which the client gives.

use crate::common::message_stream::MessageType;

/// A drawable's id (`DrawableID`); the client's drawable of an object.
pub use crate::common::game_type::DrawableId;

/// What the selection model reaches outside itself.
pub trait SelectionUiHost {
    /// `TheGameLogic->getFrame()`.
    fn logic_frame(&self) -> u32;
    /// `TheMessageStream->appendMessage(type)` with its arguments: `true` for a boolean argument.
    fn append_message_bool(&mut self, t: MessageType, b: bool);
    /// `evaluateSoloNexus(draw)` (the mob: not translated) and `TheControlBar->onDrawableSelected(draw)` /
    /// `onDrawableDeselected` (C5): told of each change.
    fn on_selection_changed(&mut self, draw: DrawableId, selected: bool);
}

/// `InGameUI`'s selection members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InGameUiSelection {
    /// `m_selectedDrawables`, the newest at the front.
    selected: Vec<DrawableId>,
    /// `m_selectCount`.
    select_count: i32,
    /// `m_maxSelectCount` (`-1`: no limit, the constructor's).
    max_select_count: i32,
    /// `m_frameSelectionChanged`.
    frame_selection_changed: u32,
    /// `m_isSelecting` (a drag selection is under way).
    is_selecting: bool,
    /// `m_forceAttackMode`.
    force_attack_mode: bool,
    /// `m_preferSelection`.
    prefer_selection: bool,
    /// `m_inputEnabled`.
    input_enabled: bool,
    /// `m_displayedMaxWarning` (the selection cap's message was shown).
    displayed_max_warning: bool,
}

impl InGameUiSelection {
    /// `InGameUI()`'s selection members: nothing selected, no limit, input enabled.
    pub fn new() -> Self {
        InGameUiSelection { max_select_count: -1, input_enabled: true, ..Default::default() }
    }

    /// `draw->isSelected()`.
    pub fn is_selected(&self, draw: DrawableId) -> bool {
        self.selected.contains(&draw)
    }

    /// `selectDrawable(draw)`: an unselected drawable is marked and pushed at the front of the list, the count goes up, and the
    /// frame of the change is kept.
    pub fn select_drawable(&mut self, draw: DrawableId, host: &mut impl SelectionUiHost) {
        if !self.is_selected(draw) {
            self.frame_selection_changed = host.logic_frame();
            self.selected.insert(0, draw);
            self.select_count += 1;
            host.on_selection_changed(draw, true);
        }
    }

    /// `deselectDrawable(draw)`: a selected drawable is cleared and taken out of the list.
    pub fn deselect_drawable(&mut self, draw: DrawableId, host: &mut impl SelectionUiHost) {
        if let Some(i) = self.selected.iter().position(|&d| d == draw) {
            self.frame_selection_changed = host.logic_frame();
            self.selected.remove(i);
            self.select_count -= 1;
            host.on_selection_changed(draw, false);
        }
    }

    /// `deselectAllDrawables(postMsg)`: each one deselected in the list's order, then (`post_msg`) `MSG_DESTROY_SELECTED_GROUP`
    /// with `true` ("deletes entire group").
    pub fn deselect_all_drawables(&mut self, post_msg: bool, host: &mut impl SelectionUiHost) {
        for d in self.selected.clone() {
            self.deselect_drawable(d, host);
        }
        self.selected.clear();
        if post_msg {
            host.append_message_bool(crate::common::message_types::MSG_DESTROY_SELECTED_GROUP, true);
        }
    }

    /// `getAllSelectedDrawables()`: the list, newest first.
    pub fn all_selected_drawables(&self) -> &[DrawableId] {
        &self.selected
    }

    /// `getFirstSelectedDrawable()`: the list's front, or `None`.
    pub fn first_selected_drawable(&self) -> Option<DrawableId> {
        self.selected.first().copied()
    }

    /// `getSelectCount()`.
    pub fn select_count(&self) -> i32 {
        self.select_count
    }

    /// `getMaxSelectCount()`.
    pub fn max_select_count(&self) -> i32 {
        self.max_select_count
    }

    /// `setMaxSelectCount(n)`.
    pub fn set_max_select_count(&mut self, n: i32) {
        self.max_select_count = n;
    }

    /// `getFrameSelectionChanged()`.
    pub fn frame_selection_changed(&self) -> u32 {
        self.frame_selection_changed
    }

    /// `setSelecting(b)` / `isSelecting()`.
    pub fn set_selecting(&mut self, b: bool) {
        self.is_selecting = b;
    }
    pub fn is_selecting(&self) -> bool {
        self.is_selecting
    }

    /// `setForceAttackMode(b)` / `isInForceAttackMode()`.
    pub fn set_force_attack_mode(&mut self, b: bool) {
        self.force_attack_mode = b;
    }
    pub fn is_in_force_attack_mode(&self) -> bool {
        self.force_attack_mode
    }

    /// `setPreferSelectionMode(b)` / `isInPreferSelectionMode()`.
    pub fn set_prefer_selection_mode(&mut self, b: bool) {
        self.prefer_selection = b;
    }
    pub fn is_in_prefer_selection_mode(&self) -> bool {
        self.prefer_selection
    }

    /// `setInputEnabled(b)` / `getInputEnabled()`.
    pub fn set_input_enabled(&mut self, b: bool) {
        self.input_enabled = b;
    }
    pub fn input_enabled(&self) -> bool {
        self.input_enabled
    }

    /// `getDisplayedMaxWarning()` / `setDisplayedMaxWarning(b)`.
    pub fn displayed_max_warning(&self) -> bool {
        self.displayed_max_warning
    }
    pub fn set_displayed_max_warning(&mut self, b: bool) {
        self.displayed_max_warning = b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Host {
        frame: u32,
        messages: Vec<(MessageType, bool)>,
        changes: Vec<(DrawableId, bool)>,
    }

    impl SelectionUiHost for Host {
        fn logic_frame(&self) -> u32 {
            self.frame
        }
        fn append_message_bool(&mut self, t: MessageType, b: bool) {
            self.messages.push((t, b));
        }
        fn on_selection_changed(&mut self, draw: DrawableId, selected: bool) {
            self.changes.push((draw, selected));
        }
    }

    /// C2a: the selection model as `InGameUI` keeps it. Predicted from InGameUI.cpp: selecting 3, 5, 7 lists them newest
    /// first (7, 5, 3) with a count of 3; selecting 5 again changes nothing; deselecting 5 leaves 7, 3; deselecting all clears
    /// the list and the count and posts one `MSG_DESTROY_SELECTED_GROUP(true)`; the change frame is the logic's frame of the
    /// last change.
    #[test]
    fn the_selection_list_is_kept_as_in_game_ui_keeps_it() {
        let mut h = Host { frame: 10, ..Default::default() };
        let mut s = InGameUiSelection::new();
        assert_eq!((s.max_select_count(), s.input_enabled()), (-1, true));
        for d in [3, 5, 7] {
            s.select_drawable(d, &mut h);
        }
        assert_eq!((s.all_selected_drawables(), s.select_count(), s.first_selected_drawable()), (&[7, 5, 3][..], 3, Some(7)));
        h.frame = 11;
        s.select_drawable(5, &mut h);
        assert_eq!((s.select_count(), s.frame_selection_changed()), (3, 10), "no change");
        s.deselect_drawable(5, &mut h);
        assert_eq!((s.all_selected_drawables(), s.frame_selection_changed()), (&[7, 3][..], 11));
        h.frame = 12;
        s.deselect_all_drawables(true, &mut h);
        assert_eq!((s.all_selected_drawables().len(), s.select_count()), (0, 0));
        assert_eq!(h.messages, vec![(crate::common::message_types::MSG_DESTROY_SELECTED_GROUP, true)]);
        assert_eq!(h.changes.len(), 3 + 1 + 2);
    }
}
