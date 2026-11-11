// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/SelectionXlat.cpp (currentlyLookingForSelection,
//   areAllSelected, deselectAll, the MSG_MOUSE_LEFT_CLICK case of SelectionTranslator::translateGameMessage; #116: its
//   input gate, the MSG_RAW_MOUSE_LEFT_BUTTON_DOWN case and the drag halves of MSG_RAW_MOUSE_POSITION and
//   MSG_RAW_MOUSE_LEFT_BUTTON_UP; #122: CanSelectDrawable, the mouse-over half of MSG_RAW_MOUSE_POSITION and the
//   MSG_MOUSEOVER_DRAWABLE_HINT case; #132: the MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN and _UP cases)
//   GeneralsMD/Code/GameEngine/Source/GameClient/SelectionInfo.cpp (addDrawableToList)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SelectionTranslator`'s left click (C2a, GitHub #6): a click, or a drag's region, turned into the selection messages the
//! logic reads (`MSG_CREATE_SELECTED_GROUP`, `MSG_REMOVE_FROM_SELECTED_GROUP`, `MSG_DESTROY_SELECTED_GROUP`).
//!
//! - **Picking is the host's:** `TheTacticalView->iterateDrawablesInRegion` needs the renderer's view. The host gives the
//!   drawables in the region, in the view's order; [`add_drawable_to_list`] filters them as upstream's callback does.
//! - **Not translated:** the debug-only modes of the click (hand of god, hurt, debug selection: `_DEBUG`/`_INTERNAL` and
//!   cheat builds only), and the sounds. The double click and the control groups are in `selection_groups`; the raw
//!   button feedback (the drag rectangle's hints) comes with the view (C2d).
//! - **The mouse-over (#122).** `CanSelectDrawable` ([`can_select_drawable`], against [`CanSelectHost`]), the
//!   `MSG_RAW_MOUSE_POSITION` case's mouse-over message ([`SelectionTranslator::mouseover_message`]) and the
//!   `MSG_MOUSEOVER_DRAWABLE_HINT` case ([`SelectionTranslator::mouseover_drawable_hint`]: over a drawable a click
//!   would not order, the InGameUI's `createMouseoverHint` and `TheMouse`'s `SELECTING` or arrow, the message eaten).
//!   `tests/differential_cursor_hint.rs` compares the last two of them with the C++ (the mouse-over message's pick is the
//!   application's and is not compared).
//! - **The input gate (#116).** Every case runs after `translateGameMessage`'s test of `TheInGameUI->getInputEnabled()`
//!   ([`SelectionTranslator::input_disabled_gate`]): with the input off the message is kept and a drag selection ends.
//!   The raw button's drag state (`m_leftMouseButtonIsDown`, `m_dragSelecting`, `m_selectFeedbackAnchor`) is kept for it.
//! - **The right button (#132).** `MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN` keeps the press's pixel, time and camera position
//!   ([`SelectionTranslator::right_button_down`]); on `MSG_RAW_MOUSE_RIGHT_BUTTON_UP` a click (within `TheMouse`'s three
//!   drag tolerances) cancels a pending GUI command, the message destroyed, or else deselects everything in the regular
//!   mouse mode, or in the alternate one while a building waits to be placed ([`SelectionTranslator::right_button_up`]).
//!   `tests/differential_input_gate.rs` compares both with the C++.

use crate::client::in_game_ui::{DrawableId, InGameUiSelection, SelectionUiHost};
use crate::client::command_xlat::{CommandEvaluateType, CommandTranslator, CommandXlatHost};
use crate::client::selection_info::{context_command_for_new_selection, SelectionInfo, SelectionInfoHost};
use crate::common::base_type::{Coord3D, ICoord2D};
use crate::common::game_common::Relationship;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::KINDOF_STRUCTURE;
use crate::common::message_stream::MessageType;
use crate::common::message_types::{MSG_CREATE_SELECTED_GROUP, MSG_INVALID, MSG_REMOVE_FROM_SELECTED_GROUP};

/// What the selection translator asks of the game beyond [`SelectionInfoHost`] and [`SelectionUiHost`].
pub trait SelectionXlatHost: SelectionInfoHost + SelectionUiHost {
    /// `TheInGameUI->isQuitMenuVisible()`.
    fn quit_menu_visible(&self) -> bool;
    /// `TheInGameUI->getGUICommand() != NULL` (a command button waiting for its target).
    fn gui_command_pending(&self) -> bool;
    /// `draw->getTemplate()->isAnyKindOf(pds->kindofsToMatch)`, the kinds of `getPickTypesForContext`.
    fn matches_pick_kinds(&self, draw: DrawableId) -> bool;
    /// `draw->isSelectable()`.
    fn is_selectable(&self, draw: DrawableId) -> bool;
    /// `obj->getContainedBy()`'s id.
    fn contained_by(&self, obj: ObjectId) -> Option<ObjectId>;
    /// `container->getContain()->isEnclosingContainerFor(obj)`; `None` when the container has no contain module.
    fn is_enclosing_container_for(&mut self, container: ObjectId, obj: ObjectId) -> Option<bool>;
    /// `obj->getDrawable()`'s id.
    fn object_drawable(&self, obj: ObjectId) -> Option<DrawableId>;
    /// `obj->getShroudedStatus(localPlayerIndex) >= OBJECTSHROUD_FOGGED`.
    fn fogged_for_local_player(&self, obj: ObjectId) -> bool;
    /// `obj->testStatus(OBJECT_STATUS_STEALTHED) && !obj->testStatus(OBJECT_STATUS_DETECTED)`.
    fn stealthed_and_undetected(&self, obj: ObjectId) -> bool;
    /// `TheMessageStream->appendMessage(t)` with its arguments: a boolean first (`create`), then the object ids.
    fn append_message(&mut self, t: MessageType, create: Option<bool>, objects: &[ObjectId]);
    /// `localPlayer->getAcademyStats()->recordDragSelection()`.
    fn record_drag_selection(&mut self);
    /// `TheInGameUI->clearAttackMoveToMode()`.
    fn clear_attack_move_to_mode(&mut self);
}

/// What `CanSelectDrawable` asks of the game beyond [`SelectionInfoHost`] (#122).
pub trait CanSelectHost: SelectionInfoHost {
    /// `obj->isEffectivelyDead()`.
    fn is_effectively_dead(&self, obj: ObjectId) -> bool;
    /// `draw->isDrawableEffectivelyHidden()`.
    fn is_drawable_effectively_hidden(&self, draw: DrawableId) -> bool;
    /// The window over the drawable's screen point (`TheTacticalView->worldToScreen` of its position,
    /// `getWindowUnderCursor`) and its parents, the window first.
    fn windows_at_drawable(&self, draw: DrawableId) -> Vec<crate::client::in_game_ui_cursor::UiWindow>;
    /// `draw->isKindOf(k)` (the drawable's template; the object's in the client).
    fn drawable_is_kind_of(&self, draw: DrawableId, k: crate::common::kind_of::KindOfType) -> bool;
    /// `obj->getStatusBits().testForAny(OBJECT_STATUS_UNSELECTABLE | OBJECT_STATUS_MASKED)`.
    fn unselectable_or_masked(&self, obj: ObjectId) -> bool;
    /// `obj->isSelectable()`.
    fn object_is_selectable(&self, obj: ObjectId) -> bool;
}

/// `CanSelectDrawable(draw, dragSelecting)` (SelectionXlat.cpp:128, #122): whether `draw` (`None`: `NULL`) may be
/// selected: not without an object, a dead one unless `ALWAYS_SELECTABLE`, a force-attackable one that is not
/// `SELECTABLE`, a hidden one, one under an opaque window, a structure in a drag, one the logic made unselectable or
/// masked, one that is not selectable, or in a drag another player's.
pub fn can_select_drawable(draw: Option<DrawableId>, drag_selecting: bool, h: &impl CanSelectHost) -> bool {
    use crate::common::kind_of::{KINDOF_ALWAYS_SELECTABLE, KINDOF_FORCEATTACKABLE, KINDOF_SELECTABLE};
    let Some(draw) = draw else { return false };
    let Some(obj) = h.drawable_object(draw) else { return false };
    if h.is_effectively_dead(obj) && !h.is_kind_of(obj, KINDOF_ALWAYS_SELECTABLE) {
        // Don't select dead/dying units.
        return false;
    }
    if !h.is_kind_of(obj, KINDOF_SELECTABLE) && h.is_kind_of(obj, KINDOF_FORCEATTACKABLE) {
        return false;
    }
    // hidden objects cannot be selected
    if h.is_drawable_effectively_hidden(draw) {
        return false;
    }
    // ignore objects obscured by the GUI
    if h.windows_at_drawable(draw).iter().any(|w| !w.see_thru) {
        return false;
    }
    // structures cannot be selected by a drag select, you must individually pick them
    if (drag_selecting || crate::logic::modules::plant(9091)) && h.drawable_is_kind_of(draw, KINDOF_STRUCTURE) {
        return false;
    }
    // You cannot select something that has a logic override of unselectability or masked
    if h.unselectable_or_masked(obj) {
        return false;
    }
    if !h.object_is_selectable(obj) {
        return false;
    }
    // In fact the only way you can drag select is if the unit is on your team.
    if drag_selecting && !h.is_locally_controlled(obj) {
        return false;
    }
    true
}

/// `GameMessageDisposition`: what the translator does with the message it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// `KEEP_MESSAGE`: the next translator sees it (the command translator, for a move or an attack).
    Keep,
    /// `DESTROY_MESSAGE`: the click was a selection.
    Destroy,
}

/// `SelectionTranslator`'s state that the left click reads and writes.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionTranslator {
    /// `m_lastGroupSelGroup` (`-1`: none), which the control groups' double press reads.
    pub last_group_sel_group: i32,
    /// `m_lastGroupSelTime` (the logic frame of the last group press; `0`: none).
    pub last_group_sel_time: u32,
    /// #116: `m_leftMouseButtonIsDown`.
    pub left_mouse_button_is_down: bool,
    /// #116: `m_dragSelecting` (a drag selection is under way).
    pub drag_selecting: bool,
    /// #116: `m_selectFeedbackAnchor`, the pixel the left button went down at.
    pub select_feedback_anchor: ICoord2D,
    /// #132: `m_deselectFeedbackAnchor`, the pixel the right button went down at.
    pub deselect_feedback_anchor: ICoord2D,
    /// #132: `m_lastClick`, the mouse's time of the right button's press.
    pub last_click: u32,
    /// #132: `m_deselectDownCameraPosition`, `TheTacticalView->getPosition` at the right button's press.
    pub deselect_down_camera_position: Coord3D,
}

/// #132: `TheMouse`'s three drag tolerances, which the right button's release reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DragTolerances {
    /// `m_dragTolerance` (pixels).
    pub pixels: u32,
    /// `m_dragTolerance3D` (world units the camera may move).
    pub world: u32,
    /// `m_dragToleranceMS` (milliseconds).
    pub ms: u32,
}

impl Default for SelectionTranslator {
    /// The constructor's: no group pressed, the button up, no drag, the anchor at (0, 0); (#132) the right button's
    /// anchor at (0, 0), `m_lastClick` 0 and the camera position zero.
    fn default() -> Self {
        SelectionTranslator {
            last_group_sel_group: -1,
            last_group_sel_time: 0,
            left_mouse_button_is_down: false,
            drag_selecting: false,
            select_feedback_anchor: ICoord2D { x: 0, y: 0 },
            deselect_feedback_anchor: ICoord2D { x: 0, y: 0 },
            last_click: 0,
            deselect_down_camera_position: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
        }
    }
}

/// `addDrawableToList(draw, pds)`: appends `draw` to `list` when the click may pick it. A drawable that is not selectable
/// but is held by a container that does not enclose it (a fire base's gunners) picks the container instead.
///
/// Upstream's fall-through is kept: an unselectable drawable whose object is contained, but enclosed (or whose container has
/// no drawable), is **not** refused here; the click drops it later, because a contained object is never selected.
pub fn add_drawable_to_list(draw: DrawableId, list: &mut Vec<DrawableId>, h: &mut impl SelectionXlatHost) -> bool {
    if !h.matches_pick_kinds(draw) {
        return false;
    }
    if !h.is_selectable(draw) {
        let obj = h.drawable_object(draw);
        match obj.and_then(|o| h.contained_by(o).map(|c| (o, c))) {
            Some((o, container)) => {
                let contain_draw = h.object_drawable(container);
                if let (Some(false), Some(cd)) = (h.is_enclosing_container_for(container, o), contain_draw) {
                    return add_drawable_to_list(cd, list, h);
                }
            }
            None => return false,
        }
    }
    if let Some(obj) = h.drawable_object(draw) {
        let rel = h.relationship_to_local_player(obj);
        if matches!(rel, Relationship::Neutral | Relationship::Enemies)
            && (h.fogged_for_local_player(obj) || h.stealthed_and_undetected(obj))
        {
            return false;
        }
    }
    list.push(draw);
    true
}

impl SelectionTranslator {
    /// #116: the first test of `translateGameMessage`, made for every message: with the input off
    /// (`TheInGameUI->getInputEnabled()` false) the message is kept ("so the other translators (WindowXlat) can handle")
    /// and a drag selection under way ends (`m_dragSelecting`, `TheInGameUI->setSelecting(FALSE)`; the view's
    /// `endAreaSelectHint` and `setMouseLock(FALSE)` are the application's, whose drag box reads
    /// [`SelectionTranslator::drag_selecting`]). `None` when the input is on: the message goes on to its case.
    pub fn input_disabled_gate(&mut self, ui: &mut InGameUiSelection) -> Option<Disposition> {
        if ui.input_enabled() {
            return None;
        }
        if self.drag_selecting && !crate::logic::modules::plant(9025) {
            self.drag_selecting = false;
            ui.set_selecting(false);
        }
        Some(Disposition::Keep)
    }

    /// #116: the `MSG_RAW_MOUSE_LEFT_BUTTON_DOWN` case (after the input gate): the button is down at `pixel`; the drag
    /// cannot start until the pointer moves.
    pub fn left_button_down(&mut self, pixel: ICoord2D, ui: &mut InGameUiSelection) -> Disposition {
        if let Some(d) = self.input_disabled_gate(ui) {
            return d;
        }
        self.left_mouse_button_is_down = true;
        self.select_feedback_anchor = pixel;
        Disposition::Keep
    }

    /// #116: the `MSG_RAW_MOUSE_POSITION` case's drag half (after the input gate): with the button down and the pointer
    /// more than `drag_tolerance` (`TheMouse->m_dragTolerance`) from the anchor on either axis, the drag selection begins
    /// (`TheInGameUI->setSelecting(TRUE)`). The area selection hint ~~and the mouse-over hints (the button up) are the
    /// application's ([`crate::client::command_xlat::CommandTranslator::mouse_over`] is asked directly)~~ is the
    /// application's. **Corrected 2026-11-11 (#122):** the mouse-over message (the button up) is
    /// [`SelectionTranslator::mouseover_message`].
    pub fn mouse_position(&mut self, pixel: ICoord2D, drag_tolerance: i32, ui: &mut InGameUiSelection) -> Disposition {
        if let Some(d) = self.input_disabled_gate(ui) {
            return d;
        }
        if self.left_mouse_button_is_down {
            let dx = (pixel.x - self.select_feedback_anchor.x).abs();
            let dy = (pixel.y - self.select_feedback_anchor.y).abs();
            if (dx > drag_tolerance || dy > drag_tolerance) && !self.drag_selecting {
                self.drag_selecting = true;
                ui.set_selecting(true);
            }
        }
        Disposition::Keep
    }

    /// #122: the `MSG_RAW_MOUSE_POSITION` case's other half (after the input gate and [`SelectionTranslator::mouse_position`]):
    /// with the left button up, the mouse-over message appended for the command translator and `HintSpy`: over `picked`
    /// (the tactical view's `pickDrawable` for the context's pick types, the application's) when its object is alive or
    /// `ALWAYS_SELECTABLE`, else over the ground at `ground` (`screenToTerrain`). `None` with the button down.
    pub fn mouseover_message(&self, picked: Option<DrawableId>, ground: crate::common::base_type::Coord3D, h: &impl CanSelectHost) -> Option<crate::client::in_game_ui_cursor::MouseoverHint> {
        use crate::client::in_game_ui_cursor::MouseoverHint;
        if self.left_mouse_button_is_down {
            return None;
        }
        let under = picked.and_then(|d| h.drawable_object(d).map(|o| (d, o)));
        Some(match under {
            Some((d, o)) if !h.is_effectively_dead(o) || h.is_kind_of(o, crate::common::kind_of::KINDOF_ALWAYS_SELECTABLE) => MouseoverHint::Drawable(d),
            _ => MouseoverHint::Location(ground),
        })
    }

    /// #122: the `MSG_MOUSEOVER_DRAWABLE_HINT` case (SelectionXlat.cpp:549-592, after the input gate): nothing while
    /// scrolling or for a drawable that is gone; else when the context command a click on `draw` would give
    /// (`evaluate_only`: `evaluateContextCommand(draw, draw->getPosition(), EVALUATE_ONLY)`) is `MSG_INVALID`, the hint is
    /// the InGameUI's (`createMouseoverHint`), the message is destroyed (the command translator and `HintSpy` never see
    /// it), and unless the pending GUI command is an attack move or a guard, or `draw` is shrubbery, `TheMouse` shows
    /// `SELECTING` over a selectable drawable and the arrow otherwise.
    pub fn mouseover_drawable_hint<H: crate::client::in_game_ui_cursor::CursorHintHost>(
        &mut self,
        draw: DrawableId,
        ui: &mut InGameUiSelection,
        h: &mut H,
        evaluate_only: impl FnOnce(&mut InGameUiSelection, &mut H) -> MessageType,
    ) -> Disposition {
        use crate::client::control_bar::gui;
        use crate::client::mouse::cursor;
        if let Some(d) = self.input_disabled_gate(ui) {
            return d;
        }
        if ui.is_scrolling() {
            // dont show this now.
            return Disposition::Keep;
        }
        if !h.find_drawable(draw) {
            return Disposition::Keep;
        }
        if evaluate_only(ui, h) != MSG_INVALID {
            return Disposition::Keep;
        }
        ui.create_mouseover_hint(&crate::client::in_game_ui_cursor::MouseoverHint::Drawable(draw), h); // this sets the cursor
        let ignore_command = ui.gui_command().is_some_and(|c| {
            // These GUI commands can take care of themselves -- don't let the selection translator meddle.
            matches!(c.command_type, gui::ATTACK_MOVE | gui::GUARD | gui::GUARD_WITHOUT_PURSUIT | gui::GUARD_FLYING_UNITS_ONLY)
        });
        if !ignore_command && (crate::logic::modules::plant(9092) || !h.drawable_is_kind_of(draw, crate::common::kind_of::KINDOF_SHRUBBERY)) {
            if can_select_drawable(Some(draw), false, h) {
                ui.mouse_set_cursor(cursor::SELECTING);
            } else {
                ui.mouse_set_cursor(cursor::ARROW);
            }
        }
        Disposition::Destroy
    }

    /// #116: the `MSG_RAW_MOUSE_LEFT_BUTTON_UP` case's drag half (after the input gate): the button is up, and a drag
    /// selection under way stops (its `MSG_AREA_SELECTION` region is the application's, which calls
    /// [`SelectionTranslator::left_click`] with the dragged box). Whether a drag ended. The other half (the alternate
    /// mouse's deselection by a left click) is not translated: the alternate mouse is off in the shipped options.
    pub fn left_button_up(&mut self, ui: &mut InGameUiSelection) -> bool {
        if self.input_disabled_gate(ui).is_some() {
            return false;
        }
        self.left_mouse_button_is_down = false;
        if self.drag_selecting {
            self.drag_selecting = false;
            ui.set_selecting(false);
            return true;
        }
        false
    }

    /// #132: the `MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN` case (SelectionXlat.cpp:977-988, after the input gate). "There are three
    /// ways in which we can ignore this as a deselect": the pixel, the time and the camera's position, each kept here for
    /// the release: `pixel` and `time` are the message's arguments 0 and 2, `camera_pos` is
    /// `TheTacticalView->getPosition()`. The message is kept.
    pub fn right_button_down(&mut self, pixel: ICoord2D, time: u32, camera_pos: Coord3D, ui: &mut InGameUiSelection) -> Disposition {
        if let Some(d) = self.input_disabled_gate(ui) {
            return d;
        }
        self.deselect_feedback_anchor = pixel;
        if !crate::logic::modules::plant(9211) {
            self.last_click = time;
        }
        self.deselect_down_camera_position = camera_pos;
        Disposition::Keep
    }

    /// #132: the `MSG_RAW_MOUSE_RIGHT_BUTTON_UP` case (SelectionXlat.cpp:990-1050, after the input gate). The release at
    /// `pixel`, `time` (the message's arguments 0 and 2), with the camera at `camera_pos`, is a click unless the pointer
    /// moved more than `tol.pixels` on an axis from the press, more than `tol.ms` passed (unsigned, so a clock that wrapped
    /// still counts forward), or the camera moved more than `tol.world` (`cameraPos.length()`, a `Real` against the
    /// `UnsignedInt` made `Real`). Upstream's first test reads `isClick && abs(dx) > tol || abs(dy) > tol`, which `&&`
    /// binds first; with `isClick` still true there it is the same as both axes tested, and the port writes it as
    /// upstream does. A click with a GUI command pending cancels it (`setGUICommand(NULL)`), stops the InGameUI's scroll
    /// (`ui_set_scrolling(false)`, `TheInGameUI->setScrolling(FALSE)`, which the camera's owner keeps) and destroys the
    /// message, so the look-at and command translators after it never see the release. Otherwise a click deselects
    /// everything (`deselectAll`: `deselectAllDrawables`, which posts `MSG_DESTROY_SELECTED_GROUP`) in the regular mouse
    /// mode, or in the alternate one (`use_alternate_mouse`) only while a building waits to be placed
    /// (`getPendingPlaceSourceObjectID() != INVALID_ID`). Anything else keeps the message.
    #[allow(clippy::too_many_arguments)]
    pub fn right_button_up(
        &mut self,
        pixel: ICoord2D,
        time: u32,
        camera_pos: Coord3D,
        tol: DragTolerances,
        use_alternate_mouse: bool,
        ui: &mut InGameUiSelection,
        h: &mut impl SelectionUiHost,
        ui_set_scrolling: impl FnOnce(bool),
    ) -> Disposition {
        if let Some(d) = self.input_disabled_gate(ui) {
            return d;
        }
        let mut disp = Disposition::Keep;
        let mut camera_pos = camera_pos;
        camera_pos.sub(&self.deselect_down_camera_position);
        let current_time = time;
        let delta = ICoord2D {
            x: self.deselect_feedback_anchor.x.wrapping_sub(pixel.x),
            y: self.deselect_feedback_anchor.y.wrapping_sub(pixel.y),
        };
        // `abs(int)` compared with an `UnsignedInt`: the int is made unsigned (abs(INT_MIN) stays INT_MIN, 2^31)
        let abs_u = |v: i32| v.wrapping_abs() as u32;
        let mut is_click = true;
        let over = |v: i32| if crate::logic::modules::plant(9220) { abs_u(v) >= tol.pixels } else { abs_u(v) > tol.pixels };
        if (is_click && over(delta.x)) || (over(delta.y) && !crate::logic::modules::plant(9221)) {
            is_click = false;
        }
        let elapsed = if crate::logic::modules::plant(9212) { self.last_click.wrapping_sub(current_time) } else { current_time.wrapping_sub(self.last_click) };
        if is_click && elapsed > tol.ms {
            is_click = false;
        }
        let limit_3d = if crate::logic::modules::plant(9213) { tol.world as f32 + 1.0f32 } else { tol.world as f32 };
        if is_click && camera_pos.length() > limit_3d {
            is_click = false;
        }
        // right click behavior (not right drag)
        if is_click || crate::logic::modules::plant(9214) {
            // "Added support to cancel the GUI command without deselecting the unit(s) involved when you right click."
            if ui.gui_command().is_some() {
                // Cancel GUI command mode... don't deselect units.
                if !crate::logic::modules::plant(9215) {
                    ui.set_gui_command(None);
                }
                // With a GUI command cancel, we want no other behavior.
                if !crate::logic::modules::plant(9216) {
                    disp = Disposition::Destroy;
                }
                if !crate::logic::modules::plant(9217) {
                    ui_set_scrolling(false);
                }
            } else {
                // No GUI command mode, so deselect everyone if we're in regular mouse mode.
                // In alternate mouse mode, right click still cancels building placement.
                let deselect = if crate::logic::modules::plant(9218) {
                    !use_alternate_mouse
                } else if crate::logic::modules::plant(9219) {
                    !use_alternate_mouse && ui.pending_place_source().is_some()
                } else {
                    !use_alternate_mouse || ui.pending_place_source().is_some()
                };
                if deselect {
                    ui.deselect_all_drawables(true, h);
                }
            }
        }
        disp
    }

    /// The `MSG_MOUSE_LEFT_CLICK` case: `in_region` are the drawables the view finds in the click's pixel region (a point
    /// when `is_point`), before [`add_drawable_to_list`]'s filter. #116: after the input gate, which keeps the click (for
    /// the command translator, whose own gate destroys it).
    pub fn left_click<H: CommandXlatHost>(
        &mut self,
        in_region: &[DrawableId],
        is_point: bool,
        cmd: &CommandTranslator,
        ui: &mut InGameUiSelection,
        h: &mut H,
    ) -> Disposition {
        if let Some(d) = self.input_disabled_gate(ui) {
            return d;
        }
        if h.quit_menu_visible() {
            return Disposition::Destroy;
        }
        let mut will_select = Vec::new();
        for &d in in_region {
            add_drawable_to_list(d, &mut will_select, h);
        }
        if will_select.is_empty() {
            return Disposition::Keep;
        }
        let current = ui.all_selected_drawables().to_vec();
        if h.gui_command_pending() {
            return Disposition::Keep;
        }
        let mut si = SelectionInfo::default();
        let shown: &mut InGameUiSelection = ui;
        let mut valid = |h: &mut H, d| {
            let pos = h.drawable_object(d).map(|o| h.object_position(o)).unwrap_or_default();
            cmd.evaluate_context_command(Some(d), &pos, CommandEvaluateType::EvaluateOnly, shown, h) != MSG_INVALID
        };
        if context_command_for_new_selection(&current, &will_select, &mut si, is_point, h, &mut valid) {
            return Disposition::Keep;
        }
        let mut add_to_group = ui.is_in_prefer_selection_mode();
        if si.current_count_enemies > 0
            || si.current_count_civilians > 0
            || si.current_count_friends > 0
            || si.current_count_mine_buildings > 0
        {
            add_to_group = false;
        }
        if si.new_count_mine > 0 {
            si.select_mine = true;
            if si.new_count_mine_buildings == 1 && si.new_count_mine == 1 {
                add_to_group = false;
                si.select_mine_buildings = true;
            } else if si.new_count_mine_buildings > 0 {
                // a drag over a building whose other drawables cannot be selected anyway gets the building
                let mut only_the_one_building = true;
                let mut building = None;
                for &d in &will_select {
                    if h.drawable_object(d).is_some_and(|o| h.is_kind_of(o, KINDOF_STRUCTURE)) {
                        match building {
                            None => building = Some(d),
                            Some(b) if b != d => only_the_one_building = false,
                            _ => {}
                        }
                    } else if h.is_selectable(d) {
                        only_the_one_building = false;
                    }
                    if !only_the_one_building {
                        break;
                    }
                }
                if only_the_one_building {
                    add_to_group = false;
                    si.select_mine_buildings = true;
                }
            }
        } else if si.new_count_enemies > 0 && si.new_count_civilians > 0 && si.new_count_friends > 0 {
            return Disposition::Keep;
        } else if si.new_count_enemies == 1 {
            add_to_group = false;
            si.select_enemies = true;
        } else if si.new_count_civilians == 1 {
            add_to_group = false;
            si.select_civilians = true;
        } else if si.new_count_friends == 1 {
            add_to_group = false;
            si.select_friends = true;
        }
        if !(si.select_mine || si.select_enemies || si.select_civilians || si.select_friends) {
            return Disposition::Keep;
        }
        self.last_group_sel_group = -1;
        if ui.is_in_prefer_selection_mode() && is_point && will_select.iter().all(|&d| ui.is_selected(d)) {
            // shift and a point on what is already selected: deselect it
            let mut ids = Vec::new();
            for &d in &will_select {
                let Some(obj) = h.drawable_object(d) else { continue };
                ids.push(obj);
                ui.deselect_drawable(d, h);
            }
            h.append_message(MSG_REMOVE_FROM_SELECTED_GROUP, None, &ids);
        } else {
            if !add_to_group {
                ui.deselect_all_drawables(true, h);
            }
            let mut ids = Vec::new();
            for &d in &will_select {
                let Some(obj) = h.drawable_object(d) else { continue };
                if h.contained_by(obj).is_some() {
                    // contained, and so not selectable
                    continue;
                }
                let take = if si.select_mine && h.is_locally_controlled(obj) {
                    !h.is_kind_of(obj, KINDOF_STRUCTURE) || si.select_mine_buildings
                } else {
                    match h.relationship_to_local_player(obj) {
                        Relationship::Enemies => si.select_enemies,
                        Relationship::Neutral => si.select_civilians,
                        Relationship::Allies => si.select_friends,
                    }
                };
                if take {
                    ids.push(obj);
                    ui.select_drawable(d, h);
                }
            }
            h.append_message(MSG_CREATE_SELECTED_GROUP, Some(!add_to_group), &ids);
            if ids.len() > 1 {
                h.record_drag_selection();
            }
        }
        h.clear_attack_move_to_mode();
        Disposition::Destroy
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::common::kind_of::KindOfType;
    use crate::common::message_types::MSG_DESTROY_SELECTED_GROUP;
    use std::collections::BTreeMap;

    /// A view's facts: drawable `d`'s object is `d`. `mine` are locally controlled; the others' relationships are `rel`.
    #[derive(Default)]
    pub(crate) struct Host {
        pub mine: Vec<ObjectId>,
        pub rel: BTreeMap<ObjectId, Relationship>,
        pub structures: Vec<ObjectId>,
        pub fogged: Vec<ObjectId>,
        pub messages: Vec<(MessageType, Option<bool>, Vec<ObjectId>)>,
        pub drags: u32,
        pub frame: u32,
        pub all: Vec<DrawableId>,
        pub screen: Vec<DrawableId>,
        pub picked: Option<DrawableId>,
        pub templates: BTreeMap<ObjectId, u32>,
        pub squads: BTreeMap<i32, Vec<ObjectId>>,
        pub looks: Vec<ObjectId>,
        pub ui_messages: Vec<Vec<u8>>,
        /// The command translator's facts (C2b).
        pub cmd: crate::client::command_xlat::tests::CmdFacts,
        /// The control bar's facts (C2c).
        pub bar: crate::client::control_bar::tests::BarFacts,
        /// Placement's facts (C2c part 2).
        pub place: crate::client::place_event::tests::PlaceFacts,
        /// #54: the objects that are `KINDOF_SELECTABLE` (the GUI command translator's pick type).
        pub selectable: Vec<ObjectId>,
        /// #125: the meta cases' facts.
        pub meta: crate::client::command_xlat_meta::tests::MetaFacts,
    }

    impl SelectionInfoHost for Host {
        fn drawable_object(&self, draw: DrawableId) -> Option<ObjectId> {
            Some(draw)
        }
        fn is_locally_controlled(&self, obj: ObjectId) -> bool {
            self.mine.contains(&obj)
        }
        fn is_kind_of(&self, obj: ObjectId, k: KindOfType) -> bool {
            (k == KINDOF_STRUCTURE && self.structures.contains(&obj)) || (k == crate::common::kind_of::KINDOF_SELECTABLE && self.selectable.contains(&obj))
        }
        fn relationship_to_local_player(&self, obj: ObjectId) -> Relationship {
            if self.mine.contains(&obj) {
                return Relationship::Allies;
            }
            self.rel.get(&obj).copied().unwrap_or(Relationship::Neutral)
        }
        fn can_local_player_garrison(&mut self, _obj: ObjectId) -> bool {
            false
        }
        fn force_fire_or_move(&self) -> bool {
            false
        }
        fn prefer_selection(&self) -> bool {
            false
        }
        fn use_alternate_mouse(&self) -> bool {
            // (#124: off but where a command translator test turns it on)
            self.cmd.alternate_mouse
        }
    }

    impl SelectionUiHost for Host {
        fn logic_frame(&self) -> u32 {
            self.frame
        }
        fn append_message_bool(&mut self, t: MessageType, b: bool) {
            self.messages.push((t, Some(b), Vec::new()));
        }
        fn on_selection_changed(&mut self, _draw: DrawableId, _selected: bool) {}
    }

    impl SelectionXlatHost for Host {
        fn quit_menu_visible(&self) -> bool {
            false
        }
        fn gui_command_pending(&self) -> bool {
            false
        }
        fn matches_pick_kinds(&self, _draw: DrawableId) -> bool {
            true
        }
        fn is_selectable(&self, _draw: DrawableId) -> bool {
            true
        }
        fn contained_by(&self, _obj: ObjectId) -> Option<ObjectId> {
            None
        }
        fn is_enclosing_container_for(&mut self, _container: ObjectId, _obj: ObjectId) -> Option<bool> {
            None
        }
        fn object_drawable(&self, obj: ObjectId) -> Option<DrawableId> {
            Some(obj)
        }
        fn fogged_for_local_player(&self, obj: ObjectId) -> bool {
            self.fogged.contains(&obj)
        }
        fn stealthed_and_undetected(&self, _obj: ObjectId) -> bool {
            false
        }
        fn append_message(&mut self, t: MessageType, create: Option<bool>, objects: &[ObjectId]) {
            self.messages.push((t, create, objects.to_vec()));
        }
        fn record_drag_selection(&mut self) {
            self.drags += 1;
        }
        fn clear_attack_move_to_mode(&mut self) {}
    }

    /// C2a: the left click. Predicted from SelectionXlat.cpp, with my units 1, 2, my building 5, an enemy 9 and a fogged
    /// enemy 8:
    /// - a click on unit 1: deselect all (`MSG_DESTROY_SELECTED_GROUP(true)`), then `MSG_CREATE_SELECTED_GROUP(true, 1)`;
    /// - a drag over 1, 2 and the building 5: the units only (the building is not the only one), a new group `(true, 1, 2)`
    ///   and one drag selection recorded;
    /// - with shift (prefer selection) a click on unit 1, already selected: `MSG_REMOVE_FROM_SELECTED_GROUP(1)`;
    /// - a click on the fogged enemy picks nothing: the message is kept (the command translator's);
    /// - a click on the enemy 9 with nothing of mine selected: it is selected `(true, 9)`.
    #[test]
    fn a_click_or_a_drag_makes_the_selection_messages() {
        let mut h = Host { mine: vec![1, 2, 5], structures: vec![5], fogged: vec![8], ..Default::default() };
        h.rel.insert(9, Relationship::Enemies);
        h.rel.insert(8, Relationship::Enemies);
        let (mut x, mut ui) = (SelectionTranslator::default(), InGameUiSelection::new());
        assert_eq!(x.left_click(&[1], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(
            h.messages,
            vec![(MSG_DESTROY_SELECTED_GROUP, Some(true), vec![]), (MSG_CREATE_SELECTED_GROUP, Some(true), vec![1])]
        );
        h.messages.clear();
        assert_eq!(x.left_click(&[1, 2, 5], false, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(h.messages.last(), Some(&(MSG_CREATE_SELECTED_GROUP, Some(true), vec![1, 2])));
        assert_eq!((ui.all_selected_drawables(), h.drags), (&[2, 1][..], 1));
        h.messages.clear();
        ui.set_prefer_selection_mode(true);
        assert_eq!(x.left_click(&[1], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(h.messages, vec![(MSG_REMOVE_FROM_SELECTED_GROUP, None, vec![1])]);
        assert_eq!(ui.all_selected_drawables(), &[2][..]);
        ui.set_prefer_selection_mode(false);
        ui.deselect_all_drawables(false, &mut h);
        h.messages.clear();
        assert_eq!(x.left_click(&[8], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Keep);
        assert!(h.messages.is_empty());
        assert_eq!(x.left_click(&[9], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(h.messages.last(), Some(&(MSG_CREATE_SELECTED_GROUP, Some(true), vec![9])));
    }

    /// #116: the raw button's drag and the input gate. Predicted from SelectionXlat.cpp, with the drag tolerance 4: a
    /// press at (10, 10) and a move to (13, 14) is not a drag; a move to (10, 15) starts one (`setSelecting(TRUE)`);
    /// the release ends it and answers that a drag ended. Pressed and dragged again, the input turned off (the InGameUI's
    /// own `setInputEnabled` already clears `isSelecting`): the next message's gate ends the drag and keeps the message,
    /// and a left click on unit 1 with the input off is kept with no message and no selection; a release then reports
    /// no drag. With the input on again the click selects.
    #[test]
    fn the_input_gate_ends_a_drag_and_keeps_the_message() {
        let mut h = Host { mine: vec![1], ..Default::default() };
        let (mut x, mut ui) = (SelectionTranslator::default(), InGameUiSelection::new());
        let at = |x: i32, y: i32| ICoord2D { x, y };
        assert_eq!(x.left_button_down(at(10, 10), &mut ui), Disposition::Keep);
        x.mouse_position(at(13, 14), 4, &mut ui);
        assert!(!x.drag_selecting && !ui.is_selecting());
        x.mouse_position(at(10, 15), 4, &mut ui);
        assert!(x.drag_selecting && ui.is_selecting());
        assert!(x.left_button_up(&mut ui));
        assert!(!x.drag_selecting && !ui.is_selecting() && !x.left_mouse_button_is_down);
        x.left_button_down(at(0, 0), &mut ui);
        x.mouse_position(at(50, 50), 4, &mut ui);
        assert!(x.drag_selecting);
        ui.set_input_enabled(false);
        assert!(!ui.is_selecting());
        assert_eq!(x.mouse_position(at(60, 60), 4, &mut ui), Disposition::Keep);
        assert!(!x.drag_selecting);
        assert_eq!(x.left_click(&[1], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Keep);
        assert!(h.messages.is_empty() && ui.select_count() == 0);
        assert!(!x.left_button_up(&mut ui));
        ui.set_input_enabled(true);
        assert_eq!(x.left_click(&[1], true, &CommandTranslator::default(), &mut ui, &mut h), Disposition::Destroy);
        assert_eq!(ui.all_selected_drawables(), &[1]);
    }
}
