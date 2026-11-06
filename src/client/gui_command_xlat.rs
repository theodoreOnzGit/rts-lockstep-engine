// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/GUICommandTranslator.cpp (validUnderCursor,
//     doFireWeaponCommand, doGuardCommand, doAttackMoveCommand, doSetRallyPointCommand, doPlaceBeacon,
//     GUICommandTranslator::translateGameMessage)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (CommandButton::isValidObjectTarget(Player*,
//     Object*))
//   GeneralsMD/Code/GameEngine/Source/GameClient/SelectionInfo.cpp (translatePickTypesToKindof)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The GUI command translator (GitHub #54): when a control bar button that is **not** a context command is waiting for
//! its target (guard, guard without pursuit, guard flying units only, attack move, set rally point, place beacon,
//! evacuate to a position, fire a weapon), the next left click in the world carries it out, as the game's own command
//! message, and the GUI command ends. Upstream attaches it to the message stream at priority 40, after placement (30) and
//! before selection (50) and the command translator (70) (`GameClient.cpp`), so a click it takes never selects.
//!
//! **What the view supplies.** Upstream's `TheTacticalView->screenToTerrain(mouse)` is the caller's `ground` (the terrain
//! under the release point, `pixelRegion.hi`), and `pickDrawable(mouse, FALSE, pickType)` is the caller's `picked` (the
//! drawable under the pointer), which [`valid_under_cursor`] filters by the kinds `translatePickTypesToKindof` maps the
//! pick type to (`W3DView::pickDrawable` casts its ray with those collision types).
//!
//! **Not here:** the radar's clicks are `control_bar_callback`. The cursor of a pending command is the view's.

use crate::client::command_xlat::{CmdArg, CommandXlatHost, GuiButton};
use crate::client::control_bar::{gui, opt};
use crate::client::in_game_ui::{DrawableId, InGameUiSelection};
use crate::client::selection_xlat::Disposition;
use crate::client::unit_voice::{pick_and_play_unit_voice_response, PickAndPlayInfo};
use crate::common::base_type::Coord3D;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{KINDOF_MINE, KINDOF_SELECTABLE, KINDOF_SHRUBBERY};
use crate::common::message_stream::MessageType;
use crate::common::message_types::*;
use crate::logic::ai::ai_update_types::{GuardMode, GUARDMODE_GUARD_FLYING_UNITS_ONLY, GUARDMODE_GUARD_WITHOUT_PURSUIT, GUARDMODE_NORMAL};
use crate::logic::world::object_command_button::is_valid_relationship_target;

/// `enum CommandStatus` (the file's private enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandStatus {
    /// `COMMAND_INCOMPLETE` (no function of the file returns it in this release).
    Incomplete,
    /// `COMMAND_COMPLETE`.
    Complete,
}

/// The kinds of `PickType` the file asks for: `PICK_TYPE_SELECTABLE`, with `PICK_TYPE_SHRUBBERY` and `PICK_TYPE_MINES`
/// when the command allows them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickType {
    pub shrubbery: bool,
    pub mines: bool,
}

/// `pickAndPlayUnitVoiceResponse(TheInGameUI->getAllSelectedDrawables(), msg, info)`.
fn voice(msg: MessageType, info: Option<&PickAndPlayInfo>, ui: &InGameUiSelection, h: &mut impl CommandXlatHost) {
    let list = ui.all_selected_drawables().to_vec();
    pick_and_play_unit_voice_response(&list, msg, info, ui, h);
}

/// `validUnderCursor(mouse, command, pickType)`: the object of the drawable picked under the cursor (`picked`), if the pick
/// type admits it (`translatePickTypesToKindof`: `SELECTABLE`, and `SHRUBBERY` / `MINE` when asked for) and the command
/// may target it (`command->isValidObjectTarget(ThePlayerList->getLocalPlayer(), pickObj)`: the local player's
/// relationship to the object's team).
pub fn valid_under_cursor(picked: Option<DrawableId>, command: &GuiButton, pick_type: PickType, h: &impl CommandXlatHost) -> Option<ObjectId> {
    let obj = picked.and_then(|d| h.drawable_object(d))?;
    let admitted = h.is_kind_of(obj, KINDOF_SELECTABLE)
        || (pick_type.shrubbery && h.is_kind_of(obj, KINDOF_SHRUBBERY))
        || (pick_type.mines && h.is_kind_of(obj, KINDOF_MINE));
    if !admitted {
        return None;
    }
    // `isValidObjectTarget(Player*, Object*)`: `sourcePlayer->getRelationship(targetObj->getTeam())`
    is_valid_relationship_target(command.options, h.relationship_to_local_player(obj)).then_some(obj)
}

/// `doFireWeaponCommand(command, mouse)`: a weapon at the ground (`NEED_TARGET_POS`, with the object under the cursor for
/// weapons that dislike obstacles), at a valid object under the cursor, or with no target.
pub fn do_fire_weapon_command(command: &GuiButton, ground: &Coord3D, picked: Option<DrawableId>, ui: &InGameUiSelection, h: &mut impl CommandXlatHost) -> CommandStatus {
    // "for single object selections get the source ID and sanity check for illegal object and bail along the way" (the id
    // itself is not used further)
    if ui.select_count() == 1 && ui.first_selected_drawable().and_then(|d| h.drawable_object(d)).is_none() {
        return CommandStatus::Complete;
    }
    if command.options & opt::NEED_TARGET_POS != 0 {
        // "Also append the object ID (incase weapon doesn't like obstacles on land)."
        let target = valid_under_cursor(picked, command, PickType::default(), h).unwrap_or(INVALID_ID);
        let args = [CmdArg::Int(command.weapon_slot), CmdArg::Location(*ground), CmdArg::Int(command.max_shots_to_fire), CmdArg::Object(target)];
        h.append_command(MSG_DO_WEAPON_AT_LOCATION, &args);
    } else if command.options & opt::NEED_OBJECT_TARGET != 0 {
        // "some commands allow us to target shrubbery"
        let pick_type = PickType {
            shrubbery: command.options & opt::ALLOW_SHRUBBERY_TARGET != 0,
            mines: command.options & opt::ALLOW_MINE_TARGET != 0,
        };
        if let Some(target) = valid_under_cursor(picked, command, pick_type, h) {
            h.append_command(MSG_DO_WEAPON_AT_OBJECT, &[CmdArg::Int(command.weapon_slot), CmdArg::Object(target), CmdArg::Int(command.max_shots_to_fire)]);
        }
    } else {
        // "This could be legit now -- think of firing a self destruct weapon"
        h.append_command(MSG_DO_WEAPON, &[CmdArg::Int(command.weapon_slot), CmdArg::Int(command.max_shots_to_fire)]);
    }
    CommandStatus::Complete
}

/// `doGuardCommand(command, guardMode, mouse)`: guard a valid object under the cursor (`COMMAND_OPTION_NEED_OBJECT_TARGET`),
/// else the ground (`NEED_TARGET_POS`), else the first selected object's own position.
pub fn do_guard_command(command: &GuiButton, guard_mode: GuardMode, ground: &Coord3D, picked: Option<DrawableId>, ui: &InGameUiSelection, h: &mut impl CommandXlatHost) -> CommandStatus {
    if ui.select_count() == 0 {
        return CommandStatus::Complete;
    }
    if command.options & opt::NEED_OBJECT_TARGET != 0 {
        if let Some(target) = valid_under_cursor(picked, command, PickType::default(), h) {
            h.append_command(MSG_DO_GUARD_OBJECT, &[CmdArg::Object(target), CmdArg::Int(guard_mode)]);
            voice(MSG_DO_GUARD_OBJECT, None, ui, h);
            return CommandStatus::Complete;
        }
    }
    let world = if command.options & opt::NEED_TARGET_POS != 0 {
        *ground
    } else {
        let Some(obj) = ui.first_selected_drawable().and_then(|d| h.drawable_object(d)) else { return CommandStatus::Complete };
        h.object_position(obj)
    };
    h.append_command(MSG_DO_GUARD_POSITION, &[CmdArg::Location(world), CmdArg::Int(guard_mode)]);
    voice(MSG_DO_GUARD_POSITION, None, ui, h);
    CommandStatus::Complete
}

/// `doAttackMoveCommand(command, mouse)`: the selection attack-moves to the ground.
pub fn do_attack_move_command(_command: &GuiButton, ground: &Coord3D, ui: &InGameUiSelection, h: &mut impl CommandXlatHost) -> CommandStatus {
    // (upstream asserts a selected object, then checks it)
    if ui.first_selected_drawable().and_then(|d| h.drawable_object(d)).is_none() {
        return CommandStatus::Complete;
    }
    h.append_command(MSG_DO_ATTACKMOVETO, &[CmdArg::Location(*ground)]);
    // "Play the unit voice response"
    voice(MSG_DO_ATTACKMOVETO, None, ui, h);
    CommandStatus::Complete
}

/// `doSetRallyPointCommand(command, mouse)`: the first selected object (a structure, never multiply selected) gets its
/// rally point at the ground.
pub fn do_set_rally_point_command(_command: &GuiButton, ground: &Coord3D, ui: &InGameUiSelection, h: &mut impl CommandXlatHost) -> CommandStatus {
    // (upstream asserts exactly one selected object; release builds go on with the first)
    let Some(obj) = ui.first_selected_drawable().and_then(|d| h.drawable_object(d)) else { return CommandStatus::Complete };
    h.append_command(MSG_SET_RALLY_POINT, &[CmdArg::Object(obj), CmdArg::Location(*ground)]);
    CommandStatus::Complete
}

/// `doPlaceBeacon(command, mouse)`: a beacon at the ground.
pub fn do_place_beacon(_command: &GuiButton, ground: &Coord3D, h: &mut impl CommandXlatHost) -> CommandStatus {
    h.append_command(MSG_PLACE_BEACON, &[CmdArg::Location(*ground)]);
    CommandStatus::Complete
}

/// `class GUICommandTranslator` (it has no members).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuiCommandTranslator;

impl GuiCommandTranslator {
    /// `translateGameMessage`'s `MSG_RAW_MOUSE_LEFT_BUTTON_DOWN`: with any GUI command pending the press is used up, so no
    /// selection rectangle starts ("we want to stay totally within the gui command mode").
    pub fn left_button_down(&self, ui: &mut InGameUiSelection) -> Disposition {
        if ui.gui_command().is_none() {
            return Disposition::Keep;
        }
        // "If we're destroying the message, it means we used it. Therefore, destroy the current attack move instruction
        // as well."
        ui.clear_attack_move_to_mode();
        Disposition::Destroy
    }

    /// `translateGameMessage`'s `MSG_MOUSE_LEFT_CLICK` / `MSG_MOUSE_LEFT_DOUBLE_CLICK`, released over `ground` with
    /// `picked` under the cursor: a pending command that is not a context command is carried out, and the click is used
    /// up (special powers are left to the command translator's context evaluation).
    pub fn left_click(&self, ground: &Coord3D, picked: Option<DrawableId>, ui: &mut InGameUiSelection, h: &mut impl CommandXlatHost) -> Disposition {
        // "only pay attention to clicks in this translator if there is a pending GUI command"
        let Some(command) = ui.gui_command().cloned() else { return Disposition::Keep };
        if command.is_context_command() {
            return Disposition::Keep;
        }
        let mut status = CommandStatus::Complete;
        match command.command_type {
            gui::FIRE_WEAPON => {
                status = do_fire_weapon_command(&command, ground, picked, ui, h);
                let info = PickAndPlayInfo { weapon_slot: Some(command.weapon_slot), ..Default::default() };
                voice(MSG_DO_WEAPON_AT_LOCATION, Some(&info), ui, h);
            }
            gui::EVACUATE => {
                if command.options & opt::NEED_TARGET_POS != 0 {
                    h.append_command(MSG_EVACUATE, &[CmdArg::Location(*ground)]);
                    voice(MSG_EVACUATE, None, ui, h);
                    status = CommandStatus::Complete;
                }
            }
            gui::GUARD => status = do_guard_command(&command, GUARDMODE_NORMAL, ground, picked, ui, h),
            gui::GUARD_WITHOUT_PURSUIT => status = do_guard_command(&command, GUARDMODE_GUARD_WITHOUT_PURSUIT, ground, picked, ui, h),
            gui::GUARD_FLYING_UNITS_ONLY => status = do_guard_command(&command, GUARDMODE_GUARD_FLYING_UNITS_ONLY, ground, picked, ui, h),
            // "Special weapons are now always context commands..." (returned before the attack move mode is cleared)
            gui::SPECIAL_POWER | gui::SPECIAL_POWER_FROM_SHORTCUT => return Disposition::Keep,
            gui::ATTACK_MOVE => status = do_attack_move_command(&command, ground, ui, h),
            gui::SET_RALLY_POINT => status = do_set_rally_point_command(&command, ground, ui, h),
            gui::PLACE_BEACON => status = do_place_beacon(&command, ground, h),
            // (no case: the click is still used and the command ends)
            _ => {}
        }
        // "get out of GUI command mode if we completed the command one way or another"
        if status == CommandStatus::Complete {
            ui.set_prevent_left_click_deselection_once(true);
            ui.set_gui_command(None);
        }
        ui.clear_attack_move_to_mode();
        Disposition::Destroy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::selection_xlat::tests::Host;
    use crate::common::game_common::Relationship;

    fn button(command_type: i32, options: u32) -> GuiButton {
        GuiButton { index: 1, command_type, options, ..Default::default() }
    }

    /// #54: each case of `translateGameMessage`'s click, on the selection test host (my unit 1 selected, an enemy 9 that
    /// is `SELECTABLE`, an ally 2 that is not, the ground at x = 50; the host places object `n` at x = `n`).
    ///
    /// *Predictions, written before the first run* (`issue54_predictions.md`): no GUI command: the press and the click are
    /// kept and nothing is sent. Guard (`NEED_TARGET_POS`) on the ground: `MSG_DO_GUARD_POSITION(ground, 0)`, the click
    /// destroyed, the command cleared and the deselection guard set. Guard without pursuit that may target an enemy, on
    /// 9: `MSG_DO_GUARD_OBJECT(9, 1)`; on 2 (not selectable): the position, mode 1. Guard flying units only with no
    /// position option: unit 1's own position, mode 2. Attack move: `MSG_DO_ATTACKMOVETO(ground)`. Rally point:
    /// `MSG_SET_RALLY_POINT(1, ground)`. Beacon: `MSG_PLACE_BEACON(ground)`. Evacuate to a position:
    /// `MSG_EVACUATE(ground)`. A weapon at a position on 9: `MSG_DO_WEAPON_AT_LOCATION(1, ground, 3, 9)`. A non-context
    /// special power: kept, still pending. A context command: the press destroyed, the click kept. Waypoints (no case):
    /// destroyed, nothing sent, the command ended.
    ///
    /// *Results (2026-11-06):* as predicted, first run.
    #[test]
    fn a_pending_gui_command_is_carried_out_by_the_click() {
        let mut h = Host { mine: vec![1, 2], ..Default::default() };
        h.rel.insert(9, Relationship::Enemies);
        h.selectable = vec![1, 9];
        let mut ui = InGameUiSelection::new();
        ui.select_drawable(1, &mut h);
        let x = GuiCommandTranslator;
        let ground = Coord3D { x: 50.0, y: 0.0, z: 0.0 };
        let click = |c: Option<GuiButton>, picked: Option<ObjectId>, ui: &mut InGameUiSelection, h: &mut Host| {
            ui.set_gui_command(c);
            let d = x.left_click(&ground, picked, ui, h);
            (d, h.cmd.commands.pop())
        };

        assert_eq!(x.left_button_down(&mut ui), Disposition::Keep);
        assert_eq!(click(None, None, &mut ui, &mut h), (Disposition::Keep, None));

        let guard = button(gui::GUARD, opt::NEED_TARGET_POS);
        assert_eq!(click(Some(guard), None, &mut ui, &mut h), (Disposition::Destroy, Some((MSG_DO_GUARD_POSITION, vec![CmdArg::Location(ground), CmdArg::Int(0)]))));
        assert_eq!(ui.gui_command(), None);
        assert!(ui.prevent_left_click_deselection_once());

        let enemy_guard = button(gui::GUARD_WITHOUT_PURSUIT, opt::NEED_TARGET_POS | opt::NEED_TARGET_ENEMY_OBJECT);
        assert_eq!(click(Some(enemy_guard.clone()), Some(9), &mut ui, &mut h).1, Some((MSG_DO_GUARD_OBJECT, vec![CmdArg::Object(9), CmdArg::Int(1)])));
        assert_eq!(click(Some(enemy_guard), Some(2), &mut ui, &mut h).1, Some((MSG_DO_GUARD_POSITION, vec![CmdArg::Location(ground), CmdArg::Int(1)])));

        let own = button(gui::GUARD_FLYING_UNITS_ONLY, opt::NEED_TARGET_ENEMY_OBJECT);
        let at_unit = Coord3D { x: 1.0, y: 0.0, z: 0.0 };
        assert_eq!(click(Some(own), None, &mut ui, &mut h).1, Some((MSG_DO_GUARD_POSITION, vec![CmdArg::Location(at_unit), CmdArg::Int(2)])));

        let pos = opt::NEED_TARGET_POS;
        assert_eq!(click(Some(button(gui::ATTACK_MOVE, pos)), None, &mut ui, &mut h).1, Some((MSG_DO_ATTACKMOVETO, vec![CmdArg::Location(ground)])));
        assert_eq!(click(Some(button(gui::SET_RALLY_POINT, pos)), None, &mut ui, &mut h).1, Some((MSG_SET_RALLY_POINT, vec![CmdArg::Object(1), CmdArg::Location(ground)])));
        assert_eq!(click(Some(button(gui::PLACE_BEACON, pos)), None, &mut ui, &mut h).1, Some((MSG_PLACE_BEACON, vec![CmdArg::Location(ground)])));
        assert_eq!(click(Some(button(gui::EVACUATE, pos)), None, &mut ui, &mut h).1, Some((MSG_EVACUATE, vec![CmdArg::Location(ground)])));
        let weapon = GuiButton { weapon_slot: 1, max_shots_to_fire: 3, ..button(gui::FIRE_WEAPON, pos | opt::NEED_TARGET_ENEMY_OBJECT) };
        let args = vec![CmdArg::Int(1), CmdArg::Location(ground), CmdArg::Int(3), CmdArg::Object(9)];
        assert_eq!(click(Some(weapon), Some(9), &mut ui, &mut h).1, Some((MSG_DO_WEAPON_AT_LOCATION, args)));

        let power = GuiButton { special_power: Some((7, 1)), ..button(gui::SPECIAL_POWER, pos) };
        assert_eq!(click(Some(power.clone()), None, &mut ui, &mut h), (Disposition::Keep, None));
        assert_eq!(ui.gui_command(), Some(&power));

        let context = button(gui::FIRE_WEAPON, pos | opt::CONTEXTMODE_COMMAND);
        ui.set_gui_command(Some(context.clone()));
        assert_eq!(x.left_button_down(&mut ui), Disposition::Destroy);
        assert_eq!(click(Some(context.clone()), None, &mut ui, &mut h), (Disposition::Keep, None));
        assert_eq!(ui.gui_command(), Some(&context));

        assert_eq!(click(Some(button(gui::WAYPOINTS, pos)), None, &mut ui, &mut h), (Disposition::Destroy, None));
        assert_eq!(ui.gui_command(), None);
        assert!(h.cmd.commands.is_empty());
    }
}
