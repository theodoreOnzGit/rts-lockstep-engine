// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarCommandProcessing.cpp (ControlBar::processCommandUI,
//     selectObjectOfType)
//   GeneralsMD/Code/GameEngine/Include/GameClient/ControlBar.h (GUICommandType, CommandOption)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The control bar's buttons (C2c, GitHub #6): `ControlBar::processCommandUI`, what pressing a command button does. A button
//! that needs a target becomes the pending GUI command (~~the cursor mode [`crate::client::command_xlat`] evaluates~~
//! **Corrected 2026-11-06 (#54):** a context command's cursor mode, and a special power's, are evaluated by
//! [`crate::client::command_xlat`]; the others (guard, attack move, rally point, beacon, evacuate to a position, fire a
//! weapon) are carried out by the next click through [`crate::client::gui_command_xlat`]); the others
//! build (or start a building's placement), queue upgrades, cancel, stop, sell, evacuate, fire, use a special power, switch
//! weapons, select every unit of a type or buy a science, as messages for the logic.
//!
//! **Not here:** the buttons' windows and images, flashing, ~~and the unit-specific sounds (the view and audio, C2d);~~
//! **Corrected 2026-11-06 (#51):** the unit voices of evacuate, hack internet and switch weapon are played
//! ([`crate::client::unit_voice`]); the EVA
//! "insufficient funds" voice is the host's [`ControlBarHost::eva_insufficient_funds`].

use crate::client::command_xlat::{CmdArg, CommandXlatHost};
use crate::client::in_game_ui::{DrawableId, InGameUiSelection};
use crate::client::unit_voice::{pick_and_play_unit_voice_response, PickAndPlayInfo};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::message_types::*;
use crate::common::special_power_type::SpecialPowerType;
use crate::logic::world::build_assistant::CanMakeType;

/// `GUICommandType` (without `ALLOW_SURRENDER`), the place in `TheGuiCommandNames`.
pub mod gui {
    pub const NONE: i32 = 0;
    pub const DOZER_CONSTRUCT: i32 = 1;
    pub const DOZER_CONSTRUCT_CANCEL: i32 = 2;
    pub const UNIT_BUILD: i32 = 3;
    pub const CANCEL_UNIT_BUILD: i32 = 4;
    pub const PLAYER_UPGRADE: i32 = 5;
    pub const OBJECT_UPGRADE: i32 = 6;
    pub const CANCEL_UPGRADE: i32 = 7;
    pub const ATTACK_MOVE: i32 = 8;
    pub const GUARD: i32 = 9;
    pub const GUARD_WITHOUT_PURSUIT: i32 = 10;
    pub const GUARD_FLYING_UNITS_ONLY: i32 = 11;
    pub const STOP: i32 = 12;
    pub const WAYPOINTS: i32 = 13;
    pub const EXIT_CONTAINER: i32 = 14;
    pub const EVACUATE: i32 = 15;
    pub const EXECUTE_RAILED_TRANSPORT: i32 = 16;
    pub const BEACON_DELETE: i32 = 17;
    pub const SET_RALLY_POINT: i32 = 18;
    pub const SELL: i32 = 19;
    pub const FIRE_WEAPON: i32 = 20;
    pub const SPECIAL_POWER: i32 = 21;
    pub const PURCHASE_SCIENCE: i32 = 22;
    pub const HACK_INTERNET: i32 = 23;
    pub const TOGGLE_OVERCHARGE: i32 = 24;
    pub const COMBATDROP: i32 = 25;
    pub const SWITCH_WEAPON: i32 = 26;
    /// `GUICOMMANDMODE_HIJACK_VEHICLE`.
    pub const HIJACK_VEHICLE: i32 = 27;
    /// `GUICOMMANDMODE_CONVERT_TO_CARBOMB`.
    pub const CONVERT_TO_CARBOMB: i32 = 28;
    /// `GUICOMMANDMODE_SABOTAGE_BUILDING`.
    pub const SABOTAGE_BUILDING: i32 = 29;
    /// `GUICOMMANDMODE_PLACE_BEACON`.
    pub const PLACE_BEACON: i32 = 30;
    pub const SPECIAL_POWER_FROM_SHORTCUT: i32 = 31;
    pub const SPECIAL_POWER_CONSTRUCT: i32 = 32;
    pub const SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT: i32 = 33;
    pub const SELECT_ALL_UNITS_OF_TYPE: i32 = 34;
}

/// `CommandOption` bits and masks.
pub mod opt {
    pub use crate::logic::world::object_command_button::opt::*;
    pub const ALLOW_SHRUBBERY_TARGET: u32 = 0x0000_0010;
    pub const CONTEXTMODE_COMMAND: u32 = 0x0000_0200;
    pub const ALLOW_MINE_TARGET: u32 = 0x0000_0800;
    pub const SINGLE_USE_COMMAND: u32 = 0x0002_0000;
    pub const USES_MINE_CLEARING_WEAPONSET: u32 = 0x0020_0000;
    /// `COMMAND_OPTION_NEED_TARGET`.
    pub const NEED_TARGET: u32 = NEED_OBJECT_TARGET | NEED_TARGET_POS | CONTEXTMODE_COMMAND;
}

/// A `CommandButton`, as the client reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiButton {
    /// The button's place in the World's button list (`cuts.command_buttons`).
    pub index: usize,
    /// `getCommandType()`.
    pub command_type: i32,
    /// `getOptions()`.
    pub options: u32,
    /// `getWeaponSlot()`.
    pub weapon_slot: i32,
    /// `getMaxShotsToFire()`.
    pub max_shots_to_fire: i32,
    /// `getSpecialPowerTemplate()`: its id and type.
    pub special_power: Option<(u32, SpecialPowerType)>,
    /// `getThingTemplate()`'s id.
    pub thing_template: Option<u16>,
    /// `getUpgradeTemplate()`'s name key.
    pub upgrade: Option<u32>,
    /// `getScienceVec()`.
    pub sciences: Vec<i32>,
}

impl GuiButton {
    /// `isContextCommand()`.
    pub fn is_context_command(&self) -> bool {
        self.options & opt::CONTEXTMODE_COMMAND != 0
    }
}

/// `enum CBCommandStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CbCommandStatus {
    Used,
    NotUsed,
}

/// What the pressed control is: a command button, an entry of the build queue (`m_queueData`), or a container's
/// occupant (`m_containData`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PressedControl {
    Command,
    /// A unit in production, by its production id.
    QueuedUnit(u32),
    /// An upgrade in production, by its name key.
    QueuedUpgrade(u32),
    /// The object wanting to exit (`None`: the slot's object is gone).
    Occupant(Option<ObjectId>),
}

/// What the control bar asks of the game, beyond the command translator's questions.
pub trait ControlBarHost: CommandXlatHost {
    /// `TheBuildAssistant->canMakeUnit(builder, what)`.
    fn can_make_unit(&mut self, builder: Option<ObjectId>, what: Option<u16>) -> CanMakeType;
    /// `TheEva->setShouldPlay(EVA_InsufficientFunds)`.
    fn eva_insufficient_funds(&mut self);
    /// `factory->getProductionUpdateInterface()->requestUniqueUnitID()`; `None` without a production interface.
    fn request_unique_unit_id(&mut self, factory: ObjectId) -> Option<u32>;
    /// `TheUpgradeCenter->canAffordUpgrade(localPlayer, upgrade, TRUE)` (which shows its own message).
    fn can_afford_upgrade(&mut self, upgrade: u32) -> bool;
    /// `obj->getProductionUpdateInterface()->canQueueUpgrade(upgrade)`; `None` without a production interface.
    fn can_queue_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> Option<CanMakeType>;
    /// `obj->hasUpgrade(upgrade)`.
    fn has_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool;
    /// `obj->affectedByUpgrade(upgrade)`.
    fn affected_by_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool;
    /// `obj->markSingleUseCommandUsed()`.
    fn mark_single_use_command_used(&mut self, obj: ObjectId);
    /// `obj->getProductionUpdateInterface()->setSpecialPowerConstructionCommandButton(button)`, when it has one.
    fn set_special_power_construction_button(&mut self, obj: ObjectId, button: usize);
    /// `ThePlayerList->getLocalPlayer()->iterateObjects`'s order.
    fn local_player_objects(&mut self) -> Vec<ObjectId>;
    /// `TheGameLogic->findObjectByID(id) != NULL`.
    fn object_exists(&self, obj: ObjectId) -> bool;
    /// `localPlayer->hasScience(st)`.
    fn has_science(&mut self, st: i32) -> bool;
    /// `TheScienceStore->playerHasPrereqsForScience(localPlayer, st)`.
    fn has_prereqs_for_science(&mut self, st: i32) -> bool;
    /// `TheScienceStore->getSciencePurchaseCost(st)`.
    fn science_purchase_cost(&mut self, st: i32) -> i32;
    /// `localPlayer->getSciencePurchasePoints()`.
    fn science_purchase_points(&mut self) -> i32;
    /// `switchToContext(CB_CONTEXT_NONE, NULL)`.
    fn switch_to_context_none(&mut self);
    /// `build->getPlacementViewAngle()` (C2c part 2); 0 without a template.
    fn placement_view_angle(&self, build: Option<u16>) -> f32;
}

/// `SCIENCE_INVALID`.
const SCIENCE_INVALID: i32 = -1;

/// The "can make" refusals' messages (`GUI:NotEnoughMoneyToBuild` with the EVA's voice, and the queue, parking and maximum
/// ones); whether the build may go on.
fn may_make(cmt: CanMakeType, h: &mut impl ControlBarHost) -> bool {
    let label: &[u8] = match cmt {
        CanMakeType::NoMoney => {
            h.eva_insufficient_funds();
            b"GUI:NotEnoughMoneyToBuild"
        }
        CanMakeType::QueueFull => b"GUI:ProductionQueueFull",
        CanMakeType::ParkingPlacesFull => b"GUI:ParkingPlacesFull",
        CanMakeType::MaxedOutForPlayer => b"GUI:UnitMaxedOut",
        _ => return true,
    };
    h.ui_message(label, None);
    false
}

/// The upgrade buttons' checks: affordable, and the producer's queue not full.
fn may_queue_upgrade(obj: Option<ObjectId>, upgrade: u32, h: &mut impl ControlBarHost) -> bool {
    if !h.can_afford_upgrade(upgrade) {
        return false;
    }
    if let Some(CanMakeType::QueueFull) = obj.and_then(|o| h.can_queue_upgrade(o, upgrade)) {
        h.ui_message(b"GUI:ProductionQueueFull", None);
        return false;
    }
    true
}

/// `pickAndPlayUnitVoiceResponse(TheInGameUI->getAllSelectedDrawables(), msg, info)` (#51).
fn voice(msg: crate::common::message_stream::MessageType, info: Option<&PickAndPlayInfo>, ui: &InGameUiSelection, h: &mut impl ControlBarHost) {
    let list = ui.all_selected_drawables().to_vec();
    pick_and_play_unit_voice_response(&list, msg, info, ui, h);
}

/// `ControlBar::processCommandUI(control, gadgetMessage)`: `button` was pressed, on `control`, while the control bar shows
/// `selected` (`m_currentSelectedDrawable`) in the context `multi_select` (`CB_CONTEXT_MULTI_SELECT`) or none
/// (`context_none`).
#[allow(clippy::too_many_arguments)]
pub fn process_command_ui(
    button: &GuiButton,
    control: PressedControl,
    selected: Option<DrawableId>,
    multi_select: bool,
    context_none: bool,
    ui: &mut InGameUiSelection,
    h: &mut impl ControlBarHost,
) -> CbCommandStatus {
    let t = button.command_type;
    let sourceless = multi_select
        || t == gui::PURCHASE_SCIENCE
        || t == gui::SPECIAL_POWER_FROM_SHORTCUT
        || t == gui::SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT
        || t == gui::SELECT_ALL_UNITS_OF_TYPE;
    let selected_obj = selected.and_then(|d| h.drawable_object(d));
    // "we won't process messages if we have no source object"
    if !sourceless && selected_obj.is_none() {
        if !context_none {
            h.switch_to_context_none();
        }
        return CbCommandStatus::NotUsed;
    }
    let obj = if sourceless { None } else { selected_obj };
    if let Some(o) = obj {
        if button.options & opt::SINGLE_USE_COMMAND != 0 {
            h.mark_single_use_command_used(o);
        }
    }
    ui.place_build_available(None, None, 0.0, h);

    if button.options & opt::NEED_TARGET != 0 {
        if button.options & opt::USES_MINE_CLEARING_WEAPONSET != 0 {
            h.append_command(MSG_SET_MINE_CLEARING_DETAIL, &[]);
        }
        ui.set_gui_command(Some(button.clone()));
        return CbCommandStatus::Used;
    }
    let sp = button.special_power;
    match t {
        gui::DOZER_CONSTRUCT => {
            if selected.is_some() && may_make(h.can_make_unit(obj, button.thing_template), h) {
                let angle = h.placement_view_angle(button.thing_template);
                ui.place_build_available(button.thing_template, selected, angle, h);
            }
        }
        gui::SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT => {
            // (upstream dereferences the template)
            let Some((_, sp_type)) = sp else { return CbCommandStatus::Used };
            let Some(builder) = h.most_ready_shortcut_special_power(sp_type) else { return CbCommandStatus::Used };
            if may_make(h.can_make_unit(Some(builder), button.thing_template), h) {
                let draw = h.object_drawable(builder);
                let angle = h.placement_view_angle(button.thing_template);
                ui.place_build_available(button.thing_template, draw, angle, h);
                h.set_special_power_construction_button(builder, button.index);
            }
        }
        gui::SPECIAL_POWER_CONSTRUCT => {
            if selected.is_some() && may_make(h.can_make_unit(obj, button.thing_template), h) {
                let angle = h.placement_view_angle(button.thing_template);
                ui.place_build_available(button.thing_template, selected, angle, h);
                if let Some(o) = obj {
                    h.set_special_power_construction_button(o, button.index);
                }
            }
        }
        gui::DOZER_CONSTRUCT_CANCEL => {
            if obj.is_some_and(|b| h.is_locally_controlled(b)) {
                h.append_command(MSG_DOZER_CANCEL_CONSTRUCT, &[]);
            }
        }
        gui::UNIT_BUILD => {
            let Some(factory) = obj else { return CbCommandStatus::Used };
            let cmt = h.can_make_unit(Some(factory), button.thing_template);
            // ("Cannot create ... returns false for canMakeUnit" asserts for any other refusal)
            if !may_make(cmt, h) || cmt != CanMakeType::Ok {
                return CbCommandStatus::Used;
            }
            let Some(id) = h.request_unique_unit_id(factory) else { return CbCommandStatus::Used };
            let what = button.thing_template.map_or(0, i32::from);
            h.append_command(MSG_QUEUE_UNIT_CREATE, &[CmdArg::Int(what), CmdArg::Int(id as i32)]);
        }
        gui::CANCEL_UNIT_BUILD => {
            if let (PressedControl::QueuedUnit(id), Some(producer)) = (control, obj) {
                if h.is_locally_controlled(producer) {
                    h.append_command(MSG_CANCEL_UNIT_CREATE, &[CmdArg::Int(id as i32)]);
                }
            }
        }
        gui::PLAYER_UPGRADE => {
            let (Some(o), Some(u)) = (obj, button.upgrade) else { return CbCommandStatus::Used };
            if may_queue_upgrade(Some(o), u, h) {
                h.append_command(MSG_QUEUE_UPGRADE, &[CmdArg::Object(o), CmdArg::Int(u as i32)]);
            }
        }
        gui::OBJECT_UPGRADE => {
            let Some(u) = button.upgrade else { return CbCommandStatus::Used };
            if !may_queue_upgrade(obj, u, h) {
                return CbCommandStatus::Used;
            }
            if obj.is_some_and(|o| h.has_upgrade(o, u) || !h.affected_by_upgrade(o, u)) {
                return CbCommandStatus::Used;
            }
            h.append_command(MSG_QUEUE_UPGRADE, &[CmdArg::Object(obj.unwrap_or(INVALID_ID)), CmdArg::Int(u as i32)]);
        }
        gui::CANCEL_UPGRADE => {
            if let (PressedControl::QueuedUpgrade(u), Some(_)) = (control, obj) {
                h.append_command(MSG_CANCEL_UPGRADE, &[CmdArg::Int(u as i32)]);
            }
        }
        // `MSG_META_TOGGLE_ATTACKMOVE`, which the command translator answers with `toggleAttackMoveToMode()`
        gui::ATTACK_MOVE => ui.toggle_attack_move_to_mode(),
        gui::STOP => h.append_command(MSG_DO_STOP, &[]),
        gui::SELECT_ALL_UNITS_OF_TYPE => {
            let Some(thing) = button.thing_template else { return CbCommandStatus::Used };
            ui.deselect_all_drawables(true, h);
            let mut ids = Vec::new();
            for o in h.local_player_objects() {
                if h.is_equivalent_to(o, u32::from(thing)) {
                    ids.push(o);
                    if let Some(d) = h.object_drawable(o) {
                        ui.select_drawable(d, h);
                    }
                }
            }
            let mut args = vec![CmdArg::Bool(true)];
            args.extend(ids.into_iter().map(CmdArg::Object));
            h.append_command(MSG_CREATE_SELECTED_GROUP, &args);
        }
        gui::EXIT_CONTAINER => {
            if let PressedControl::Occupant(Some(o)) = control {
                if h.object_exists(o) {
                    h.append_command(MSG_EXIT, &[CmdArg::Object(o)]);
                }
            }
        }
        gui::EVACUATE => {
            ui.set_gui_command(None);
            if button.options & opt::NEED_TARGET_POS == 0 {
                // #51: the voice first, as upstream
                voice(MSG_EVACUATE, None, ui, h);
                h.append_command(MSG_EVACUATE, &[]);
            }
        }
        gui::EXECUTE_RAILED_TRANSPORT => h.append_command(MSG_EXECUTE_RAILED_TRANSPORT, &[]),
        gui::HACK_INTERNET => {
            voice(MSG_INTERNET_HACK, None, ui, h);
            h.append_command(MSG_INTERNET_HACK, &[]);
        }
        gui::SELL => h.append_command(MSG_SELL, &[]),
        gui::TOGGLE_OVERCHARGE => h.append_command(MSG_TOGGLE_OVERCHARGE, &[]),
        // ("hmm, should never occur": these need a target)
        gui::WAYPOINTS | gui::SET_RALLY_POINT | gui::BEACON_DELETE | gui::GUARD | gui::GUARD_WITHOUT_PURSUIT
        | gui::GUARD_FLYING_UNITS_ONLY | gui::COMBATDROP => {}
        gui::SWITCH_WEAPON => {
            // "Play mode change acknowledgement" (between the message and its argument upstream)
            let info = PickAndPlayInfo { weapon_slot: Some(button.weapon_slot), ..Default::default() };
            voice(MSG_SWITCH_WEAPONS, Some(&info), ui, h);
            h.append_command(MSG_SWITCH_WEAPONS, &[CmdArg::Int(button.weapon_slot)]);
        }
        gui::FIRE_WEAPON => h.append_command(MSG_DO_WEAPON, &[CmdArg::Int(button.weapon_slot), CmdArg::Int(button.max_shots_to_fire)]),
        gui::SPECIAL_POWER_FROM_SHORTCUT => {
            let Some((id, sp_type)) = sp else { return CbCommandStatus::Used };
            if let Some(src) = h.most_ready_shortcut_special_power(sp_type) {
                h.append_command(MSG_DO_SPECIAL_POWER, &[CmdArg::Int(id as i32), CmdArg::Int(button.options as i32), CmdArg::Object(src)]);
            }
        }
        gui::SPECIAL_POWER => {
            let Some((id, _)) = sp else { return CbCommandStatus::Used };
            h.append_command(MSG_DO_SPECIAL_POWER, &[CmdArg::Int(id as i32), CmdArg::Int(button.options as i32), CmdArg::Object(INVALID_ID)]);
        }
        gui::PURCHASE_SCIENCE => {
            // the first science the player lacks, has the prerequisites for and can pay for; else the last one listed
            let mut st = SCIENCE_INVALID;
            for &s in &button.sciences {
                st = s;
                if !h.has_science(s) && h.has_prereqs_for_science(s) && h.science_purchase_cost(s) <= h.science_purchase_points() {
                    break;
                }
            }
            if st == SCIENCE_INVALID {
                h.switch_to_context_none();
                return CbCommandStatus::Used;
            }
            h.append_command(MSG_PURCHASE_SCIENCE, &[CmdArg::Int(st)]);
        }
        _ => return CbCommandStatus::NotUsed, // "Unknown command"
    }
    CbCommandStatus::Used
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::common::message_stream::MessageType;
    use crate::client::selection_xlat::tests::Host;
    use std::collections::{BTreeMap, BTreeSet};

    /// The control bar's answers, on the selection test host.
    #[derive(Default)]
    pub(crate) struct BarFacts {
        can_make: BTreeMap<u16, CanMakeType>,
        eva: u32,
        next_unit_id: u32,
        sciences_owned: BTreeSet<i32>,
        points: i32,
        switched_to_none: u32,
        locals: Vec<ObjectId>,
    }

    impl ControlBarHost for Host {
        fn can_make_unit(&mut self, _builder: Option<ObjectId>, what: Option<u16>) -> CanMakeType {
            what.and_then(|t| self.bar.can_make.get(&t).copied()).unwrap_or(CanMakeType::Ok)
        }
        fn eva_insufficient_funds(&mut self) {
            self.bar.eva += 1;
        }
        fn request_unique_unit_id(&mut self, _factory: ObjectId) -> Option<u32> {
            self.bar.next_unit_id += 1;
            Some(self.bar.next_unit_id)
        }
        fn can_afford_upgrade(&mut self, _upgrade: u32) -> bool {
            true
        }
        fn can_queue_upgrade(&mut self, _obj: ObjectId, _upgrade: u32) -> Option<CanMakeType> {
            Some(CanMakeType::Ok)
        }
        fn has_upgrade(&mut self, _obj: ObjectId, _upgrade: u32) -> bool {
            false
        }
        fn affected_by_upgrade(&mut self, _obj: ObjectId, _upgrade: u32) -> bool {
            true
        }
        fn mark_single_use_command_used(&mut self, _obj: ObjectId) {}
        fn set_special_power_construction_button(&mut self, _obj: ObjectId, _button: usize) {}
        fn local_player_objects(&mut self) -> Vec<ObjectId> {
            self.bar.locals.clone()
        }
        fn object_exists(&self, _obj: ObjectId) -> bool {
            true
        }
        fn has_science(&mut self, st: i32) -> bool {
            self.bar.sciences_owned.contains(&st)
        }
        fn has_prereqs_for_science(&mut self, _st: i32) -> bool {
            true
        }
        fn science_purchase_cost(&mut self, _st: i32) -> i32 {
            1
        }
        fn science_purchase_points(&mut self) -> i32 {
            self.bar.points
        }
        fn switch_to_context_none(&mut self) {
            self.bar.switched_to_none += 1;
        }
        fn placement_view_angle(&self, _build: Option<u16>) -> f32 {
            0.5
        }
    }

    fn button(command_type: i32) -> GuiButton {
        GuiButton { command_type, ..Default::default() }
    }

    /// C2c: `processCommandUI`'s buttons. Predicted from ControlBarCommandProcessing.cpp, with my factory 1 shown:
    /// - build template 12: `MSG_QUEUE_UNIT_CREATE(12, production id 1)`; without money: no message, the EVA's voice once and
    ///   `GUI:NotEnoughMoneyToBuild`;
    /// - a dozer's construct button: placement of template 20 starts, built by object 1; no message;
    /// - a special power that needs a position: it becomes the pending GUI command; no message;
    /// - stop `MSG_DO_STOP`, sell `MSG_SELL`, attack move toggles the mode (no message), switch weapon
    ///   `MSG_SWITCH_WEAPONS(slot)`;
    /// - a button with nothing shown (no source, and not a sourceless type): not used, and the bar switches to no context;
    /// - buy a science among [3, 4] owning 3: `MSG_PURCHASE_SCIENCE(4)`;
    /// - select all of template 30 among my objects 1, 2, 3 (1 and 3 of it): `MSG_CREATE_SELECTED_GROUP(true, 1, 3)`, the
    ///   client's selection [3, 1] (newest first).
    #[test]
    fn the_control_bar_buttons_make_their_messages() {
        let mut h = Host { mine: vec![1, 2, 3], ..Default::default() };
        let mut ui = InGameUiSelection::new();
        let used = |b: &GuiButton, ui: &mut InGameUiSelection, h: &mut Host| process_command_ui(b, PressedControl::Command, Some(1), false, false, ui, h);
        let build = GuiButton { thing_template: Some(12), ..button(gui::UNIT_BUILD) };
        assert_eq!(used(&build, &mut ui, &mut h), CbCommandStatus::Used);
        assert_eq!(h.cmd.commands.pop(), Some((MSG_QUEUE_UNIT_CREATE, vec![CmdArg::Int(12), CmdArg::Int(1)])));
        h.bar.can_make.insert(12, CanMakeType::NoMoney);
        used(&build, &mut ui, &mut h);
        assert!(h.cmd.commands.is_empty());
        assert_eq!((h.bar.eva, h.ui_messages.pop()), (1, Some(b"GUI:NotEnoughMoneyToBuild".to_vec())));

        used(&GuiButton { thing_template: Some(20), ..button(gui::DOZER_CONSTRUCT) }, &mut ui, &mut h);
        assert_eq!((ui.pending_place_type(), ui.pending_place_source()), (Some(20), Some(1)));
        let power = GuiButton { options: opt::NEED_TARGET_POS, special_power: Some((7, 1)), ..button(gui::SPECIAL_POWER) };
        used(&power, &mut ui, &mut h);
        assert_eq!(ui.gui_command(), Some(&power));
        assert_eq!(ui.pending_place_type(), None, "the button ends the placement");
        assert!(h.cmd.commands.is_empty());

        used(&button(gui::STOP), &mut ui, &mut h);
        used(&button(gui::SELL), &mut ui, &mut h);
        used(&GuiButton { weapon_slot: 2, ..button(gui::SWITCH_WEAPON) }, &mut ui, &mut h);
        let made: Vec<MessageType> = h.cmd.commands.drain(..).map(|(t, _)| t).collect();
        assert_eq!(made, vec![MSG_DO_STOP, MSG_SELL, MSG_SWITCH_WEAPONS]);
        used(&button(gui::ATTACK_MOVE), &mut ui, &mut h);
        assert!(ui.is_in_attack_move_to_mode());
        assert!(h.cmd.commands.is_empty());

        assert_eq!(process_command_ui(&button(gui::STOP), PressedControl::Command, None, false, false, &mut ui, &mut h), CbCommandStatus::NotUsed);
        assert_eq!(h.bar.switched_to_none, 1);

        h.bar.sciences_owned.insert(3);
        h.bar.points = 1;
        used(&GuiButton { sciences: vec![3, 4], ..button(gui::PURCHASE_SCIENCE) }, &mut ui, &mut h);
        assert_eq!(h.cmd.commands.pop(), Some((MSG_PURCHASE_SCIENCE, vec![CmdArg::Int(4)])));

        h.bar.locals = vec![1, 2, 3];
        h.templates.insert(1, 30);
        h.templates.insert(3, 30);
        used(&GuiButton { thing_template: Some(30), ..button(gui::SELECT_ALL_UNITS_OF_TYPE) }, &mut ui, &mut h);
        assert_eq!(h.cmd.commands.pop(), Some((MSG_CREATE_SELECTED_GROUP, vec![CmdArg::Bool(true), CmdArg::Object(1), CmdArg::Object(3)])));
        assert_eq!(ui.all_selected_drawables(), &[3, 1]);
    }
}
