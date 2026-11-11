// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (setMouseCursor, createMouseoverHint, createCommandHint,
//     triggerDoubleClickAttackMoveGuardHint's timer)
//   GeneralsMD/Code/GameEngine/Include/GameClient/InGameUI.h (MouseMode, setRadiusCursorNone)
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/HintSpy.cpp (which hints reach createCommandHint)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The cursors the in-game UI asks for (GitHub #122): `InGameUI::createMouseoverHint` and `createCommandHint`, which
//! `HintSpyTranslator` (priority 100) calls for the hints the selection and command translators made, and the mouse
//! mode they read (`setGUICommand` and `placeBuildAvailable` set it: [`InGameUiSelection::set_gui_command`],
//! [`InGameUiSelection::place_build_available`]).
//!
//! # As upstream reads it
//!
//! **`setMouseCursor(c)`**: `TheMouse->setCursor(c)`; in `MOUSEMODE_GUI_COMMAND` a cursor other than the arrow and the
//! scroll one becomes `m_mouseModeCursor`.
//!
//! **`createMouseoverHint(msg)`** (`MSG_MOUSEOVER_DRAWABLE_HINT` or `MSG_MOUSEOVER_LOCATION_HINT`): nothing while scrolling
//! or drag selecting. The window under the mouse is walked to its parents: a window whose input function is
//! `LeftHUDInput` (the radar) ends the walk with nothing in the way; one that is not `WIN_STATUS_SEE_THRU` ends it with
//! the cursor over a window, which sets the arrow (whatever the mouse mode) and returns. Over a drawable the cursor's
//! tooltip is first cleared and `m_mousedOverDrawableID` reset; a drawable with an object becomes the moused-over one
//! (an `IGNORED_IN_GUI` mob member makes its slaver's drawable the moused-over one instead, or none) and its tooltip is
//! made: the controlling player (the container's apparent one first), a disguiser's disguise for a player not allied to
//! the local one, the display name (or `ThingTemplate:<name>` fetched), a supply warehouse's value, the player's name in
//! a multiplayer game; set, with the player's or the indicator's colour (a garrisonable container's apparent player's),
//! only when the object's cell is clear for the local player and the name is not `OBJECT:Prop`'s. Over the ground the
//! moused-over drawable is none. A change of it resets the tooltip delay. Then, in `MOUSEMODE_DEFAULT`, with nothing
//! selected and (in a playback) the mouse moved recently, the cursor is `SELECTING` over a selectable drawable of the
//! local player (`CanSelectDrawable(draw, FALSE)`) and the arrow otherwise; in `MOUSEMODE_GUI_COMMAND` it is the mode's
//! cursor; in `MOUSEMODE_BUILD_PLACE`, or in the default mode with a selection, it is left alone.
//!
//! **`createCommandHint(msg)`** (the command translator's `*_HINT` messages): nothing while scrolling, drag selecting or
//! playing back. The attack hints over an object shrouded for the local player become `MSG_DO_MOVETO_HINT`. The radius
//! cursor is cleared; with `m_doubleClickAttackMove` on, the guard hint's timer is decremented and while it is still
//! above zero the cursor is `FORCE_ATTACK_GROUND` with the guard area's radius cursor, and nothing else happens. The
//! window walk is `createMouseoverHint`'s. Then by mouse mode: **default**: the arrow over a window or with one selected
//! object that is not the local player's; else a cursor for the hint (the move hint's `GENERIC_INVALID` for the local
//! player's lone structure off a selectable drawable, `SELECTING` over the local player's selectable non-mine, the arrow
//! over the radar window while the radar is not forced and is hidden or missing for the local player, else `MOVETO`;
//! and so on, see [`InGameUiSelection::create_command_hint`]); **build placement**: the arrow over a window, else
//! `BUILD_PLACEMENT` for the move hints and `INVALID_BUILD_PLACEMENT` for the attack ones; **GUI command**: the arrow over
//! a window, else for a context command or a special power the command's cursor (`MSG_VALID_GUICOMMAND_HINT`) or its
//! invalid cursor (any other hint), by name through `getCursorIndex`, `CROSS` when the name is not a cursor, with the
//! command's radius cursor; for a command needing a target its cursor likewise; else no radius cursor.
//!
//! # Upstream quirks that are kept
//!
//! - **The build placement mode tests `MSG_ADD_WAYPOINT`, not `MSG_ADD_WAYPOINT_HINT`**: the waypoint hint while placing
//!   a building sets no cursor (the cursor stays as it was).
//! - **`MSG_DO_ATTACK_OBJECT_HINT` over a shrouded object becomes the move hint before the mode is read**, so in the
//!   build placement mode it shows `BUILD_PLACEMENT`, not the invalid placement.
//! - **The guard hint's timer is decremented by every command hint** while `m_doubleClickAttackMove` is on, below zero
//!   too (`--timer > 0`): it is not reset by the hints; only `triggerDoubleClickAttackMoveGuardHint` sets it (to 11).
//! - **The radar's arrow test reads the window the walk stopped at**, which over a see-through window chain is none:
//!   `isRadarWindow(NULL)` is false, so the move hint over see-through windows shows `MOVETO`.
//! - `createMouseoverHint` leaves the cursor alone in the default mode while anything is selected (the command hint
//!   that follows sets it), and while playing back with the mouse still.
//! - The tooltip is made for the hovered drawable's object even when a mob member makes the slaver the moused-over one.
//!
//! # Where this differs from upstream
//!
//! - **The calls on `TheMouse` and the radius cursor are queued** ([`UiMouseCall`] in
//!   [`InGameUiSelection::take_mouse_calls`]) and made on the client's mouse by the application, in order, once a frame
//!   before `TheMouse->draw()` (the cursor shows only at the next draw upstream too, see `client::mouse`).
//!   `TheMouse->getMouseCursor()` right after `setMouseCursor(c)`, which `setGUICommand` reads, is `c`
//!   (`W3DMouse::setCursor` stores the cursor whatever else it does): the last queued cursor.
//! - **The window under the mouse** (`TheWindowManager->getWindowUnderCursor` and its parents) is the host's list of
//!   [`UiWindow`]s, from the window under the cursor up; `TheRadar->isRadarWindow(w)` is [`UiWindow::radar`].
//! - **The tooltip** is the text and colour `setCursorTooltip` was given ([`UiMouseCall::Tooltip`]); drawing it is the
//!   application's (not wired yet: #122 gaps). The disguise, containers, warehouse and text are the host's questions.
//! - `m_isScrolling` is a copy of the camera's (`LookAtTranslator` keeps the flag `setScrolling` writes).
//! - The debug builds' branches (`_DEBUG`/`_INTERNAL`'s constant debug update and shroud switch, `AI_DEBUG_TOOLTIPS`)
//!   are not translated: the release build has none of them.
//!
//! # Representation
//!
//! The InGameUI's members are [`InGameUiSelection`]'s (`mouse_mode`, `mouse_mode_cursor`, `moused_over_drawable`,
//! `is_scrolling`, the guard hint's timer, the queued calls). Cursors are `client::mouse::cursor`'s numbers; a drawable
//! is its id (`None`: `INVALID_DRAWABLE_ID`, and a drawable the host does not find is `findDrawableByID`'s `NULL`).
//! `tests/differential_cursor_hint.rs` compares all of it with the C++.

use crate::client::control_bar::{gui, opt, GuiButton};
use crate::client::in_game_ui::{DrawableId, InGameUiSelection};
use crate::client::mouse::{cursor, get_cursor_index};
use crate::client::selection_xlat::{can_select_drawable, CanSelectHost};
use crate::common::base_type::Coord3D;
use crate::common::game_common::ObjectShroudStatus;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{KINDOF_DISGUISER, KINDOF_IGNORED_IN_GUI, KINDOF_MINE, KINDOF_STRUCTURE};
use crate::common::message_stream::MessageType;
use crate::common::message_types::*;
use crate::logic::modules::plant;

/// `InGameUI::MouseMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseMode {
    /// `MOUSEMODE_DEFAULT`.
    #[default]
    Default = 0,
    /// `MOUSEMODE_BUILD_PLACE`.
    BuildPlace = 1,
    /// `MOUSEMODE_GUI_COMMAND`.
    GuiCommand = 2,
}

/// `RADIUSCURSOR_NONE`, `RADIUSCURSOR_GUARD_AREA` (`enum RadiusCursorType`).
pub const RADIUSCURSOR_NONE: i32 = 0;
pub const RADIUSCURSOR_GUARD_AREA: i32 = 4;
/// `PRIMARY_WEAPON`.
pub const PRIMARY_WEAPON: i32 = 0;

/// A call the InGameUI makes on `TheMouse` or on its own radius cursor, queued for the owner of the mouse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiMouseCall {
    /// `TheMouse->setCursor(c)` (from `setMouseCursor`, or made directly).
    SetCursor(i32),
    /// `TheMouse->setCursorTooltip(text, -1, &rgb)`; `color` is `None` for `setCursorTooltip(TheEmptyString)`.
    Tooltip { text: Vec<u16>, color: Option<i32> },
    /// `TheMouse->resetTooltipDelay()`.
    ResetTooltipDelay,
    /// `TheMouse->capture()`.
    Capture,
    /// `TheMouse->releaseCapture()`.
    ReleaseCapture,
    /// `setRadiusCursor(type, specialPowerTemplate, weaponSlot)` (the template's id; `setRadiusCursorNone()` is
    /// [`UiMouseCall::RADIUS_CURSOR_NONE`]).
    RadiusCursor { kind: i32, special_power: Option<u32>, weapon_slot: i32 },
}

impl UiMouseCall {
    /// `setRadiusCursorNone()`: `setRadiusCursor(RADIUSCURSOR_NONE, NULL, PRIMARY_WEAPON)`.
    pub const RADIUS_CURSOR_NONE: UiMouseCall = UiMouseCall::RadiusCursor { kind: RADIUSCURSOR_NONE, special_power: None, weapon_slot: PRIMARY_WEAPON };

    /// `setRadiusCursor(command->getRadiusCursorType(), command->getSpecialPowerTemplate(), command->getWeaponSlot())`.
    pub fn radius_cursor_of(c: &GuiButton) -> UiMouseCall {
        UiMouseCall::RadiusCursor { kind: c.radius_cursor, special_power: c.special_power.map(|(id, _)| id), weapon_slot: c.weapon_slot }
    }
}

/// A window under the mouse, as the hints' walk reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiWindow {
    /// `winGetInputFunc() == LeftHUDInput`.
    pub left_hud_input: bool,
    /// `BitTest(winGetStatus(), WIN_STATUS_SEE_THRU)`.
    pub see_thru: bool,
    /// `TheRadar->isRadarWindow(window)`.
    pub radar: bool,
}

/// The walk of `createMouseoverHint` and `createCommandHint` over `windows` (the window under the cursor, then its
/// parents): whether the cursor is over a window that keeps it from the world (`underWindow`), and the window the walk
/// stopped at (`None` when it walked off the top, or there was none).
pub fn window_walk(windows: &[UiWindow]) -> (bool, Option<UiWindow>) {
    for w in windows {
        if w.left_hud_input {
            return (false, Some(*w));
        }
        // check to see if it or any of its parents are opaque.  If so, we can't select anything.
        if !w.see_thru {
            return (true, Some(*w));
        }
    }
    (false, None)
}

/// The mouse-over message: `MSG_MOUSEOVER_DRAWABLE_HINT` with its drawable, or `MSG_MOUSEOVER_LOCATION_HINT`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseoverHint {
    Drawable(DrawableId),
    Location(Coord3D),
}

/// `ContainModuleInterface`'s answers for the tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainInfo {
    /// `getApparentControllingPlayer(ThePlayerList->getLocalPlayer())`'s index (`None`: `NULL`).
    pub apparent_controlling_player: Option<i32>,
    /// `isGarrisonable()`.
    pub garrisonable: bool,
}

/// A disguiser's `StealthUpdate` answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisguiseInfo {
    /// `isDisguised()`.
    pub disguised: bool,
    /// `getDisguisedPlayerIndex()`.
    pub player_index: i32,
    /// `getDisguisedTemplate()`'s id (`None`: `NULL`).
    pub template: Option<u16>,
}

/// What the hints ask of the game beyond [`CanSelectHost`]'s questions.
pub trait CursorHintHost: CanSelectHost {
    /// `TheGameClient->findDrawableByID(id)` is not `NULL`.
    fn find_drawable(&self, draw: DrawableId) -> bool;
    /// `TheWindowManager->getWindowUnderCursor(mouse)` and its parents, the window under the cursor first.
    fn windows_under_cursor(&self) -> Vec<UiWindow>;
    /// `TheRecorder->getMode() == RECORDERMODETYPE_PLAYBACK`.
    fn recorder_playback(&self) -> bool;
    /// `TheLookAtTranslator->hasMouseMovedRecently()`.
    fn mouse_moved_recently(&mut self) -> bool;
    /// `TheGlobalData->m_doubleClickAttackMove`.
    fn double_click_attack_move(&self) -> bool;
    /// `TheRadar->isRadarForced()`.
    fn radar_forced(&self) -> bool;
    /// `TheRadar->isRadarHidden()`.
    fn radar_hidden(&self) -> bool;
    /// `ThePlayerList->getLocalPlayer()->hasRadar()`.
    fn local_player_has_radar(&self) -> bool;
    /// `obj->getShroudedStatus(localPlayerIndex)`.
    fn object_shroud_status_for_local_player(&self, obj: ObjectId) -> ObjectShroudStatus;
    /// `(MobMemberSlavedUpdate*)obj->findUpdateModule("MobMemberSlavedUpdate")`'s `getSlaverID()`; `None` without the module.
    fn mob_member_slaver(&self, obj: ObjectId) -> Option<ObjectId>;
    /// `TheGameLogic->findObjectByID(id)`'s `getDrawable()`: `None` when either is `NULL`.
    fn object_drawable_of(&self, obj: ObjectId) -> Option<DrawableId>;

    // ---- the tooltip ----
    /// `obj->getContain()`'s answers; `None` without a contain module.
    fn contain_info(&mut self, obj: ObjectId) -> Option<ContainInfo>;
    /// `obj->getControllingPlayer()`'s index.
    fn controlling_player(&self, obj: ObjectId) -> Option<i32>;
    /// `obj->getStealth()`'s disguise; `None` without a stealth module.
    fn disguise(&self, obj: ObjectId) -> Option<DisguiseInfo>;
    /// `player->getRelationship(ThePlayerList->getLocalPlayer()->getDefaultTeam()) == ALLIES`.
    fn player_allied_to_local_default_team(&self, player: i32) -> bool;
    /// `ThePlayerList->getLocalPlayer()->isPlayerActive()`.
    fn local_player_active(&self) -> bool;
    /// `obj->getTemplate()`'s id.
    fn object_template(&self, obj: ObjectId) -> u16;
    /// `thingTemplate->getDisplayName()`.
    fn template_display_name(&self, template: u16) -> Vec<u16>;
    /// `thingTemplate->getName()`.
    fn template_name(&self, template: u16) -> Vec<u8>;
    /// `TheGameText->fetch(label)`.
    fn fetch(&self, label: &[u8]) -> Vec<u16>;
    /// `SupplyWarehouseDockUpdate`'s `getBoxesStored()`; `None` without the module.
    fn warehouse_boxes(&mut self, obj: ObjectId) -> Option<i32>;
    /// `TheGlobalData->m_baseValuePerSupplyBox`.
    fn base_value_per_supply_box(&self) -> i32;
    /// `UnicodeString::format(format, value)` with one integer.
    fn format_int(&self, format: &[u16], value: i32) -> Vec<u16>;
    /// `TheRecorder->isMultiplayer()`.
    fn recorder_multiplayer(&self) -> bool;
    /// `player->isPlayableSide()`.
    fn player_is_playable_side(&self, player: i32) -> bool;
    /// `player->getPlayerDisplayName()`.
    fn player_display_name(&self, player: i32) -> Vec<u16>;
    /// The object's cell (`worldToCell` of its position) is `CELLSHROUD_CLEAR` for the local player.
    fn object_cell_clear_for_local_player(&self, obj: ObjectId) -> bool;
    /// `player->getPlayerColor()`.
    fn player_color(&self, player: i32) -> i32;
    /// `obj->getIndicatorColor()`.
    fn indicator_color(&self, obj: ObjectId) -> i32;
}

/// HintSpy's list: whether `t` reaches `createCommandHint` (`MSG_DEFECTOR_HINT` falls through to it).
pub fn hint_spy_creates_command_hint(t: MessageType) -> bool {
    matches!(
        t,
        MSG_DEFECTOR_HINT
            | MSG_DO_MOVETO_HINT
            | MSG_DO_ATTACKMOVETO_HINT
            | MSG_DO_ATTACK_OBJECT_HINT
            | MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT
            | MSG_DO_FORCE_ATTACK_OBJECT_HINT
            | MSG_DO_FORCE_ATTACK_GROUND_HINT
            | MSG_ADD_WAYPOINT_HINT
            | MSG_GET_REPAIRED_HINT
            | MSG_DOCK_HINT
            | MSG_GET_HEALED_HINT
            | MSG_DO_REPAIR_HINT
            | MSG_RESUME_CONSTRUCTION_HINT
            | MSG_ENTER_HINT
            | MSG_HIJACK_HINT
            | MSG_SABOTAGE_HINT
            | MSG_CONVERT_TO_CARBOMB_HINT
            | MSG_VALID_GUICOMMAND_HINT
            | MSG_INVALID_GUICOMMAND_HINT
            | MSG_CAPTUREBUILDING_HINT
            | MSG_HACK_HINT
            | MSG_SET_RALLY_POINT_HINT
            | MSG_IMPOSSIBLE_ATTACK_HINT
            | MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT
            | MSG_DO_SALVAGE_HINT
            | MSG_DO_INVALID_HINT
    )
}

fn concat(a: &[u16], b: &[u16]) -> Vec<u16> {
    let mut v = a.to_vec();
    v.extend_from_slice(b);
    v
}

impl InGameUiSelection {
    /// `getMouseMode()` (#122).
    pub fn mouse_mode(&self) -> MouseMode {
        self.mouse_mode
    }
    /// `m_mouseModeCursor` (#122).
    pub fn mouse_mode_cursor(&self) -> i32 {
        self.mouse_mode_cursor
    }
    /// `getMousedOverDrawableID()` (#122).
    pub fn moused_over_drawable(&self) -> Option<DrawableId> {
        self.moused_over_drawable
    }
    /// #122: `m_isScrolling` as the camera's `LookAtTranslator` left it.
    pub fn set_is_scrolling(&mut self, scrolling: bool) {
        self.is_scrolling = scrolling;
    }
    /// `isScrolling()`.
    pub fn is_scrolling(&self) -> bool {
        self.is_scrolling
    }
    /// #122: `TheRecorder->getMode() == RECORDERMODETYPE_PLAYBACK` for `setGUICommand`'s guard (the hints ask their host).
    pub fn set_recorder_playback(&mut self, playback: bool) {
        self.recorder_playback = playback;
    }
    /// `triggerDoubleClickAttackMoveGuardHint()`'s timer (its stashed position is the radius cursor's, not here).
    pub fn trigger_double_click_attack_move_guard_hint(&mut self) {
        self.double_click_attack_move_guard_hint_timer = 11;
    }
    /// `m_duringDoubleClickAttackMoveGuardHintTimer`.
    pub fn double_click_attack_move_guard_hint_timer(&self) -> i32 {
        self.double_click_attack_move_guard_hint_timer
    }

    /// The calls on `TheMouse` and the radius cursor since the last take, in order.
    pub fn take_mouse_calls(&mut self) -> Vec<UiMouseCall> {
        std::mem::take(&mut self.mouse_calls)
    }

    /// `TheMouse->getMouseCursor()` after the queued calls: the last cursor queued.
    pub(crate) fn last_mouse_cursor_set(&self) -> Option<i32> {
        self.mouse_calls.iter().rev().find_map(|c| match c {
            UiMouseCall::SetCursor(c) => Some(*c),
            _ => None,
        })
    }

    /// `TheMouse->setCursor(c)` made directly (not through `setMouseCursor`), as `SelectionTranslator`'s mouse-over does.
    pub fn mouse_set_cursor(&mut self, c: i32) {
        self.mouse_calls.push(UiMouseCall::SetCursor(c));
    }

    /// `setMouseCursor(c)`: `TheMouse->setCursor(c)` (queued), and in `MOUSEMODE_GUI_COMMAND` a cursor other than the arrow
    /// and the scroll one becomes `m_mouseModeCursor` (`client::mouse::remember_mode_cursor`, shared with the camera's
    /// path).
    pub fn set_mouse_cursor(&mut self, c: i32) {
        // (`if (!TheMouse) return;`: the client's mouse always exists)
        self.mouse_calls.push(UiMouseCall::SetCursor(c));
        crate::client::mouse::remember_mode_cursor(self.mouse_mode == MouseMode::GuiCommand, &mut self.mouse_mode_cursor, c);
    }

    /// `createMouseoverHint(msg)`; see the module docs.
    pub fn create_mouseover_hint(&mut self, msg: &MouseoverHint, h: &mut impl CursorHintHost) {
        if self.is_scrolling || self.is_selecting() {
            return; // no mouseover for you
        }
        let (under_window, _) = window_walk(&h.windows_under_cursor());
        if under_window {
            self.set_mouse_cursor(cursor::ARROW); // regardless of m_mouseMode
            return;
        }
        let old_id = self.moused_over_drawable;
        if let MouseoverHint::Drawable(id) = *msg {
            self.mouse_calls.push(UiMouseCall::Tooltip { text: Vec::new(), color: None });
            self.moused_over_drawable = None;
            let obj = if h.find_drawable(id) { h.drawable_object(id) } else { None };
            if let Some(obj) = obj {
                // Ahh, here is a wierd exception: if the moused-over drawable is a mob-member (e.g. AngryMob), Lets fool
                // the UI into creating the hint for the NEXUS instead...
                if h.is_kind_of(obj, KINDOF_IGNORED_IN_GUI) && !plant(9088) {
                    if let Some(slaver) = h.mob_member_slaver(obj) {
                        // if this fails, not to worry... it has already defaulted to INVALID_DRAWABLE_ID, above
                        if let Some(d) = h.object_drawable_of(slaver) {
                            self.moused_over_drawable = Some(d);
                        }
                    }
                } else {
                    self.moused_over_drawable = Some(id);
                }
                self.mouseover_tooltip(obj, h);
            }
        } else {
            self.moused_over_drawable = None;
        }
        if old_id != self.moused_over_drawable {
            self.mouse_calls.push(UiMouseCall::ResetTooltipDelay);
        }
        if self.mouse_mode == MouseMode::Default
            && !self.is_scrolling
            && !self.is_selecting()
            && self.select_count() == 0
            && (!h.recorder_playback() || h.mouse_moved_recently())
        {
            match self.moused_over_drawable {
                Some(m) => {
                    // Add basic logic to determine if we can select a unit (or hint)
                    let draw = h.find_drawable(m).then_some(m);
                    let obj = draw.and_then(|d| h.drawable_object(d));
                    let mut draw_selectable = can_select_drawable(draw, false, h);
                    if obj.is_none() {
                        draw_selectable = false;
                    }
                    if draw_selectable && (plant(9089) || obj.is_some_and(|o| h.is_locally_controlled(o))) {
                        self.set_mouse_cursor(cursor::SELECTING);
                    } else {
                        self.set_mouse_cursor(cursor::ARROW);
                    }
                }
                None => self.set_mouse_cursor(cursor::ARROW),
            }
        } else if self.mouse_mode != MouseMode::Default && (self.mouse_mode != MouseMode::BuildPlace || plant(9090)) {
            self.set_mouse_cursor(self.mouse_mode_cursor);
        }
    }

    /// `createMouseoverHint`'s tooltip for the hovered drawable's object `obj`.
    fn mouseover_tooltip(&mut self, obj: ObjectId, h: &mut impl CursorHintHost) {
        let mut player = None;
        let contain = h.contain_info(obj);
        if let Some(c) = contain {
            player = c.apparent_controlling_player;
        }
        if player.is_none() {
            player = h.controlling_player(obj);
        }
        let mut template = h.object_template(obj);
        let mut disguised = false;
        if h.is_kind_of(obj, KINDOF_DISGUISER) {
            // Because we have support for disguised units pretending to be units from another team, we need to intercept
            // it here and make sure it's rendered appropriately based on which client is rendering it.
            if let Some(d) = h.disguise(obj) {
                // (upstream calls `player->getRelationship` here without a NULL test)
                if d.disguised && player.is_some_and(|p| !h.player_allied_to_local_default_team(p)) && h.local_player_active() {
                    // Neutrals and enemies will see this disguised unit as the team it's disguised as.
                    player = Some(d.player_index);
                    if let Some(t) = d.template {
                        template = t;
                        disguised = true;
                    }
                }
            }
        }
        let mut s = h.template_display_name(template);
        let display_name = s.clone();
        if s.is_empty() {
            let mut label = b"ThingTemplate:".to_vec();
            label.extend(h.template_name(h.object_template(obj)));
            s = h.fetch(&label);
        }
        // Add on dollar amount of warehouse contents so people don't freak out until the art is hooked up
        if let Some(boxes) = h.warehouse_boxes(obj) {
            let value = boxes.wrapping_mul(h.base_value_per_supply_box());
            let feedback = h.format_int(&h.fetch(b"TOOLTIP:SupplyWarehouse"), value);
            s = concat(&s, &feedback);
        }
        let Some(player) = player else { return };
        let tooltip = if h.recorder_multiplayer() && h.player_is_playable_side(player) {
            // `tooltip.format(L"%s\n%s", str.str(), player->getPlayerDisplayName().str())`
            concat(&concat(&s, &[u16::from(b'\n')]), &h.player_display_name(player))
        } else {
            s
        };
        if !h.object_cell_clear_for_local_player(obj) {
            return;
        }
        let color = if disguised {
            h.player_color(player)
        } else {
            let mut rgb = h.indicator_color(obj);
            // Unless this is a stealth garrisoned building, Let's not use the contained's housecolor
            if let Some(c) = contain {
                if c.garrisonable {
                    if let Some(p) = c.apparent_controlling_player {
                        rgb = h.player_color(p);
                    }
                }
            }
            rgb
        };
        // Object:Prop is a blank string... but we don't want to show any popup box at all if that is the case!
        if display_name != h.fetch(b"OBJECT:Prop") {
            self.mouse_calls.push(UiMouseCall::Tooltip { text: tooltip, color: Some(color) });
        }
    }

    /// `createCommandHint(msg)` for the hint `t`; see the module docs.
    pub fn create_command_hint(&mut self, t: MessageType, h: &mut impl CursorHintHost) {
        if self.is_scrolling || self.is_selecting() || h.recorder_playback() {
            return;
        }
        let draw = self.moused_over_drawable.filter(|&d| h.find_drawable(d));
        let mut t = t;
        if draw.is_some() && (t == MSG_DO_ATTACK_OBJECT_HINT || t == MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT) {
            let obj = draw.and_then(|d| h.drawable_object(d));
            let ss = obj.map_or(ObjectShroudStatus::Clear, |o| h.object_shroud_status_for_local_player(o));
            if ss == ObjectShroudStatus::Shrouded && !plant(9083) {
                t = MSG_DO_MOVETO_HINT; // if the object is hidden, switch to something innocuous
            }
        }
        self.mouse_calls.push(UiMouseCall::RADIUS_CURSOR_NONE);
        if h.double_click_attack_move() {
            let fire = if plant(9084) {
                let f = self.double_click_attack_move_guard_hint_timer > 0;
                self.double_click_attack_move_guard_hint_timer = self.double_click_attack_move_guard_hint_timer.wrapping_sub(1);
                f
            } else {
                self.double_click_attack_move_guard_hint_timer = self.double_click_attack_move_guard_hint_timer.wrapping_sub(1);
                self.double_click_attack_move_guard_hint_timer > 0
            };
            if fire {
                self.set_mouse_cursor(cursor::FORCE_ATTACK_GROUND);
                self.mouse_calls.push(UiMouseCall::RadiusCursor { kind: RADIUSCURSOR_GUARD_AREA, special_power: None, weapon_slot: PRIMARY_WEAPON });
                return;
            }
        }
        // set cursor to normal if there is a window under the cursor
        let (under_window, window) = window_walk(&h.windows_under_cursor());
        let under_window = under_window && !plant(9081);
        // Add basic logic to determine if we can select a unit (or hint)
        let obj = draw.and_then(|d| h.drawable_object(d));
        let mut draw_selectable = can_select_drawable(draw, false, h);
        if obj.is_none() {
            draw_selectable = false;
        }
        // Note: These are only non-NULL if there is exactly one thing selected.
        let src_obj = if self.select_count() == 1 { self.first_selected_drawable().and_then(|d| h.drawable_object(d)) } else { None };
        let src_local = src_obj.is_some_and(|o| h.is_locally_controlled(o));
        let obj_local = obj.is_some_and(|o| h.is_locally_controlled(o));
        match self.mouse_mode {
            MouseMode::Default => {
                // This section of code only gets called when there is no specific cursor mode happening.
                if under_window || (src_obj.is_some() && !src_local) {
                    self.set_mouse_cursor(cursor::ARROW);
                    return;
                }
                let c = match t {
                    MSG_DO_MOVETO_HINT => {
                        if !draw_selectable && src_local && src_obj.is_some_and(|o| h.is_kind_of(o, KINDOF_STRUCTURE)) {
                            Some(cursor::GENERIC_INVALID)
                        } else if draw_selectable && obj_local && !obj.is_some_and(|o| h.is_kind_of(o, KINDOF_MINE)) {
                            Some(cursor::SELECTING)
                        } else if window.is_some_and(|w| w.radar)
                            && (plant(9082) || !h.radar_forced())
                            && (h.radar_hidden() || !h.local_player_has_radar())
                        {
                            Some(cursor::ARROW)
                        } else {
                            Some(cursor::MOVETO)
                        }
                    }
                    MSG_DO_ATTACKMOVETO_HINT => Some(if draw_selectable && obj_local { cursor::SELECTING } else { cursor::ATTACKMOVETO }),
                    MSG_ADD_WAYPOINT_HINT => Some(cursor::WAYPOINT),
                    MSG_DO_ATTACK_OBJECT_HINT => Some(cursor::ATTACK_OBJECT),
                    MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT => Some(cursor::OUTRANGE),
                    MSG_DO_FORCE_ATTACK_OBJECT_HINT => Some(cursor::FORCE_ATTACK_OBJECT),
                    MSG_DO_FORCE_ATTACK_GROUND_HINT => Some(cursor::FORCE_ATTACK_GROUND),
                    MSG_GET_REPAIRED_HINT => Some(cursor::GET_REPAIRED),
                    MSG_DOCK_HINT => Some(cursor::DOCK),
                    MSG_GET_HEALED_HINT => Some(cursor::GET_HEALED),
                    MSG_DO_REPAIR_HINT => Some(cursor::DO_REPAIR),
                    MSG_RESUME_CONSTRUCTION_HINT => Some(cursor::RESUME_CONSTRUCTION),
                    MSG_ENTER_HINT => Some(cursor::ENTER_FRIENDLY),
                    MSG_CONVERT_TO_CARBOMB_HINT | MSG_HIJACK_HINT | MSG_SABOTAGE_HINT => Some(cursor::ENTER_AGGRESSIVELY),
                    MSG_DEFECTOR_HINT => Some(cursor::DEFECTOR),
                    // (`MSG_PICK_UP_PRISONER_HINT`: ALLOW_SURRENDER only, not in the release build)
                    MSG_CAPTUREBUILDING_HINT => Some(cursor::CAPTUREBUILDING),
                    MSG_HACK_HINT => Some(cursor::HACK),
                    MSG_IMPOSSIBLE_ATTACK_HINT => Some(cursor::GENERIC_INVALID),
                    MSG_SET_RALLY_POINT_HINT => Some(if !draw_selectable { cursor::SET_RALLY_POINT } else { cursor::SELECTING }),
                    MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT => Some(cursor::PARTICLE_UPLINK_CANNON),
                    MSG_DO_SALVAGE_HINT => Some(cursor::MOVETO),
                    MSG_DO_INVALID_HINT => Some(cursor::GENERIC_INVALID),
                    _ => None,
                };
                if let Some(c) = c {
                    self.set_mouse_cursor(c);
                }
            }
            MouseMode::BuildPlace => {
                if under_window {
                    self.set_mouse_cursor(cursor::ARROW);
                    return;
                }
                // (upstream's third case is `MSG_ADD_WAYPOINT`, not the hint: a quirk kept)
                let waypoint = if plant(9085) { MSG_ADD_WAYPOINT_HINT } else { MSG_ADD_WAYPOINT };
                if t == MSG_DO_MOVETO_HINT || t == MSG_DO_ATTACKMOVETO_HINT || t == waypoint {
                    self.set_mouse_cursor(cursor::BUILD_PLACEMENT);
                } else if t == MSG_DO_ATTACK_OBJECT_HINT || t == MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT {
                    self.set_mouse_cursor(cursor::INVALID_BUILD_PLACEMENT);
                }
            }
            MouseMode::GuiCommand => {
                if under_window {
                    self.set_mouse_cursor(cursor::ARROW);
                    return;
                }
                // set the mouse cursor for commands that need a targeting or to normal with no command
                let Some(cmd) = self.gui_command().cloned() else { return };
                if cmd.is_context_command() || cmd.command_type == gui::SPECIAL_POWER || cmd.command_type == gui::SPECIAL_POWER_FROM_SHORTCUT {
                    // Here is the hook for when we are in a context sensitive command mode. We can either do the
                    // specified command mode command or nothing! Whether or not the command is valid or not was
                    // determined in evaluateContextCommand which is called first, and posts the appropriate message.
                    let valid = if plant(9086) { MSG_INVALID_GUICOMMAND_HINT } else { MSG_VALID_GUICOMMAND_HINT };
                    let name = if t == valid { &cmd.cursor_name } else { &cmd.invalid_cursor_name };
                    let index = get_cursor_index(name);
                    if index != cursor::INVALID_MOUSE_CURSOR {
                        self.set_mouse_cursor(index);
                    } else {
                        self.set_mouse_cursor(if plant(9087) { cursor::ARROW } else { cursor::CROSS });
                    }
                    self.mouse_calls.push(UiMouseCall::radius_cursor_of(&cmd));
                } else if cmd.options & opt::NEED_TARGET != 0 {
                    let index = get_cursor_index(&cmd.cursor_name);
                    if index != cursor::INVALID_MOUSE_CURSOR {
                        self.set_mouse_cursor(index);
                    } else {
                        self.set_mouse_cursor(cursor::CROSS);
                    }
                    self.mouse_calls.push(UiMouseCall::radius_cursor_of(&cmd));
                } else {
                    self.mouse_calls.push(UiMouseCall::RADIUS_CURSOR_NONE);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::selection_info::SelectionInfoHost;
    use crate::common::game_common::Relationship;
    use crate::common::kind_of::KindOfType;
    use std::collections::BTreeSet;

    /// A small world: drawable `d`'s object is `d`; `mine` are the local player's (player 0).
    #[derive(Default)]
    struct World {
        objects: BTreeSet<ObjectId>,
        mine: BTreeSet<ObjectId>,
        kinds: Vec<(ObjectId, KindOfType)>,
        windows: Vec<UiWindow>,
        shrouded: BTreeSet<ObjectId>,
        dcam: bool,
        radar_off: bool,
    }

    impl SelectionInfoHost for World {
        fn drawable_object(&self, d: DrawableId) -> Option<ObjectId> {
            self.objects.contains(&d).then_some(d)
        }
        fn is_locally_controlled(&self, o: ObjectId) -> bool {
            self.mine.contains(&o)
        }
        fn is_kind_of(&self, o: ObjectId, k: KindOfType) -> bool {
            self.kinds.contains(&(o, k)) || k == crate::common::kind_of::KINDOF_SELECTABLE
        }
        fn relationship_to_local_player(&self, _o: ObjectId) -> Relationship {
            Relationship::Enemies
        }
        fn can_local_player_garrison(&mut self, _o: ObjectId) -> bool {
            false
        }
        fn force_fire_or_move(&self) -> bool {
            false
        }
        fn prefer_selection(&self) -> bool {
            false
        }
        fn use_alternate_mouse(&self) -> bool {
            false
        }
    }
    impl CanSelectHost for World {
        fn is_effectively_dead(&self, _o: ObjectId) -> bool {
            false
        }
        fn is_drawable_effectively_hidden(&self, _d: DrawableId) -> bool {
            false
        }
        fn drawable_is_kind_of(&self, d: DrawableId, k: KindOfType) -> bool {
            self.is_kind_of(d, k)
        }
        fn windows_at_drawable(&self, _d: DrawableId) -> Vec<UiWindow> {
            Vec::new()
        }
        fn unselectable_or_masked(&self, _o: ObjectId) -> bool {
            false
        }
        fn object_is_selectable(&self, _o: ObjectId) -> bool {
            true
        }
    }
    impl CursorHintHost for World {
        fn find_drawable(&self, d: DrawableId) -> bool {
            self.objects.contains(&d)
        }
        fn windows_under_cursor(&self) -> Vec<UiWindow> {
            self.windows.clone()
        }
        fn recorder_playback(&self) -> bool {
            false
        }
        fn mouse_moved_recently(&mut self) -> bool {
            true
        }
        fn double_click_attack_move(&self) -> bool {
            self.dcam
        }
        fn radar_forced(&self) -> bool {
            false
        }
        fn radar_hidden(&self) -> bool {
            self.radar_off
        }
        fn local_player_has_radar(&self) -> bool {
            true
        }
        fn object_shroud_status_for_local_player(&self, o: ObjectId) -> ObjectShroudStatus {
            if self.shrouded.contains(&o) { ObjectShroudStatus::Shrouded } else { ObjectShroudStatus::Clear }
        }
        fn mob_member_slaver(&self, _o: ObjectId) -> Option<ObjectId> {
            None
        }
        fn object_drawable_of(&self, o: ObjectId) -> Option<DrawableId> {
            self.objects.contains(&o).then_some(o)
        }
        fn contain_info(&mut self, _o: ObjectId) -> Option<ContainInfo> {
            None
        }
        fn controlling_player(&self, o: ObjectId) -> Option<i32> {
            Some(if self.mine.contains(&o) { 0 } else { 1 })
        }
        fn disguise(&self, _o: ObjectId) -> Option<DisguiseInfo> {
            None
        }
        fn player_allied_to_local_default_team(&self, p: i32) -> bool {
            p == 0
        }
        fn local_player_active(&self) -> bool {
            true
        }
        fn object_template(&self, o: ObjectId) -> u16 {
            o as u16
        }
        fn template_display_name(&self, _t: u16) -> Vec<u16> {
            "Tank".encode_utf16().collect()
        }
        fn template_name(&self, _t: u16) -> Vec<u8> {
            b"Tank".to_vec()
        }
        fn fetch(&self, label: &[u8]) -> Vec<u16> {
            label.iter().map(|&b| u16::from(b)).collect()
        }
        fn warehouse_boxes(&mut self, _o: ObjectId) -> Option<i32> {
            None
        }
        fn base_value_per_supply_box(&self) -> i32 {
            100
        }
        fn format_int(&self, f: &[u16], v: i32) -> Vec<u16> {
            concat(f, &v.to_string().encode_utf16().collect::<Vec<_>>())
        }
        fn recorder_multiplayer(&self) -> bool {
            false
        }
        fn player_is_playable_side(&self, _p: i32) -> bool {
            true
        }
        fn player_display_name(&self, _p: i32) -> Vec<u16> {
            Vec::new()
        }
        fn object_cell_clear_for_local_player(&self, _o: ObjectId) -> bool {
            true
        }
        fn player_color(&self, p: i32) -> i32 {
            p + 10
        }
        fn indicator_color(&self, o: ObjectId) -> i32 {
            o as i32 + 100
        }
    }

    fn cursors(ui: &mut InGameUiSelection) -> Vec<i32> {
        ui.take_mouse_calls().into_iter().filter_map(|c| if let UiMouseCall::SetCursor(c) = c { Some(c) } else { None }).collect()
    }

    /// #122, predicted before the run: over the ground with nothing selected the mouse-over hint sets the arrow; over
    /// the local player's tank `SELECTING` (and a tooltip "Tank" in the indicator colour 105); with an enemy (7)
    /// moused over and the tank selected, the attack hint shows `ATTACK_OBJECT`, but `MOVETO` when 7 is shrouded; an
    /// opaque window makes it the arrow; the radar window with the radar hidden makes the move hint the arrow; with
    /// double-click attack move on and the guard timer triggered the next ten hints are `FORCE_ATTACK_GROUND` and the
    /// eleventh is the hint's own; a GUI command whose cursor name is "Target" shows `CROSS` (4) for the valid hint and,
    /// with no invalid cursor name, `CROSS` too; placing a building, the move hint is `BUILD_PLACEMENT` and the waypoint
    /// hint leaves the cursor alone.
    ///
    /// Results (2026-11-11): all held.
    #[test]
    fn the_hints_set_the_cursors_upstream_sets() {
        let mut w = World { objects: [5, 7].into(), mine: [5].into(), ..Default::default() };
        let mut ui = InGameUiSelection::new();
        ui.create_mouseover_hint(&MouseoverHint::Location(Coord3D::default()), &mut w);
        assert_eq!(cursors(&mut ui), [cursor::ARROW]);
        ui.create_mouseover_hint(&MouseoverHint::Drawable(5), &mut w);
        let calls = ui.take_mouse_calls();
        assert!(calls.contains(&UiMouseCall::Tooltip { text: "Tank".encode_utf16().collect(), color: Some(105) }), "{calls:?}");
        assert_eq!(calls.last(), Some(&UiMouseCall::SetCursor(cursor::SELECTING)));
        struct NoUi;
        impl crate::client::in_game_ui::SelectionUiHost for NoUi {
            fn logic_frame(&self) -> u32 {
                0
            }
            fn append_message_bool(&mut self, _t: MessageType, _b: bool) {}
            fn on_selection_changed(&mut self, _d: DrawableId, _s: bool) {}
        }
        ui.select_drawable(5, &mut NoUi);
        ui.create_mouseover_hint(&MouseoverHint::Drawable(7), &mut w);
        assert_eq!(cursors(&mut ui), Vec::<i32>::new(), "with a selection the mouse-over leaves the cursor");
        ui.create_command_hint(MSG_DO_ATTACK_OBJECT_HINT, &mut w);
        assert_eq!(cursors(&mut ui), [cursor::ATTACK_OBJECT]);
        w.shrouded.insert(7);
        ui.create_command_hint(MSG_DO_ATTACK_OBJECT_HINT, &mut w);
        assert_eq!(cursors(&mut ui), [cursor::MOVETO]);
        w.windows = vec![UiWindow { see_thru: false, ..Default::default() }];
        ui.create_command_hint(MSG_DO_ATTACK_OBJECT_HINT, &mut w);
        assert_eq!(cursors(&mut ui), [cursor::ARROW]);
        w.windows = vec![UiWindow { left_hud_input: true, radar: true, see_thru: false }];
        w.radar_off = true;
        ui.create_mouseover_hint(&MouseoverHint::Location(Coord3D::default()), &mut w);
        ui.create_command_hint(MSG_DO_MOVETO_HINT, &mut w);
        assert_eq!(cursors(&mut ui), [cursor::ARROW]);
        w.windows.clear();
        w.dcam = true;
        ui.trigger_double_click_attack_move_guard_hint();
        let mut seen = Vec::new();
        for _ in 0..11 {
            ui.create_command_hint(MSG_DO_MOVETO_HINT, &mut w);
            seen.extend(cursors(&mut ui));
        }
        assert_eq!(seen[..10], [cursor::FORCE_ATTACK_GROUND; 10]);
        assert_eq!(seen[10], cursor::MOVETO);
        w.dcam = false;
        let target = GuiButton { command_type: gui::SPECIAL_POWER, options: opt::NEED_TARGET_POS, cursor_name: b"Target".to_vec(), ..Default::default() };
        ui.set_gui_command(Some(target));
        assert_eq!((ui.mouse_mode(), ui.mouse_mode_cursor()), (MouseMode::GuiCommand, cursor::ARROW));
        cursors(&mut ui);
        ui.create_command_hint(MSG_VALID_GUICOMMAND_HINT, &mut w);
        ui.create_command_hint(MSG_INVALID_GUICOMMAND_HINT, &mut w);
        assert_eq!(cursors(&mut ui), [cursor::CROSS, cursor::CROSS]);
        assert_eq!(ui.mouse_mode_cursor(), cursor::CROSS);
        ui.set_gui_command(None);
        ui.place_build_available(Some(3), Some(5), 0.0, &w);
        assert_eq!((ui.mouse_mode(), ui.mouse_mode_cursor()), (MouseMode::BuildPlace, cursor::CROSS));
        cursors(&mut ui);
        ui.create_command_hint(MSG_DO_MOVETO_HINT, &mut w);
        ui.create_command_hint(MSG_ADD_WAYPOINT_HINT, &mut w);
        assert_eq!(cursors(&mut ui), [cursor::BUILD_PLACEMENT]);
        ui.place_build_available(None, None, 0.0, &w);
        assert_eq!((ui.mouse_mode(), ui.mouse_mode_cursor()), (MouseMode::Default, cursor::ARROW));
    }

    /// #122, predicted before the run: the old translation's defect is gone: starting a placement while one from a
    /// builder is pending runs upstream's `placeBuildAvailable(NULL, NULL)` first, so the prevent-deselection flag ends
    /// FALSE (the old code left it TRUE), the mouse is released and captured again, and the mode is the placement's.
    ///
    /// Results (2026-11-11): held.
    #[test]
    fn a_placement_over_a_placement_ends_the_first() {
        let w = World { objects: [5].into(), mine: [5].into(), ..Default::default() };
        let mut ui = InGameUiSelection::new();
        ui.place_build_available(Some(3), Some(5), 0.0, &w);
        assert!(!ui.prevent_left_click_deselection_once());
        ui.take_mouse_calls();
        ui.place_build_available(Some(4), None, 0.0, &w);
        assert!(!ui.prevent_left_click_deselection_once(), "the recursion cleared the source first");
        assert_eq!(
            ui.take_mouse_calls(),
            [
                UiMouseCall::RADIUS_CURSOR_NONE,
                UiMouseCall::ReleaseCapture,
                UiMouseCall::SetCursor(cursor::ARROW),
                UiMouseCall::Capture,
                UiMouseCall::SetCursor(cursor::CROSS)
            ]
        );
        assert_eq!(ui.mouse_mode(), MouseMode::BuildPlace);
    }
}
