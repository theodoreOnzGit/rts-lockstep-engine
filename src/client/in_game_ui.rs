// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (selectDrawable, deselectDrawable, deselectAllDrawables,
//   getAllSelectedDrawables, getSelectCount, getMaxSelectCount, incrementSelectCount, decrementSelectCount,
//   isAnySelectedKindOf, the force-attack and prefer-selection modes, setSelecting, setInputEnabled)
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
    /// `m_waypointMode` (C2b).
    waypoint_mode: bool,
    /// `m_forceMoveToMode` (C2b).
    force_move_to_mode: bool,
    /// `m_attackMoveToMode` (C2b).
    attack_move_to_mode: bool,
    /// `m_pendingGUICommand` (C2c): the button whose target the cursor is picking.
    gui_command: Option<crate::client::control_bar::GuiButton>,
    /// `m_pendingPlaceType` (C2c): the thing template being placed.
    pending_place_type: Option<u16>,
    /// `m_pendingPlaceSourceObjectID` (C2c): the object that will build it.
    pending_place_source: Option<crate::common::game_type::ObjectId>,
    /// `m_preventLeftClickDeselectionInAlternateMouseModeForOneClick` (C2c).
    prevent_left_click_deselection_once: bool,
    /// `m_placeAnchorStart`, `m_placeAnchorEnd` and `m_placeAnchorInProgress` (C2c part 2), in screen pixels.
    place_anchor: (crate::common::base_type::ICoord2D, crate::common::base_type::ICoord2D),
    place_anchor_in_progress: bool,
    /// `m_placeIcon[0]->getOrientation()` (C2c part 2): the building's placement angle.
    placement_angle: f32,
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

    /// `setInputEnabled(enable)` / `getInputEnabled()`. ~~A plain setter.~~ **Corrected 2026-11-10 (#116):** as upstream,
    /// disabling also ends a drag's `setSelecting`, and going from enabled to disabled clears the modes a key held through
    /// a cinematic would leave on: force attack (CTRL), force move, waypoint (ALT) and prefer selection (SHIFT). The four
    /// camera rotate and zoom flags it also clears are not InGameUI members in this port (the application's camera keys).
    pub fn set_input_enabled(&mut self, enable: bool) {
        if !enable {
            self.set_selecting(false);
        }
        let was_enabled = self.input_enabled;
        self.input_enabled = enable;
        if (was_enabled || crate::logic::modules::plant(9021)) && !enable {
            self.set_force_attack_mode(false);
            self.set_force_move_mode(false);
            self.set_waypoint_mode(false);
            if !crate::logic::modules::plant(9022) {
                self.set_prefer_selection_mode(false);
            }
        }
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

    /// `setWaypointMode(b)` / `isInWaypointMode()` (C2b).
    pub fn set_waypoint_mode(&mut self, b: bool) {
        self.waypoint_mode = b;
    }
    pub fn is_in_waypoint_mode(&self) -> bool {
        self.waypoint_mode
    }

    /// `setForceMoveMode(b)` / `isInForceMoveToMode()` (C2b).
    pub fn set_force_move_mode(&mut self, b: bool) {
        self.force_move_to_mode = b;
    }
    pub fn is_in_force_move_to_mode(&self) -> bool {
        self.force_move_to_mode
    }

    /// `setAttackMoveToMode(b)` / `isInAttackMoveToMode()` / `clearAttackMoveToMode()` (C2b).
    pub fn set_attack_move_to_mode(&mut self, b: bool) {
        self.attack_move_to_mode = b;
    }
    pub fn is_in_attack_move_to_mode(&self) -> bool {
        self.attack_move_to_mode
    }
    pub fn clear_attack_move_to_mode(&mut self) {
        self.attack_move_to_mode = false;
    }
    /// `toggleAttackMoveToMode()` (C2c).
    pub fn toggle_attack_move_to_mode(&mut self) {
        self.attack_move_to_mode = !self.attack_move_to_mode;
    }

    /// `setGUICommand(command)` (C2c): a command that needs no target is refused ("does not need additional user
    /// interaction", a `DEBUG_ASSERTCRASH` upstream) and clears the pending one. The cursor and radius cursor are the view's
    /// (C2d); the recorder's playback guard comes with the replay viewer.
    pub fn set_gui_command(&mut self, command: Option<crate::client::control_bar::GuiButton>) {
        if command.as_ref().is_some_and(|c| c.options & crate::client::control_bar::opt::NEED_TARGET == 0) {
            self.gui_command = None;
            return;
        }
        self.gui_command = command;
    }
    /// `getGUICommand()` (C2c).
    pub fn gui_command(&self) -> Option<&crate::client::control_bar::GuiButton> {
        self.gui_command.as_ref()
    }

    /// `placeBuildAvailable(build, buildDrawable)` (C2c): start (or with `None`, end) placing `build`, built by the object of
    /// `builder`; the icon starts at `view_angle` (`build->getPlacementViewAngle()`). The building at the cursor and the
    /// cursor itself are the view's (C2d).
    pub fn place_build_available(&mut self, build: Option<u16>, builder: Option<DrawableId>, view_angle: f32, h: &impl crate::client::selection_info::SelectionInfoHost) {
        if build.is_some() {
            self.placement_angle = view_angle;
        } else {
            // (upstream destroys the icon; its angle is gone with it)
            self.place_anchor_in_progress = false;
        }
        // (upstream first ends a placement under way when a new one starts; here that is the same assignment)
        self.pending_place_type = build;
        self.prevent_left_click_deselection_once = self.pending_place_source.is_some();
        self.pending_place_source = builder.and_then(|d| h.drawable_object(d));
    }
    /// `getPendingPlaceType()` / `getPendingPlaceSourceObjectID()` (C2c).
    pub fn pending_place_type(&self) -> Option<u16> {
        self.pending_place_type
    }
    pub fn pending_place_source(&self) -> Option<crate::common::game_type::ObjectId> {
        self.pending_place_source
    }
    /// `setPlacementStart(start)` (C2c part 2): `Some` anchors the placement there, `None` releases it.
    pub fn set_placement_start(&mut self, start: Option<crate::common::base_type::ICoord2D>) {
        match start {
            Some(s) => {
                self.place_anchor = (s, s);
                self.place_anchor_in_progress = true;
            }
            None => self.place_anchor_in_progress = false,
        }
    }
    /// `setPlacementEnd(end)` (C2c part 2).
    pub fn set_placement_end(&mut self, end: crate::common::base_type::ICoord2D) {
        self.place_anchor.1 = end;
    }
    /// `isPlacementAnchored()` (C2c part 2).
    pub fn is_placement_anchored(&self) -> bool {
        self.place_anchor_in_progress
    }
    /// `getPlacementPoints(&start, &end)` (C2c part 2).
    pub fn placement_points(&self) -> (crate::common::base_type::ICoord2D, crate::common::base_type::ICoord2D) {
        self.place_anchor
    }
    /// `getPlacementAngle()` (C2c part 2).
    pub fn placement_angle(&self) -> f32 {
        self.placement_angle
    }
    /// `handleBuildPlacements()`'s angle (C2c part 2): while anchored and dragged, the icon faces from the anchor's terrain
    /// point `world_start` to the drag's `world_end` (`Coord2D::toAngle`).
    pub fn turn_placement(&mut self, world_start: crate::common::base_type::Coord3D, world_end: crate::common::base_type::Coord3D) {
        let (s, e) = self.place_anchor;
        if self.pending_place_type.is_some() && self.place_anchor_in_progress && s != e {
            let v = crate::common::base_type::Coord2D { x: world_end.x - world_start.x, y: world_end.y - world_start.y };
            self.placement_angle = v.to_angle();
        }
    }

    /// `getPreventLeftClickDeselectionInAlternateMouseModeForOneClick()` (C2c).
    pub fn prevent_left_click_deselection_once(&self) -> bool {
        self.prevent_left_click_deselection_once
    }
    /// `setPreventLeftClickDeselectionInAlternateMouseModeForOneClick(set)` (#54: the GUI command translator sets it when
    /// a command is carried out).
    pub fn set_prevent_left_click_deselection_once(&mut self, set: bool) {
        self.prevent_left_click_deselection_once = set;
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

    /// #116: `setInputEnabled`. Predicted from InGameUI.cpp: disabling ends a drag's `isSelecting` and, from enabled,
    /// clears force attack, force move, waypoint and prefer selection, leaving attack move; a second disable with the
    /// modes set again clears only `isSelecting` (the modes stay: the input was already off); enabling changes no mode.
    #[test]
    fn set_input_enabled_clears_the_held_modes_once() {
        let mut s = InGameUiSelection::new();
        let set_all = |s: &mut InGameUiSelection| {
            s.set_selecting(true);
            s.set_force_attack_mode(true);
            s.set_force_move_mode(true);
            s.set_waypoint_mode(true);
            s.set_prefer_selection_mode(true);
            s.set_attack_move_to_mode(true);
        };
        let modes = |s: &InGameUiSelection| {
            [s.is_selecting(), s.is_in_force_attack_mode(), s.is_in_force_move_to_mode(), s.is_in_waypoint_mode(), s.is_in_prefer_selection_mode(), s.is_in_attack_move_to_mode()]
        };
        set_all(&mut s);
        s.set_input_enabled(false);
        assert_eq!((s.input_enabled(), modes(&s)), (false, [false, false, false, false, false, true]));
        set_all(&mut s);
        s.set_input_enabled(false);
        assert_eq!(modes(&s), [false, true, true, true, true, true]);
        s.set_input_enabled(true);
        assert_eq!((s.input_enabled(), modes(&s)), (true, [false, true, true, true, true, true]));
    }
}
