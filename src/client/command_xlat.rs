// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (CommandTranslator: issueMoveToLocationCommand,
//     createAttackMessage, issueAttackCommand, issueSpecialPowerCommand, createEnterMessage, evaluateForceAttack,
//     evaluateContextCommand, canSelectionSalvage, canAnyForceAttack, and translateGameMessage's selected-group, mouse-over hint,
//     left and right click cases)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (areSelectedObjectsControllable, getCanSelectedObjectsAttack,
//     canSelectedObjectsDoAction, canSelectedObjectsOverrideSpecialPowerDestination)
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (CommandButton::isValidObjectTarget)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The command translator (C2b, GitHub #6): what a click on the ground or on a drawable orders the selected units to do: move,
//! attack, enter, dock, repair, get repaired or healed, resume construction, hijack, sabotage, salvage, capture and hack (through
//! the selected unit's command set), set a rally point, force attack; and the hints a mouse-over makes.
//!
//! **What is not here yet:**
//! - **A pending GUI command** (a control bar button that put the cursor in a mode: special powers, fire weapon, combat drop,
//!   the car bomb, hijack and sabotage modes, and construction specials) is C2c. Until then [`CommandTranslator::evaluate_context_command`]
//!   runs as upstream does when `TheInGameUI->getGUICommand()` is `NULL`.
//! - **Unit voices** (`pickAndPlayUnitVoiceResponse`) and the stats collector are audio and debug; they change no message.
//! - **The double-click guard order** (`m_doubleClickAttackMove`, off in the shipped options) and the right-button drag
//!   bookkeeping that cancels build placement (C2c, with placement).

use crate::client::in_game_ui::{DrawableId, InGameUiSelection};
use crate::client::selection_groups::SelectionGroupsHost;
use crate::common::base_type::Coord3D;
use crate::common::game_common::Relationship;
use crate::common::game_type::{ObjectId, INVALID_ANGLE, INVALID_ID};
use crate::common::kind_of::{
    KINDOF_AUTO_RALLYPOINT, KINDOF_FORCEATTACKABLE, KINDOF_MINE, KINDOF_SALVAGER, KINDOF_SHRUBBERY,
};
use crate::common::message_stream::MessageType;
use crate::common::message_types::*;
use crate::common::special_power_type::{
    SpecialPowerType, SPECIAL_BLACKLOTUS_CAPTURE_BUILDING, SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK,
    SPECIAL_BLACKLOTUS_STEAL_CASH_HACK, SPECIAL_HACKER_DISABLE_BUILDING, SPECIAL_INFANTRY_CAPTURE_BUILDING, SPECIAL_INVALID,
};
use crate::logic::ai::ai_update_types::{
    CanAttackResultValue, ATTACKRESULT_INVALID_SHOT, ATTACKRESULT_NOT_POSSIBLE, ATTACKRESULT_POSSIBLE,
    ATTACKRESULT_POSSIBLE_AFTER_MOVING,
};
use crate::logic::world::object_command_button::{is_valid_relationship_target, opt};

/// `enum CommandEvaluateType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandEvaluateType {
    /// `DO_COMMAND`: make the command's messages.
    DoCommand,
    /// `DO_HINT`: make the hint the cursor shows.
    DoHint,
    /// `EVALUATE_ONLY`: only answer which command it would be.
    EvaluateOnly,
}

/// `InGameUI::SelectionRules`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionRules {
    /// `SELECTION_ANY`: one of the selected units has to qualify.
    Any,
    /// `SELECTION_ALL`: all of them have to.
    All,
}

/// `InGameUI::ActionType` (without `ALLOW_SURRENDER`'s `ACTIONTYPE_PICK_UP_PRISONER`, which the release does not build).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionType {
    None,
    AttackObject,
    GetRepairedAt,
    DockAt,
    GetHealedAt,
    RepairObject,
    ResumeConstruction,
    EnterObject,
    HijackVehicle,
    ConvertObjectToCarbomb,
    CaptureBuilding,
    DisableVehicleViaHacking,
    StealCashViaHacking,
    DisableBuildingViaHacking,
    MakeDefector,
    SetRallyPoint,
    CombatdropInto,
    SabotageBuilding,
}

/// A message argument, in the order the message takes them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CmdArg {
    Int(i32),
    Real(f32),
    Bool(bool),
    Object(ObjectId),
    Location(Coord3D),
}

/// A `GUI_COMMAND_SPECIAL_POWER` button of a command set, as the capture and hack branches read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialPowerButton {
    /// `getSpecialPowerTemplate()->getID()`.
    pub template_id: u32,
    /// `getSpecialPowerTemplate()->getSpecialPowerType()`.
    pub sp_type: SpecialPowerType,
    /// `getOptions()`.
    pub options: u32,
}

/// What the command translator asks of the game, beyond the selection's questions. The per-object action questions are the
/// action manager's; [`crate::client::command_world`] answers them from a World.
pub trait CommandXlatHost: SelectionGroupsHost {
    /// `obj->getStatusBits().test(OBJECT_STATUS_MASKED)`.
    fn object_is_masked(&self, obj: ObjectId) -> bool;
    /// `obj->getPosition()`.
    fn object_position(&self, obj: ObjectId) -> Coord3D;
    /// `src->getRelationship(target)`.
    fn object_relationship(&self, src: ObjectId, target: ObjectId) -> Relationship;
    /// The body of `canSelectedObjectsDoAction`'s switch for one selected object (`src` is `other->getObject()`), for the
    /// actions the action manager answers (not `None`, `AttackObject` or `SetRallyPoint`, which are answered here).
    fn object_can_do_action(&mut self, src: Option<ObjectId>, action: ActionType, target: Option<ObjectId>, additional: bool) -> bool;
    /// `TheActionManager->getCanAttackObject(src, target, CMD_FROM_PLAYER, forced ? ATTACK_NEW_TARGET_FORCED : ATTACK_NEW_TARGET)`.
    fn object_can_attack(&mut self, src: Option<ObjectId>, target: Option<ObjectId>, forced: bool) -> CanAttackResultValue;
    /// `canObjectForceAttack(obj, victim, pos)` (CommandXlat.cpp's static).
    fn object_can_force_attack(&mut self, src: ObjectId, victim: Option<ObjectId>, pos: Option<Coord3D>) -> CanAttackResultValue;
    /// `TheActionManager->canOverrideSpecialPowerDestination(src, loc, spType, CMD_FROM_PLAYER)`.
    fn object_can_override_special_power_destination(&mut self, src: Option<ObjectId>, loc: &Coord3D, sp: SpecialPowerType) -> bool;
    /// `obj->isSalvageCrate()`.
    fn is_salvage_crate(&mut self, obj: ObjectId) -> bool;
    /// `ThePartitionManager->getShroudStatusForPlayer(localPlayerIndex, pos) == CELLSHROUD_CLEAR`.
    fn shroud_clear_for_local_player(&mut self, pos: &Coord3D) -> bool;
    /// `obj->getAI() != NULL`.
    fn object_has_ai(&self, obj: ObjectId) -> bool;
    /// `obj->getAI()->isQuickPathAvailable(pos)`.
    fn quick_path_available(&mut self, obj: ObjectId, pos: &Coord3D) -> bool;
    /// `obj->getAI()->hasLocomotorForSurface(LOCOMOTORSURFACE_CLIFF)`.
    fn has_cliff_locomotor(&mut self, obj: ObjectId) -> bool;
    /// `TheTerrainLogic->isCliffCell(x, y)`.
    fn is_cliff_cell(&mut self, x: f32, y: f32) -> bool;
    /// `TheControlBar->findCommandSet(obj->getCommandSetString())`'s `GUI_COMMAND_SPECIAL_POWER` buttons, in slot order.
    fn special_power_buttons(&mut self, obj: ObjectId) -> Vec<SpecialPowerButton>;
    /// `TheMessageStream->appendMessage(t)` with its arguments: a command for the logic.
    fn append_command(&mut self, t: MessageType, args: &[CmdArg]);
    /// The same for a hint (`*_HINT`): for the client's hint spy and cursor, not the logic.
    fn append_hint(&mut self, t: MessageType, args: &[CmdArg]);
}

/// `class CommandTranslator`'s members this batch uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandTranslator {
    /// `m_teamExists`: a selected group was made and not destroyed since.
    pub team_exists: bool,
}

fn objects_of(ui: &InGameUiSelection, h: &impl CommandXlatHost) -> Vec<Option<ObjectId>> {
    ui.all_selected_drawables().iter().map(|&d| h.drawable_object(d)).collect()
}

/// `InGameUI::areSelectedObjectsControllable()`: the first selected object is locally controlled (all of them share it).
pub fn are_selected_objects_controllable(ui: &InGameUiSelection, h: &impl CommandXlatHost) -> bool {
    // (upstream dereferences `draw->getObject()`; a selected drawable without one is not controllable here)
    ui.all_selected_drawables().first().and_then(|&d| h.drawable_object(d)).is_some_and(|o| h.is_locally_controlled(o))
}

/// The sanity check both selection questions start with: no object (and not the rally point), or an object for the rally
/// point.
fn bad_target(action: ActionType, target: Option<ObjectId>) -> bool {
    (target.is_none() && action != ActionType::SetRallyPoint) || (target.is_some() && action == ActionType::SetRallyPoint)
}

/// `InGameUI::getCanSelectedObjectsAttack(action, obj, rule, additionalChecking)`: the best result over the selection for
/// `SELECTION_ANY`, the worst for `SELECTION_ALL`. `additional` is force attack mode.
pub fn get_can_selected_objects_attack(
    action: ActionType,
    target: Option<ObjectId>,
    rule: SelectionRules,
    additional: bool,
    ui: &InGameUiSelection,
    h: &mut impl CommandXlatHost,
) -> CanAttackResultValue {
    if bad_target(action, target) {
        return ATTACKRESULT_NOT_POSSIBLE;
    }
    let selected = objects_of(ui, h);
    let (mut best, mut worst) = (ATTACKRESULT_NOT_POSSIBLE, ATTACKRESULT_POSSIBLE);
    for src in &selected {
        if action != ActionType::AttackObject {
            // "Only accepts attack types!" (a DEBUG_CRASH upstream)
            return ATTACKRESULT_INVALID_SHOT;
        }
        let r = h.object_can_attack(*src, target, additional);
        best = best.max(r);
        worst = worst.min(r);
    }
    if selected.is_empty() {
        return ATTACKRESULT_NOT_POSSIBLE;
    }
    if rule == SelectionRules::Any {
        best
    } else {
        worst
    }
}

/// `InGameUI::canSelectedObjectsDoAction(action, obj, rule, additionalChecking)`.
pub fn can_selected_objects_do_action(
    action: ActionType,
    target: Option<ObjectId>,
    rule: SelectionRules,
    additional: bool,
    ui: &InGameUiSelection,
    h: &mut impl CommandXlatHost,
) -> bool {
    if bad_target(action, target) {
        return false;
    }
    let selected = objects_of(ui, h);
    let mut qualify = 0;
    for src in &selected {
        let success = match action {
            // "it is always possible to do 'nothing'"
            ActionType::None => return true,
            // "You must use InGameUI::getCanSelectedObjectsAttack() instead." (a DEBUG_CRASH upstream)
            ActionType::AttackObject => return false,
            ActionType::SetRallyPoint => src.is_some_and(|o| h.is_kind_of(o, KINDOF_AUTO_RALLYPOINT) && h.is_locally_controlled(o)),
            _ => h.object_can_do_action(*src, action, target, additional),
        };
        if success {
            if rule == SelectionRules::Any {
                return true;
            }
            qualify += 1;
        }
    }
    rule == SelectionRules::All && !selected.is_empty() && qualify == selected.len()
}

/// `InGameUI::canSelectedObjectsOverrideSpecialPowerDestination(loc, rule, spType)`.
pub fn can_selected_objects_override_special_power_destination(
    loc: &Coord3D,
    rule: SelectionRules,
    sp: SpecialPowerType,
    ui: &InGameUiSelection,
    h: &mut impl CommandXlatHost,
) -> bool {
    let selected = objects_of(ui, h);
    let mut qualify = 0;
    for src in &selected {
        if h.object_can_override_special_power_destination(*src, loc, sp) {
            if rule == SelectionRules::Any {
                return true;
            }
            qualify += 1;
        }
    }
    rule == SelectionRules::All && !selected.is_empty() && qualify == selected.len()
}

/// `canSelectionSalvage(targetObj)`: the target is a salvage crate and a selected object is a salvager.
fn can_selection_salvage(target: Option<ObjectId>, ui: &InGameUiSelection, h: &mut impl CommandXlatHost) -> bool {
    let Some(t) = target else { return false };
    if !h.is_salvage_crate(t) {
        return false;
    }
    objects_of(ui, h).into_iter().flatten().any(|o| h.is_kind_of(o, KINDOF_SALVAGER))
}

/// `canAnyForceAttack(allSelected, victim, pos)`: the first selected drawable with an object answers for all.
fn can_any_force_attack(victim: Option<ObjectId>, pos: Option<Coord3D>, ui: &InGameUiSelection, h: &mut impl CommandXlatHost) -> CanAttackResultValue {
    match objects_of(ui, h).into_iter().flatten().next() {
        Some(o) => h.object_can_force_attack(o, victim, pos),
        None => ATTACKRESULT_NOT_POSSIBLE,
    }
}

impl CommandTranslator {
    /// `translateGameMessage`'s selected-group cases: `m_teamExists` follows the group messages that pass through.
    pub fn observe_message(&mut self, t: MessageType) {
        if t == MSG_CREATE_SELECTED_GROUP || t == MSG_CREATE_SELECTED_GROUP_NO_SOUND || (MSG_ADD_TEAM0.0..=MSG_ADD_TEAM9.0).contains(&t.0) {
            self.team_exists = true;
        } else if t == MSG_DESTROY_SELECTED_GROUP {
            self.team_exists = false;
        }
    }

    /// `issueMoveToLocationCommand(pos, drawableInWay, commandType)`.
    pub fn issue_move_to_location_command(
        &self,
        pos: &Coord3D,
        drawable_in_way: Option<DrawableId>,
        ty: CommandEvaluateType,
        ui: &InGameUiSelection,
        h: &mut impl CommandXlatHost,
    ) -> MessageType {
        let obj = drawable_in_way.and_then(|d| h.drawable_object(d));
        let force_attackable = obj.is_some_and(|o| h.is_kind_of(o, KINDOF_FORCEATTACKABLE));
        let mut t = MSG_INVALID;
        if self.team_exists {
            t = if ui.is_in_waypoint_mode() {
                MSG_ADD_WAYPOINT
            } else if ui.is_in_attack_move_to_mode() {
                MSG_DO_ATTACKMOVETO
            } else if ui.is_in_force_move_to_mode() {
                MSG_DO_FORCEMOVETO
            } else if ui.is_in_force_attack_mode() && force_attackable {
                MSG_DO_ATTACK_OBJECT
            } else {
                MSG_DO_MOVETO
            };
            if ty == CommandEvaluateType::DoCommand {
                match (t == MSG_DO_ATTACK_OBJECT, obj) {
                    (true, Some(o)) => h.append_command(t, &[CmdArg::Object(o)]),
                    _ => h.append_command(t, &[CmdArg::Location(*pos)]),
                }
            }
        }
        t
    }

    /// `createAttackMessage(draw, other, commandType)`: both need an object.
    fn create_attack_message(draw: DrawableId, other: DrawableId, ty: CommandEvaluateType, h: &mut impl CommandXlatHost) -> MessageType {
        if h.drawable_object(draw).is_none() {
            return MSG_INVALID;
        }
        let Some(target) = h.drawable_object(other) else { return MSG_INVALID };
        if ty == CommandEvaluateType::DoCommand {
            h.append_command(MSG_DO_ATTACK_OBJECT, &[CmdArg::Object(target)]);
        }
        MSG_DO_ATTACK_OBJECT
    }

    /// `issueAttackCommand(target, commandType, GUI_COMMAND_NONE)`: one attack for the selected group, else one per selected
    /// drawable.
    pub fn issue_attack_command(&self, target: Option<DrawableId>, ty: CommandEvaluateType, ui: &InGameUiSelection, h: &mut impl CommandXlatHost) -> MessageType {
        let Some(target) = target else { return MSG_INVALID };
        let Some(target_obj) = h.drawable_object(target) else { return MSG_INVALID };
        let mut t = MSG_INVALID;
        if self.team_exists {
            t = MSG_DO_ATTACK_OBJECT;
            if ty == CommandEvaluateType::DoCommand {
                h.append_command(t, &[CmdArg::Object(target_obj)]);
            }
        } else {
            for &d in ui.all_selected_drawables() {
                t = Self::create_attack_message(d, target, ty, h);
            }
        }
        t
    }

    /// `createEnterMessage(enter, commandType)`: `MSG_ENTER` for the selected group (`INVALID_ID`) into the target.
    fn create_enter_message(&self, enter: Option<DrawableId>, ty: CommandEvaluateType, h: &mut impl CommandXlatHost) -> MessageType {
        if ty == CommandEvaluateType::EvaluateOnly {
            return MSG_ENTER;
        }
        let Some(target) = enter.and_then(|d| h.drawable_object(d)) else { return MSG_ENTER };
        // (without a team upstream DEBUG_CRASHes and makes no message)
        if self.team_exists {
            h.append_command(MSG_ENTER, &[CmdArg::Object(INVALID_ID), CmdArg::Object(target)]);
        }
        MSG_ENTER
    }

    /// `issueSpecialPowerCommand(command, commandType, target, pos, NULL)` for a set's special power button (the capture and
    /// hack branches; the shortcut case is C2c's).
    fn issue_special_power_command(
        b: &SpecialPowerButton,
        ty: CommandEvaluateType,
        target: Option<DrawableId>,
        pos: &Coord3D,
        ui: &InGameUiSelection,
        h: &mut impl CommandXlatHost,
    ) -> MessageType {
        let source = ui.first_selected_drawable();
        let target_obj = target.and_then(|d| h.drawable_object(d));
        if b.options & opt::NEED_OBJECT_TARGET != 0 {
            // `command->isValidObjectTarget(sourceDraw, target)`
            let src_obj = source.and_then(|d| h.drawable_object(d));
            let (Some(s), Some(t)) = (src_obj, target_obj) else { return MSG_INVALID };
            if !is_valid_relationship_target(b.options, h.object_relationship(s, t)) {
                return MSG_INVALID;
            }
            if ty == CommandEvaluateType::DoCommand {
                let args = [CmdArg::Int(b.template_id as i32), CmdArg::Object(t), CmdArg::Int(b.options as i32), CmdArg::Object(INVALID_ID)];
                h.append_command(MSG_DO_SPECIAL_POWER_AT_OBJECT, &args);
            }
            MSG_DO_SPECIAL_POWER_AT_OBJECT
        } else if b.options & opt::NEED_TARGET_POS != 0 {
            if ty == CommandEvaluateType::DoCommand {
                let args = [
                    CmdArg::Int(b.template_id as i32),
                    CmdArg::Location(*pos),
                    CmdArg::Real(INVALID_ANGLE),
                    CmdArg::Object(target_obj.unwrap_or(INVALID_ID)),
                    CmdArg::Int(b.options as i32),
                    CmdArg::Object(INVALID_ID),
                ];
                h.append_command(MSG_DO_SPECIAL_POWER_AT_LOCATION, &args);
            }
            MSG_DO_SPECIAL_POWER_AT_LOCATION
        } else {
            if ty == CommandEvaluateType::DoCommand {
                let args = [CmdArg::Int(b.template_id as i32), CmdArg::Int(b.options as i32), CmdArg::Object(INVALID_ID)];
                h.append_command(MSG_DO_SPECIAL_POWER, &args);
            }
            MSG_DO_SPECIAL_POWER
        }
    }

    /// `evaluateForceAttack(draw, pos, type)`.
    pub fn evaluate_force_attack(
        &self,
        draw: Option<DrawableId>,
        pos: Option<&Coord3D>,
        ty: CommandEvaluateType,
        ui: &InGameUiSelection,
        h: &mut impl CommandXlatHost,
    ) -> MessageType {
        let (cmd, hint) = if let Some(d) = draw {
            let Some(obj) = h.drawable_object(d) else { return MSG_INVALID };
            let r = can_any_force_attack(Some(obj), pos.copied(), ui, h);
            ((r, MSG_DO_FORCE_ATTACK_OBJECT, MSG_DO_FORCE_ATTACK_OBJECT_HINT), CmdArg::Object(obj))
        } else if let Some(p) = pos {
            let r = can_any_force_attack(None, Some(*p), ui, h);
            ((r, MSG_DO_FORCE_ATTACK_GROUND, MSG_DO_FORCE_ATTACK_GROUND_HINT), CmdArg::Location(*p))
        } else {
            return MSG_INVALID;
        };
        let ((r, command, hint_type), arg) = (cmd, hint);
        if r == ATTACKRESULT_POSSIBLE || r == ATTACKRESULT_POSSIBLE_AFTER_MOVING {
            match ty {
                CommandEvaluateType::DoCommand => {
                    h.append_command(command, &[arg]);
                    command
                }
                CommandEvaluateType::DoHint => {
                    h.append_hint(hint_type, &[]);
                    hint_type
                }
                CommandEvaluateType::EvaluateOnly => command,
            }
        } else if r == ATTACKRESULT_INVALID_SHOT && ty == CommandEvaluateType::DoHint {
            h.append_hint(MSG_IMPOSSIBLE_ATTACK_HINT, &[]);
            MSG_IMPOSSIBLE_ATTACK_HINT
        } else {
            MSG_INVALID
        }
    }

    /// A branch that commands (`DO_COMMAND`, `EVALUATE_ONLY`) with `command`, an object argument and (when doing) the message,
    /// or hints with `hint`.
    fn object_branch(command: MessageType, hint: MessageType, obj: ObjectId, ty: CommandEvaluateType, h: &mut impl CommandXlatHost) -> MessageType {
        match ty {
            CommandEvaluateType::DoHint => {
                h.append_hint(hint, &[CmdArg::Object(obj)]);
                hint
            }
            CommandEvaluateType::DoCommand => {
                h.append_command(command, &[CmdArg::Object(obj)]);
                command
            }
            CommandEvaluateType::EvaluateOnly => command,
        }
    }

    /// The capture and hack branches' search of the first selected object's command set (upstream's "PRELIMINARY CODE FOR
    /// HOOKING IN AUTO SPECIALS"): the first special power button of one of `types` issues its special power.
    fn auto_special(
        types: &[SpecialPowerType],
        ty: CommandEvaluateType,
        draw: Option<DrawableId>,
        pos: &Coord3D,
        ui: &InGameUiSelection,
        h: &mut impl CommandXlatHost,
    ) -> MessageType {
        let Some(source) = ui.first_selected_drawable().and_then(|d| h.drawable_object(d)) else { return MSG_INVALID };
        for b in h.special_power_buttons(source) {
            if types.contains(&b.sp_type) {
                return Self::issue_special_power_command(&b, ty, draw, pos, ui, h);
            }
        }
        MSG_INVALID
    }

    /// `evaluateContextCommand(draw, pos, type)` with no GUI command pending (see the module docs): the command a click on
    /// `draw` (or on the ground at `pos`) gives the selection, made when `ty` is `DoCommand`, hinted when `DoHint`.
    pub fn evaluate_context_command(
        &self,
        draw: Option<DrawableId>,
        pos: &Coord3D,
        ty: CommandEvaluateType,
        ui: &InGameUiSelection,
        h: &mut impl CommandXlatHost,
    ) -> MessageType {
        use CommandEvaluateType::{DoCommand, DoHint, EvaluateOnly};
        let mut obj = draw.and_then(|d| h.drawable_object(d));
        let mut draw = draw;
        let mut drawable_in_way = draw;
        // masked objects are evaluated as the position, except shrubbery and the force-attackable
        if obj.is_none_or(|o| h.object_is_masked(o) && !h.is_kind_of(o, KINDOF_SHRUBBERY) && !h.is_kind_of(o, KINDOF_FORCEATTACKABLE)) {
            draw = None;
            obj = None;
        }
        // "If the thing is a mine, and is locally controlled, then we should issue a moveto to its location."
        if obj.is_some_and(|o| h.is_locally_controlled(o) && h.is_kind_of(o, KINDOF_MINE)) {
            draw = None;
            obj = None;
        }
        if ui.is_in_force_move_to_mode() {
            draw = None;
            obj = None;
        } else if ui.is_in_force_attack_mode() {
            drawable_in_way = draw;
        }
        if obj.is_some_and(|o| h.is_locally_controlled(o) && ui.is_in_prefer_selection_mode()) {
            return MSG_INVALID;
        }
        // "Now that we can select non-controllable units/structures, don't allow any actions to be performed."
        if !are_selected_objects_controllable(ui, h) {
            return MSG_INVALID;
        }
        if ui.is_in_waypoint_mode() {
            if ty == DoHint {
                h.append_hint(MSG_ADD_WAYPOINT_HINT, &[CmdArg::Location(*pos)]);
                return MSG_ADD_WAYPOINT_HINT;
            }
            return self.issue_move_to_location_command(pos, draw, ty, ui, h);
        }
        let force = ui.is_in_force_attack_mode();
        let any = SelectionRules::Any;
        let can = |a: ActionType, rule: SelectionRules, extra: bool, h: &mut _| can_selected_objects_do_action(a, obj, rule, extra, ui, h);
        let mut result = ATTACKRESULT_NOT_POSSIBLE;
        if can_selected_objects_override_special_power_destination(pos, any, SPECIAL_INVALID, ui, h) {
            return match ty {
                DoHint => {
                    h.append_hint(MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT, &[]);
                    MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT
                }
                _ => {
                    if ty == DoCommand {
                        let args = [CmdArg::Location(*pos), CmdArg::Int(SPECIAL_INVALID), CmdArg::Object(INVALID_ID)];
                        h.append_command(MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION, &args);
                    }
                    MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION
                }
            };
        }
        // (the branches below test `draw`; with `draw` set, `obj` is too)
        let o = obj.unwrap_or(INVALID_ID);
        if draw.is_some() && !force && can(ActionType::ResumeConstruction, any, false, h) {
            return Self::object_branch(MSG_RESUME_CONSTRUCTION, MSG_RESUME_CONSTRUCTION_HINT, o, ty, h);
        }
        if draw.is_some() && !force && can(ActionType::DockAt, SelectionRules::All, false, h) {
            return Self::object_branch(MSG_DOCK, MSG_DOCK_HINT, o, ty, h);
        }
        if draw.is_some() && !force && can(ActionType::RepairObject, any, false, h) {
            return Self::object_branch(MSG_DO_REPAIR, MSG_DO_REPAIR_HINT, o, ty, h);
        }
        if draw.is_some() && !force && can(ActionType::GetRepairedAt, any, false, h) {
            return Self::object_branch(MSG_GET_REPAIRED, MSG_GET_REPAIRED_HINT, o, ty, h);
        }
        if draw.is_some() && !force && can(ActionType::GetHealedAt, any, false, h) {
            return Self::object_branch(MSG_GET_HEALED, MSG_GET_HEALED_HINT, o, ty, h);
        }
        for (action, hint) in [
            (ActionType::HijackVehicle, MSG_HIJACK_HINT),
            (ActionType::ConvertObjectToCarbomb, MSG_CONVERT_TO_CARBOMB_HINT),
            (ActionType::SabotageBuilding, MSG_SABOTAGE_HINT),
        ] {
            if draw.is_some() && !force && can(action, any, false, h) {
                if ty == DoHint {
                    h.append_hint(hint, &[CmdArg::Object(o)]);
                    return hint;
                }
                return self.create_enter_message(draw, ty, h);
            }
        }
        if draw.is_some() && !force && can_selection_salvage(obj, ui, h) {
            let at = CmdArg::Location(h.object_position(o));
            if ty == DoHint {
                h.append_hint(MSG_DO_SALVAGE_HINT, &[at]);
                return MSG_DO_SALVAGE_HINT;
            }
            if ty == DoCommand {
                h.append_command(MSG_DO_SALVAGE, &[at]);
            }
            return MSG_DO_SALVAGE;
        }
        if draw.is_some() && !force && can(ActionType::EnterObject, any, true, h) {
            if ty == DoHint {
                h.append_hint(MSG_ENTER_HINT, &[CmdArg::Object(o)]);
                return MSG_ENTER_HINT;
            }
            return self.create_enter_message(draw, ty, h);
        }
        if draw.is_some() {
            result = get_can_selected_objects_attack(ActionType::AttackObject, obj, any, force, ui, h);
            for (r, hint) in [(ATTACKRESULT_POSSIBLE, MSG_DO_ATTACK_OBJECT_HINT), (ATTACKRESULT_POSSIBLE_AFTER_MOVING, MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT)] {
                if result == r {
                    if ty == DoHint {
                        h.append_hint(hint, &[CmdArg::Object(o)]);
                        return hint;
                    }
                    return self.issue_attack_command(draw, ty, ui, h);
                }
            }
        }
        if draw.is_some() && can(ActionType::CaptureBuilding, any, false, h) {
            // upstream walks the set for either type when commanding, and hints from the first matching button
            if ty == DoHint {
                let Some(source) = ui.first_selected_drawable().and_then(|d| h.drawable_object(d)) else { return MSG_INVALID };
                let mut t = MSG_INVALID;
                for b in h.special_power_buttons(source) {
                    // (no `break` upstream: a later button's hint is made too)
                    if b.sp_type == SPECIAL_BLACKLOTUS_CAPTURE_BUILDING {
                        t = MSG_HACK_HINT;
                        h.append_hint(t, &[CmdArg::Object(o)]);
                    } else if b.sp_type == SPECIAL_INFANTRY_CAPTURE_BUILDING {
                        t = MSG_CAPTUREBUILDING_HINT;
                        h.append_hint(t, &[CmdArg::Object(o)]);
                    }
                }
                return t;
            }
            return Self::auto_special(&[SPECIAL_BLACKLOTUS_CAPTURE_BUILDING, SPECIAL_INFANTRY_CAPTURE_BUILDING], ty, draw, pos, ui, h);
        }
        for (action, sp) in [
            (ActionType::DisableVehicleViaHacking, SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK),
            (ActionType::StealCashViaHacking, SPECIAL_BLACKLOTUS_STEAL_CASH_HACK),
            (ActionType::DisableBuildingViaHacking, SPECIAL_HACKER_DISABLE_BUILDING),
        ] {
            if draw.is_some() && can(action, any, false, h) {
                if ty == DoHint {
                    h.append_hint(MSG_HACK_HINT, &[CmdArg::Object(o)]);
                    return MSG_HACK_HINT;
                }
                return Self::auto_special(&[sp], ty, draw, pos, ui, h);
            }
        }
        if draw.is_none() && can_selected_objects_do_action(ActionType::SetRallyPoint, None, SelectionRules::All, false, ui, h) {
            match ty {
                DoCommand => {
                    for src in objects_of(ui, h).into_iter().flatten() {
                        h.append_command(MSG_SET_RALLY_POINT, &[CmdArg::Object(src), CmdArg::Location(*pos)]);
                    }
                }
                DoHint => {
                    h.append_hint(MSG_SET_RALLY_POINT_HINT, &[CmdArg::Location(*pos)]);
                    return MSG_SET_RALLY_POINT_HINT;
                }
                EvaluateOnly => {}
            }
            return MSG_SET_RALLY_POINT;
        }
        if draw.is_some() && result == ATTACKRESULT_INVALID_SHOT {
            // (the hint is made whatever the type, as upstream does)
            h.append_hint(MSG_IMPOSSIBLE_ATTACK_HINT, &[CmdArg::Location(*pos)]);
            return MSG_IMPOSSIBLE_ATTACK_HINT;
        }
        // the move
        if ty != DoHint {
            // "If draw is valid, then its one of ours and we don't have something more specific to do."
            return if draw.is_none() { self.issue_move_to_location_command(pos, drawable_in_way, ty, ui, h) } else { MSG_INVALID };
        }
        let mut valid_quick_path = !h.shroud_clear_for_local_player(pos);
        if !valid_quick_path {
            for src in objects_of(ui, h).into_iter().flatten() {
                if !h.object_has_ai(src) {
                    continue;
                }
                if h.quick_path_available(src, pos) || (h.has_cliff_locomotor(src) && h.is_cliff_cell(pos.x, pos.y)) {
                    valid_quick_path = true;
                    break;
                }
            }
        }
        let t = if !valid_quick_path {
            MSG_DO_INVALID_HINT
        } else if ui.is_in_waypoint_mode() {
            MSG_ADD_WAYPOINT_HINT
        } else if ui.is_in_attack_move_to_mode() {
            MSG_DO_ATTACKMOVETO_HINT
        } else {
            MSG_DO_MOVETO_HINT
        };
        h.append_hint(t, &[CmdArg::Location(*pos)]);
        t
    }

    /// `MSG_MOUSEOVER_DRAWABLE_HINT` / `MSG_MOUSEOVER_LOCATION_HINT`: with something selected, the hint of the command a click
    /// on `draw` (at its position) or on the ground at `pos` would give.
    pub fn mouse_over(&self, draw: Option<DrawableId>, pos: &Coord3D, ui: &InGameUiSelection, h: &mut impl CommandXlatHost) -> MessageType {
        let pos = match draw {
            Some(d) => {
                if ui.select_count() == 0 {
                    return MSG_INVALID;
                }
                match h.drawable_object(d) {
                    Some(o) => h.object_position(o),
                    // (upstream reads the drawable's own position; without an object no command is possible)
                    None => *pos,
                }
            }
            None => *pos,
        };
        if ui.is_in_force_attack_mode() {
            self.evaluate_force_attack(draw, Some(&pos), CommandEvaluateType::DoHint, ui, h)
        } else {
            self.evaluate_context_command(draw, &pos, CommandEvaluateType::DoHint, ui, h)
        }
    }

    /// `MSG_MOUSE_LEFT_CLICK` (and `MSG_MOUSE_RIGHT_CLICK` in alternate mouse mode, `use_alternate_mouse` set and the button
    /// released as a click): a point click with a controllable selection orders it, at `pos` on the terrain, onto `picked` (the
    /// tactical view's `pickDrawable` for the context). Returns whether the message is destroyed.
    pub fn click(&self, right: bool, is_point: bool, pos: &Coord3D, picked: Option<DrawableId>, ui: &mut InGameUiSelection, h: &mut impl CommandXlatHost) -> bool {
        // in alternate mouse mode a left click is only actioned here for a firing GUI command (C2c), and a right click only there
        if h.use_alternate_mouse() != right {
            return false;
        }
        if !(is_point && are_selected_objects_controllable(ui, h)) {
            return false;
        }
        if ui.is_in_force_attack_mode() {
            self.evaluate_force_attack(picked, Some(pos), CommandEvaluateType::DoCommand, ui, h);
        } else {
            self.evaluate_context_command(picked, pos, CommandEvaluateType::DoCommand, ui, h);
        }
        ui.clear_attack_move_to_mode();
        true
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::selection_xlat::tests::Host;
    use std::collections::{BTreeMap, BTreeSet};

    /// The command questions' answers, on the selection test host.
    #[derive(Default)]
    pub(crate) struct CmdFacts {
        /// `(src, action, target)` the action manager allows.
        actions: BTreeSet<(ObjectId, u8, ObjectId)>,
        /// `(src, target)`: the attack result (not possible when absent).
        attacks: BTreeMap<(ObjectId, ObjectId), CanAttackResultValue>,
        commands: Vec<(MessageType, Vec<CmdArg>)>,
        hints: Vec<(MessageType, Vec<CmdArg>)>,
        no_path: bool,
        buttons: Vec<SpecialPowerButton>,
    }

    fn code(a: ActionType) -> u8 {
        a as u8
    }

    impl CommandXlatHost for Host {
        fn object_is_masked(&self, _obj: ObjectId) -> bool {
            false
        }
        fn object_position(&self, obj: ObjectId) -> Coord3D {
            Coord3D { x: obj as f32, y: 0.0, z: 0.0 }
        }
        fn object_relationship(&self, src: ObjectId, target: ObjectId) -> Relationship {
            if self.mine.contains(&src) == self.mine.contains(&target) {
                Relationship::Allies
            } else {
                Relationship::Enemies
            }
        }
        fn object_can_do_action(&mut self, src: Option<ObjectId>, action: ActionType, target: Option<ObjectId>, _additional: bool) -> bool {
            match (src, target) {
                (Some(s), Some(t)) => self.cmd.actions.contains(&(s, code(action), t)),
                _ => false,
            }
        }
        fn object_can_attack(&mut self, src: Option<ObjectId>, target: Option<ObjectId>, _forced: bool) -> CanAttackResultValue {
            match (src, target) {
                (Some(s), Some(t)) => self.cmd.attacks.get(&(s, t)).copied().unwrap_or(ATTACKRESULT_NOT_POSSIBLE),
                _ => ATTACKRESULT_NOT_POSSIBLE,
            }
        }
        fn object_can_force_attack(&mut self, _src: ObjectId, _victim: Option<ObjectId>, _pos: Option<Coord3D>) -> CanAttackResultValue {
            ATTACKRESULT_POSSIBLE
        }
        fn object_can_override_special_power_destination(&mut self, _src: Option<ObjectId>, _loc: &Coord3D, _sp: SpecialPowerType) -> bool {
            false
        }
        fn is_salvage_crate(&mut self, _obj: ObjectId) -> bool {
            false
        }
        fn shroud_clear_for_local_player(&mut self, _pos: &Coord3D) -> bool {
            true
        }
        fn object_has_ai(&self, _obj: ObjectId) -> bool {
            true
        }
        fn quick_path_available(&mut self, _obj: ObjectId, _pos: &Coord3D) -> bool {
            !self.cmd.no_path
        }
        fn has_cliff_locomotor(&mut self, _obj: ObjectId) -> bool {
            false
        }
        fn is_cliff_cell(&mut self, _x: f32, _y: f32) -> bool {
            false
        }
        fn special_power_buttons(&mut self, _obj: ObjectId) -> Vec<SpecialPowerButton> {
            self.cmd.buttons.clone()
        }
        fn append_command(&mut self, t: MessageType, args: &[CmdArg]) {
            self.cmd.commands.push((t, args.to_vec()));
        }
        fn append_hint(&mut self, t: MessageType, args: &[CmdArg]) {
            self.cmd.hints.push((t, args.to_vec()));
        }
    }

    /// C2b: `evaluateContextCommand`'s order of precedence. Predicted from CommandXlat.cpp, with my tank 1 selected as a group,
    /// an enemy 9, my transport 2 and the ground at x = 50:
    /// - the ground: `MSG_DO_MOVETO(50,0,0)` (`DO_COMMAND`); the hint `MSG_DO_MOVETO_HINT`, and `MSG_DO_INVALID_HINT` without a
    ///   quick path;
    /// - the enemy, attackable: `MSG_DO_ATTACK_OBJECT(9)`; after moving: the same command, the hint
    ///   `MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT`;
    /// - my transport, enterable: `MSG_ENTER(INVALID_ID, 2)`, which wins over attacking it;
    /// - my transport, nothing to do: no command (`MSG_INVALID`) and no move (it is one of mine);
    /// - with no group (`m_teamExists` false), the ground gives no move and the enemy one attack per selected drawable;
    /// - the waypoint mode makes `MSG_ADD_WAYPOINT`, attack move `MSG_DO_ATTACKMOVETO`;
    /// - an engineer (3) that can capture with an infantry capture button issues `MSG_DO_SPECIAL_POWER_AT_OBJECT(template, 9,
    ///   options, INVALID_ID)`, and hints `MSG_CAPTUREBUILDING_HINT`.
    #[test]
    fn a_click_orders_the_selection_by_the_context() {
        use CommandEvaluateType::{DoCommand, DoHint};
        let mut h = Host { mine: vec![1, 2, 3], ..Default::default() };
        h.rel.insert(9, Relationship::Enemies);
        let mut ui = InGameUiSelection::new();
        ui.select_drawable(1, &mut h);
        let mut x = CommandTranslator::default();
        x.observe_message(MSG_CREATE_SELECTED_GROUP);
        let ground = Coord3D { x: 50.0, y: 0.0, z: 0.0 };

        assert_eq!(x.evaluate_context_command(None, &ground, DoCommand, &ui, &mut h), MSG_DO_MOVETO);
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DO_MOVETO, vec![CmdArg::Location(ground)])));
        assert_eq!(x.evaluate_context_command(None, &ground, DoHint, &ui, &mut h), MSG_DO_MOVETO_HINT);
        h.cmd.no_path = true;
        assert_eq!(x.evaluate_context_command(None, &ground, DoHint, &ui, &mut h), MSG_DO_INVALID_HINT);
        h.cmd.no_path = false;

        h.cmd.attacks.insert((1, 9), ATTACKRESULT_POSSIBLE);
        assert_eq!(x.evaluate_context_command(Some(9), &ground, DoCommand, &ui, &mut h), MSG_DO_ATTACK_OBJECT);
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DO_ATTACK_OBJECT, vec![CmdArg::Object(9)])));
        h.cmd.attacks.insert((1, 9), ATTACKRESULT_POSSIBLE_AFTER_MOVING);
        assert_eq!(x.evaluate_context_command(Some(9), &ground, DoHint, &ui, &mut h), MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT);

        h.cmd.attacks.insert((1, 2), ATTACKRESULT_POSSIBLE);
        h.cmd.actions.insert((1, code(ActionType::EnterObject), 2));
        assert_eq!(x.evaluate_context_command(Some(2), &ground, DoCommand, &ui, &mut h), MSG_ENTER);
        assert_eq!(h.cmd.commands.pop(), Some((MSG_ENTER, vec![CmdArg::Object(INVALID_ID), CmdArg::Object(2)])));
        h.cmd.actions.clear();
        h.cmd.attacks.remove(&(1, 2));
        let before = h.cmd.commands.len();
        assert_eq!(x.evaluate_context_command(Some(2), &ground, DoCommand, &ui, &mut h), MSG_INVALID);
        assert_eq!(h.cmd.commands.len(), before, "no move onto one of mine");

        x.observe_message(MSG_DESTROY_SELECTED_GROUP);
        assert_eq!(x.evaluate_context_command(None, &ground, DoCommand, &ui, &mut h), MSG_INVALID);
        h.cmd.attacks.insert((1, 9), ATTACKRESULT_POSSIBLE);
        assert_eq!(x.evaluate_context_command(Some(9), &ground, DoCommand, &ui, &mut h), MSG_DO_ATTACK_OBJECT);
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DO_ATTACK_OBJECT, vec![CmdArg::Object(9)])));
        x.observe_message(MSG_ADD_TEAM0);

        ui.set_waypoint_mode(true);
        assert_eq!(x.evaluate_context_command(None, &ground, DoCommand, &ui, &mut h), MSG_ADD_WAYPOINT);
        ui.set_waypoint_mode(false);
        ui.set_attack_move_to_mode(true);
        assert_eq!(x.evaluate_context_command(None, &ground, DoCommand, &ui, &mut h), MSG_DO_ATTACKMOVETO);
        ui.clear_attack_move_to_mode();

        ui.deselect_all_drawables(false, &mut h);
        ui.select_drawable(3, &mut h);
        h.cmd.actions.insert((3, code(ActionType::CaptureBuilding), 9));
        let capture = SpecialPowerButton { template_id: 7, sp_type: SPECIAL_INFANTRY_CAPTURE_BUILDING, options: opt::NEED_TARGET_ENEMY_OBJECT };
        h.cmd.buttons = vec![capture];
        assert_eq!(x.evaluate_context_command(Some(9), &ground, DoCommand, &ui, &mut h), MSG_DO_SPECIAL_POWER_AT_OBJECT);
        let args = vec![CmdArg::Int(7), CmdArg::Object(9), CmdArg::Int(opt::NEED_TARGET_ENEMY_OBJECT as i32), CmdArg::Object(INVALID_ID)];
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DO_SPECIAL_POWER_AT_OBJECT, args)));
        assert_eq!(x.evaluate_context_command(Some(9), &ground, DoHint, &ui, &mut h), MSG_CAPTUREBUILDING_HINT);
    }

    /// C2b: the selection questions' rules and the click. Predicted from InGameUI.cpp: with 1 and 2 selected and only 1 able
    /// to dock at 5, `SELECTION_ANY` is true and `SELECTION_ALL` false; the attack result is the best (`ANY`) or worst (`ALL`)
    /// over the selection; a rally point needs no object, every selected unit `AUTO_RALLYPOINT`. A left click with a point
    /// orders and destroys the message; a drag (not a point) does not; a right click is not actioned without the alternate
    /// mouse; the attack move mode is cleared after an order.
    #[test]
    fn the_selection_questions_and_the_click() {
        let mut h = Host { mine: vec![1, 2], ..Default::default() };
        let mut ui = InGameUiSelection::new();
        ui.select_drawable(1, &mut h);
        ui.select_drawable(2, &mut h);
        h.cmd.actions.insert((1, code(ActionType::DockAt), 5));
        assert!(can_selected_objects_do_action(ActionType::DockAt, Some(5), SelectionRules::Any, false, &ui, &mut h));
        assert!(!can_selected_objects_do_action(ActionType::DockAt, Some(5), SelectionRules::All, false, &ui, &mut h));
        assert!(!can_selected_objects_do_action(ActionType::DockAt, None, SelectionRules::Any, false, &ui, &mut h));
        h.cmd.attacks.insert((1, 9), ATTACKRESULT_POSSIBLE);
        h.cmd.attacks.insert((2, 9), ATTACKRESULT_INVALID_SHOT);
        assert_eq!(get_can_selected_objects_attack(ActionType::AttackObject, Some(9), SelectionRules::Any, false, &ui, &mut h), ATTACKRESULT_POSSIBLE);
        assert_eq!(get_can_selected_objects_attack(ActionType::AttackObject, Some(9), SelectionRules::All, false, &ui, &mut h), ATTACKRESULT_INVALID_SHOT);
        assert!(!can_selected_objects_do_action(ActionType::SetRallyPoint, None, SelectionRules::All, false, &ui, &mut h));

        let mut x = CommandTranslator::default();
        x.observe_message(MSG_CREATE_SELECTED_GROUP);
        let ground = Coord3D { x: 4.0, y: 5.0, z: 0.0 };
        ui.set_attack_move_to_mode(true);
        assert!(!x.click(false, false, &ground, None, &mut ui, &mut h), "a drag is not a click");
        assert!(!x.click(true, true, &ground, None, &mut ui, &mut h), "a right click without the alternate mouse");
        assert!(x.click(false, true, &ground, None, &mut ui, &mut h));
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DO_ATTACKMOVETO, vec![CmdArg::Location(ground)])));
        assert!(!ui.is_in_attack_move_to_mode());
        ui.set_force_attack_mode(true);
        assert!(x.click(false, true, &ground, None, &mut ui, &mut h));
        assert_eq!(h.cmd.commands.pop(), Some((MSG_DO_FORCE_ATTACK_GROUND, vec![CmdArg::Location(ground)])));
    }
}
