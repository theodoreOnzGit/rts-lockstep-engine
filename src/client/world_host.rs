// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/SelectionInfo.cpp (PickDrawableStruct, getPickTypesForContext,
//     getPickTypesForCurrentSelection, translatePickTypesToKindof)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The client's host over a `World` (C2d, GitHub #6): every question the selection, command, control bar and placement
//! translators ask, answered from the logic, and their messages put on the World's command list as the local player's. A
//! drawable's id is its object's id (the client draws one drawable per object).
//!
//! **What the view supplies.** The host does not draw: before a translator runs, the client sets [`ClientView`]'s facts
//! (what is on screen, what the double click picked, where pixels meet the terrain) and [`WorldClient::sync_modes`] copies
//! InGameUI's modes. The questions only read the logic: [`tests`] checks that asking them every frame leaves the gate game's
//! CRCs as they were (`AGENTS.md`: the client never changes the simulation).
//!
//! **Not read from the World yet:** the construction special's button kept by a production module
//! (`getSpecialPowerConstructionCommandButton`: no module call returns it), so placement here always builds with a dozer;
//! the quit menu (no shell yet). **Not written:** the single-use command flag ([`ControlBarHost::mark_single_use_command_used`]).

use crate::client::command_xlat::{ActionType, CmdArg, CommandXlatHost, GuiButton, TargetMode};
use crate::client::control_bar::ControlBarHost;
use crate::client::in_game_ui::{DrawableId, InGameUiSelection, SelectionUiHost};
use crate::client::place_event::PlaceHost;
use crate::client::selection_groups::SelectionGroupsHost;
use crate::client::selection_info::SelectionInfoHost;
use crate::client::selection_xlat::SelectionXlatHost;
use crate::common::base_type::{Coord3D, ICoord2D};
use crate::common::game_common::{CellShroudStatus, CommandSourceType, ObjectShroudStatus, Relationship};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{
    KindOfType, KINDOF_ALWAYS_SELECTABLE, KINDOF_FORCEATTACKABLE, KINDOF_MINE, KINDOF_SELECTABLE, KINDOF_SHRUBBERY,
};
use crate::common::message_stream::MessageType;
use crate::common::name_key_generator::NameKeyType;
use crate::common::object_status_types::{OBJECT_STATUS_DETECTED, OBJECT_STATUS_IS_CARBOMB, OBJECT_STATUS_MASKED, OBJECT_STATUS_STEALTHED};
use crate::common::science::ScienceType;
use crate::common::special_power_type::SpecialPowerType;
use crate::logic::action_manager::ActionHost;
use crate::logic::ai::ai_update_types::CanAttackResultValue;
use crate::logic::damage::DAMAGE_FLAME;
use crate::logic::world::build_assistant::{self as ba, CanMakeType, LegalBuildCode};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleReply};

/// What the view knows, set by the client before a translator runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientView {
    /// `TheTacticalView->iterateDrawablesInRegion` over the whole view: the objects on screen.
    pub on_screen: Vec<ObjectId>,
    /// `pickDrawable(point, FALSE, PICK_TYPE_SELECTABLE)` at the last double click.
    pub picked_at_click: Option<ObjectId>,
    /// `screenToTerrain`: the terrain point under each pixel the client asked about. A pixel not listed maps to (x, y, 0).
    pub terrain: Vec<(ICoord2D, Coord3D)>,
    /// `getPickTypesForContext`'s kinds ([`pick_kinds`]).
    pub pick_kinds: Vec<KindOfType>,
}

/// InGameUI's modes, as the selection info's questions read them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientModes {
    pub force_attack: bool,
    pub force_move: bool,
    pub prefer_selection: bool,
    pub gui_command_pending: bool,
    /// `TheGlobalData->m_useAlternateMouse` (off by default, as the shipped options).
    pub alternate_mouse: bool,
}

/// The client's host over a `World`.
pub struct WorldClient {
    pub world: World,
    /// The local player's index.
    pub player: i32,
    pub view: ClientView,
    pub modes: ClientModes,
    /// The message types sent to the logic, in order (the command translator observes them).
    pub sent: Vec<MessageType>,
    /// The hints made, with their arguments (for the cursor).
    pub hints: Vec<(MessageType, Vec<CmdArg>)>,
    /// `TheInGameUI->message(label)`'s labels.
    pub ui_messages: Vec<Vec<u8>>,
    /// The objects `lookAt` was asked to look at.
    pub looks: Vec<ObjectId>,
    /// `EVA_InsufficientFunds` plays.
    pub eva_insufficient_funds: u32,
}

/// `getPickTypesForContext(forceAttackMode)` and `PickDrawableStruct`'s kinds: selectable things; force-attackable ones in
/// force attack mode; mines and shrubbery when the pending GUI command allows them, or (with none pending) shrubbery in
/// force attack mode when a controllable selected unit has a flame weapon; and `ALWAYS_SELECTABLE` outside force attack.
pub fn pick_kinds(ui: &InGameUiSelection, selection_has_flame_weapon: bool) -> Vec<KindOfType> {
    use crate::client::control_bar::opt;
    let force = ui.is_in_force_attack_mode();
    let mut kinds = vec![KINDOF_SELECTABLE];
    if force {
        kinds.push(KINDOF_FORCEATTACKABLE);
    }
    match ui.gui_command() {
        Some(c) => {
            if c.options & opt::ALLOW_MINE_TARGET != 0 {
                kinds.push(KINDOF_MINE);
            }
            if c.options & opt::ALLOW_SHRUBBERY_TARGET != 0 {
                kinds.push(KINDOF_SHRUBBERY);
            }
        }
        None => {
            if force && selection_has_flame_weapon {
                kinds.push(KINDOF_SHRUBBERY);
            }
        }
    }
    if !force {
        kinds.push(KINDOF_ALWAYS_SELECTABLE);
    }
    kinds
}

impl WorldClient {
    pub fn new(world: World, player: i32) -> Self {
        WorldClient {
            world,
            player,
            view: ClientView { pick_kinds: vec![KINDOF_SELECTABLE, KINDOF_ALWAYS_SELECTABLE], ..Default::default() },
            modes: ClientModes::default(),
            sent: Vec::new(),
            hints: Vec::new(),
            ui_messages: Vec::new(),
            looks: Vec::new(),
            eva_insufficient_funds: 0,
        }
    }

    /// Copies InGameUI's modes and the pick kinds they imply, before a translator runs.
    pub fn sync_modes(&mut self, ui: &InGameUiSelection) {
        self.modes.force_attack = ui.is_in_force_attack_mode();
        self.modes.force_move = ui.is_in_force_move_to_mode();
        self.modes.prefer_selection = ui.is_in_prefer_selection_mode();
        self.modes.gui_command_pending = ui.gui_command().is_some();
        // `getPickTypesForCurrentSelection`: only for a controllable selection
        let controllable = ui.all_selected_drawables().first().is_some_and(|&d| self.is_locally_controlled(d));
        let flame = controllable
            && ui.all_selected_drawables().iter().any(|&d| crate::logic::world::object_weapons::has_weapon_to_deal_damage_type(&self.world, d, DAMAGE_FLAME));
        self.view.pick_kinds = pick_kinds(ui, flame);
    }

    fn push(&mut self, t: MessageType, args: &[CmdArg]) {
        self.sent.push(t);
        self.world.push_command(t, self.player, |m| {
            for a in args {
                match *a {
                    CmdArg::Int(i) => m.append_integer_argument(i),
                    CmdArg::Real(r) => m.append_real_argument(r),
                    CmdArg::Bool(b) => m.append_boolean_argument(b),
                    CmdArg::Object(o) => m.append_object_id_argument(o as u32),
                    CmdArg::Location(l) => m.append_location_argument(l),
                }
            }
        });
    }

    fn upgrade_mask(&self, upgrade: u32) -> Option<crate::common::upgrade::UpgradeMaskType> {
        self.world.statics.upgrade_center.find_upgrade_by_key(NameKeyType(upgrade)).map(|t| t.upgrade_mask())
    }
}

impl SelectionUiHost for WorldClient {
    fn logic_frame(&self) -> u32 {
        self.world.frame()
    }
    fn append_message_bool(&mut self, t: MessageType, b: bool) {
        self.push(t, &[CmdArg::Bool(b)]);
    }
    fn on_selection_changed(&mut self, _draw: DrawableId, _selected: bool) {}
}

impl SelectionInfoHost for WorldClient {
    fn drawable_object(&self, draw: DrawableId) -> Option<ObjectId> {
        self.world.find_object(draw).map(|_| draw)
    }
    fn is_locally_controlled(&self, obj: ObjectId) -> bool {
        self.world.get_controlling_player(obj) == Some(self.player)
    }
    fn is_kind_of(&self, obj: ObjectId, k: KindOfType) -> bool {
        self.world.find_object(obj).is_some_and(|o| o.is_kind_of(k))
    }
    fn relationship_to_local_player(&self, obj: ObjectId) -> Relationship {
        let team = self.world.get_team(obj);
        self.world.tq().player_relationship_to_team(self.player, team)
    }
    fn can_local_player_garrison(&mut self, obj: ObjectId) -> bool {
        crate::logic::action_manager::can_player_garrison(&mut self.world, Some(self.player), Some(obj), CommandSourceType::FromPlayer).unwrap_or(false)
    }
    fn force_fire_or_move(&self) -> bool {
        self.modes.force_attack || self.modes.force_move
    }
    fn prefer_selection(&self) -> bool {
        self.modes.prefer_selection
    }
    fn use_alternate_mouse(&self) -> bool {
        self.modes.alternate_mouse
    }
}

impl SelectionXlatHost for WorldClient {
    fn quit_menu_visible(&self) -> bool {
        false
    }
    fn gui_command_pending(&self) -> bool {
        self.modes.gui_command_pending
    }
    fn matches_pick_kinds(&self, draw: DrawableId) -> bool {
        self.view.pick_kinds.iter().any(|&k| self.is_kind_of(draw, k))
    }
    fn is_selectable(&self, draw: DrawableId) -> bool {
        self.world.is_selectable(draw)
    }
    fn contained_by(&self, obj: ObjectId) -> Option<ObjectId> {
        self.world.find_object(obj).map(|o| o.contained_by).filter(|&c| c != INVALID_ID)
    }
    fn is_enclosing_container_for(&mut self, container: ObjectId, obj: ObjectId) -> Option<bool> {
        let c = self.world.find_object(container)?.contain?;
        Some(self.world.module_call(container, c, ModuleCall::IsEnclosingContainerFor(obj)).as_bool())
    }
    fn object_drawable(&self, obj: ObjectId) -> Option<DrawableId> {
        self.world.find_object(obj).map(|_| obj)
    }
    fn fogged_for_local_player(&self, obj: ObjectId) -> bool {
        // (`ObjectShroudStatus`'s order: clear, partial clear, fogged, shrouded)
        let s = self.world.get_shrouded_status(obj, self.player);
        matches!(s, ObjectShroudStatus::Fogged | ObjectShroudStatus::Shrouded)
    }
    fn stealthed_and_undetected(&self, obj: ObjectId) -> bool {
        self.world.find_object(obj).is_some_and(|o| o.test_status(OBJECT_STATUS_STEALTHED) && !o.test_status(OBJECT_STATUS_DETECTED))
    }
    fn append_message(&mut self, t: MessageType, create: Option<bool>, objects: &[ObjectId]) {
        let mut args: Vec<CmdArg> = create.map(CmdArg::Bool).into_iter().collect();
        args.extend(objects.iter().map(|&o| CmdArg::Object(o)));
        self.push(t, &args);
    }
    fn record_drag_selection(&mut self) {}
    fn clear_attack_move_to_mode(&mut self) {}
}

impl SelectionGroupsHost for WorldClient {
    fn all_drawables(&self) -> Vec<DrawableId> {
        self.world.objects.ids_in_order()
    }
    fn drawables_on_screen(&self) -> Vec<DrawableId> {
        self.view.on_screen.clone()
    }
    fn pick_selectable_at_click(&self) -> Option<DrawableId> {
        self.view.picked_at_click
    }
    fn is_mass_selectable(&self, obj: ObjectId) -> bool {
        self.world.is_mass_selectable(obj)
    }
    fn object_template(&self, obj: ObjectId) -> u32 {
        self.world.find_object(obj).map_or(0, |o| u32::from(o.template().get_template_id()))
    }
    fn is_equivalent_to(&self, obj: ObjectId, template: u32) -> bool {
        self.object_template(obj) == template
    }
    fn is_car_bomb(&self, obj: ObjectId) -> bool {
        self.world.find_object(obj).is_some_and(|o| o.test_status(OBJECT_STATUS_IS_CARBOMB))
    }
    fn is_off_map(&self, obj: ObjectId) -> bool {
        self.world.find_object(obj).is_some_and(|o| o.is_off_map())
    }
    fn hotkey_squad_live_objects(&mut self, group: i32) -> Option<Vec<ObjectId>> {
        use crate::logic::world::player_squads::{player_get_hotkey_squad, squad_get_live_objects};
        let mut squad = player_get_hotkey_squad(&self.world, self.player, group).ok().flatten()?;
        Some(squad_get_live_objects(&self.world, &mut squad))
    }
    fn look_at(&mut self, obj: ObjectId) {
        self.looks.push(obj);
    }
    fn ui_message(&mut self, label: &[u8], _count: Option<i32>) {
        self.ui_messages.push(label.to_vec());
    }
    fn append_message_drawable(&mut self, t: MessageType, draw: DrawableId) {
        self.sent.push(t);
        self.world.push_command(t, self.player, |m| m.append_drawable_id_argument(draw as u32));
    }
}

impl CommandXlatHost for WorldClient {
    fn object_is_masked(&self, obj: ObjectId) -> bool {
        self.world.find_object(obj).is_some_and(|o| o.test_status(OBJECT_STATUS_MASKED))
    }
    fn object_position(&self, obj: ObjectId) -> Coord3D {
        self.world.find_object(obj).map(|o| o.get_position()).unwrap_or_default()
    }
    fn object_relationship(&self, src: ObjectId, target: ObjectId) -> Relationship {
        self.world.get_relationship(src, target)
    }
    fn object_can_do_action(&mut self, src: Option<ObjectId>, action: ActionType, target: Option<ObjectId>, additional: bool) -> bool {
        crate::client::command_world::object_can_do_action(&mut self.world, src, action, target, additional)
    }
    fn object_can_attack(&mut self, src: Option<ObjectId>, target: Option<ObjectId>, forced: bool) -> CanAttackResultValue {
        crate::client::command_world::object_can_attack(&mut self.world, src, target, forced)
    }
    fn object_can_force_attack(&mut self, src: ObjectId, victim: Option<ObjectId>, pos: Option<Coord3D>) -> CanAttackResultValue {
        crate::client::command_world::object_can_force_attack(&mut self.world, src, victim, pos)
    }
    fn object_can_override_special_power_destination(&mut self, src: Option<ObjectId>, loc: &Coord3D, sp: SpecialPowerType) -> bool {
        crate::client::command_world::object_can_override_special_power_destination(&mut self.world, src, loc, sp)
    }
    fn is_salvage_crate(&mut self, obj: ObjectId) -> bool {
        self.world.is_salvage_crate(obj)
    }
    fn shroud_clear_for_local_player(&mut self, pos: &Coord3D) -> bool {
        self.world.cell_shroud_status_for_player(self.player, pos) == CellShroudStatus::Clear
    }
    fn object_has_ai(&self, obj: ObjectId) -> bool {
        self.world.find_object(obj).is_some_and(|o| o.ai.is_some())
    }
    fn quick_path_available(&mut self, obj: ObjectId, pos: &Coord3D) -> bool {
        ba::BuildAssistantHost::ai_is_quick_path_available(&mut self.world, obj, *pos)
    }
    fn has_cliff_locomotor(&mut self, obj: ObjectId) -> bool {
        let call = ModuleCall::AiHasLocomotorForSurface(crate::logic::locomotor::LOCOMOTORSURFACE_CLIFF as u32);
        matches!(self.world.ai_ask(obj, call), Some(ModuleReply::Bool(true)))
    }
    fn is_cliff_cell(&mut self, x: f32, y: f32) -> bool {
        crate::logic::ai::pathfinder::PathfinderHost::is_cliff_cell(&self.world, x, y)
    }
    fn special_power_buttons(&mut self, obj: ObjectId) -> Vec<GuiButton> {
        crate::client::command_world::special_power_buttons(&self.world, obj)
    }
    fn object_can_do_special_power(&mut self, src: Option<ObjectId>, button: &GuiButton, mode: TargetMode) -> bool {
        crate::client::command_world::object_can_do_special_power(&mut self.world, src, button, mode)
    }
    fn object_can_fire_weapon(&mut self, src: Option<ObjectId>, button: &GuiButton, mode: TargetMode) -> bool {
        crate::client::command_world::object_can_fire_weapon(&mut self.world, src, button, mode)
    }
    fn most_ready_shortcut_special_power(&mut self, sp: SpecialPowerType) -> Option<ObjectId> {
        crate::logic::world::player_objects::player_find_most_ready_shortcut_special_power_of_type(&mut self.world, self.player, sp).ok().flatten()
    }
    fn has_overridable_destination_special_power(&mut self, obj: ObjectId) -> bool {
        self.world.find_special_power_with_overridable_destination(obj).is_some()
    }
    fn append_command(&mut self, t: MessageType, args: &[CmdArg]) {
        self.push(t, args);
    }
    fn append_hint(&mut self, t: MessageType, args: &[CmdArg]) {
        self.hints.push((t, args.to_vec()));
    }
}

impl ControlBarHost for WorldClient {
    fn can_make_unit(&mut self, builder: Option<ObjectId>, what: Option<u16>) -> CanMakeType {
        match (builder, what) {
            (Some(b), Some(t)) => ba::build_assistant_can_make_unit(&mut self.world, b, t.into()).unwrap_or(CanMakeType::NoPrereq),
            // (upstream dereferences both)
            _ => CanMakeType::NoPrereq,
        }
    }
    fn eva_insufficient_funds(&mut self) {
        self.eva_insufficient_funds += 1;
    }
    fn request_unique_unit_id(&mut self, factory: ObjectId) -> Option<u32> {
        let pu = self.world.get_production_update_interface(factory)?;
        Some(self.world.module_call(factory, pu, ModuleCall::ProductionRequestUniqueUnitId).as_int(0) as u32)
    }
    fn can_afford_upgrade(&mut self, upgrade: u32) -> bool {
        let cost = self.world.statics.upgrade_center.find_upgrade_by_key(NameKeyType(upgrade)).map(|t| t.cost_to_build());
        let affordable = cost.is_some_and(|c| self.world.upgrade_center_can_afford_upgrade(Some(self.player), c));
        if !affordable {
            // (`canAffordUpgrade(..., TRUE)` says so)
            self.ui_messages.push(b"GUI:NotEnoughMoneyToUpgrade".to_vec());
        }
        affordable
    }
    fn can_queue_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> Option<CanMakeType> {
        let pu = self.world.get_production_update_interface(obj)?;
        let r = self.world.module_call(obj, pu, ModuleCall::ProductionCanQueueUpgrade { upgrade: Some(upgrade) }).as_int(0);
        Some(if r == CanMakeType::QueueFull as i32 { CanMakeType::QueueFull } else { CanMakeType::Ok })
    }
    fn has_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool {
        self.upgrade_mask(upgrade).is_some_and(|m| self.world.has_upgrade(obj, &m))
    }
    fn affected_by_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool {
        match self.upgrade_mask(upgrade) {
            Some(m) => self.world.affected_by_upgrade(obj, &m),
            None => false,
        }
    }
    fn mark_single_use_command_used(&mut self, _obj: ObjectId) {
        // **A difference from upstream:** `Object::markSingleUseCommandUsed` writes the logic object's flag from the client
        // (it is saved with the object, `object_xfer`). This client writes the World only through command messages
        // (AGENTS.md), so the flag is not set and a single-use button can be pressed again. Upstream's only user is the
        // nuke convoy truck's detonate button, which "currently" has the option off (ControlBarCommandProcessing.cpp).
    }
    fn set_special_power_construction_button(&mut self, _obj: ObjectId, _button: usize) {
        // (the button reaches the logic with the placement's `MSG_DO_SPECIAL_POWER_AT_LOCATION`; see the module docs)
    }
    fn local_player_objects(&mut self) -> Vec<ObjectId> {
        let p = self.player;
        self.world.objects.ids_in_order().into_iter().filter(|&o| self.world.get_controlling_player(o) == Some(p)).collect()
    }
    fn object_exists(&self, obj: ObjectId) -> bool {
        self.world.find_object(obj).is_some()
    }
    fn has_science(&mut self, st: i32) -> bool {
        crate::logic::world::player_ops::player_has_science(&self.world, self.player, ScienceType(st)).unwrap_or(false)
    }
    fn has_prereqs_for_science(&mut self, st: i32) -> bool {
        crate::logic::world::player_ops::player_has_prereqs_for_science(&self.world, self.player, ScienceType(st))
    }
    fn science_purchase_cost(&mut self, st: i32) -> i32 {
        crate::logic::world::player_ops::PlayerHost::science_purchase_cost(&self.world, ScienceType(st))
    }
    fn science_purchase_points(&mut self) -> i32 {
        self.world.players.get_nth(self.player).map_or(0, |p| p.science_purchase_points)
    }
    fn switch_to_context_none(&mut self) {}
    fn placement_view_angle(&self, build: Option<u16>) -> f32 {
        build.and_then(|t| self.world.statics.thing_factory.find_by_template_id(t)).map_or(0.0, |t| t.data.get_placement_view_angle())
    }
}

impl PlaceHost for WorldClient {
    fn screen_to_terrain(&mut self, pixel: ICoord2D) -> Coord3D {
        self.view.terrain.iter().find(|(p, _)| *p == pixel).map_or(Coord3D { x: pixel.x as f32, y: pixel.y as f32, z: 0.0 }, |(_, c)| *c)
    }
    fn is_line_build_template(&self, build: u16) -> bool {
        ba::build_assistant_is_line_build_template(&self.world, build.into())
    }
    fn is_location_legal_to_build(&mut self, world: Coord3D, build: u16, angle: f32, builder: Option<ObjectId>) -> LegalBuildCode {
        let flags = ba::USE_QUICK_PATHFIND
            | ba::TERRAIN_RESTRICTIONS
            | ba::CLEAR_PATH
            | ba::NO_OBJECT_OVERLAP
            | ba::SHROUD_REVEALED
            | ba::IGNORE_STEALTHED
            | ba::FAIL_STEALTHED_WITHOUT_FEEDBACK;
        ba::build_assistant_is_location_legal_to_build(&mut self.world, world, build.into(), angle, flags, builder, None)
    }
    fn special_power_construction_button(&mut self, _builder: ObjectId) -> Option<GuiButton> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::command_xlat::{CommandEvaluateType, CommandTranslator};
    use crate::logic::world::gate_scenario::{gate_world, run_gate_scenario};

    /// C2d (#6), the rule of `AGENTS.md`: the client's questions never change the simulation. Predicted: in the gate game,
    /// player 1 hovering every frame (a mouse-over hint on every object and on four ground points, in normal and in force
    /// attack mode, and the selection translator's `EVALUATE_ONLY` on every object), with a selection of player 1's objects
    /// kept on the client only, sends nothing and leaves the logic's CRCs equal, frame by frame for 300 frames, to the
    /// plain gate run's. The questions go through the action manager, the weapon checks (which may touch weapon status
    /// cells), the quick path search and the shroud; if any of them writes the World, a CRC differs.
    #[test]
    fn the_clients_questions_leave_the_crcs_alone() {
        let frames = 300;
        let plain = run_gate_scenario(frames);
        let mut c = WorldClient::new(gate_world(), 1);
        let (cmd, mut ui) = (CommandTranslator { team_exists: true }, InGameUiSelection::new());
        let ground = [(60.0, 60.0), (200.0, 200.0), (360.0, 100.0), (390.0, 390.0)];
        let mut asked = 0u64;
        let mut crcs = Vec::new();
        for f in 0..frames {
            c.world.logic_update().expect("a frame");
            if f % 50 == 0 {
                // (the client's selection only: no message reaches the logic)
                let mine: Vec<ObjectId> = c.local_player_objects();
                for &o in mine.iter().take(4) {
                    if !ui.is_selected(o) {
                        ui.select_drawable(o, &mut c);
                    }
                }
            }
            for force in [false, true] {
                ui.set_force_attack_mode(force);
                c.sync_modes(&ui);
                for o in c.world.objects.ids_in_order() {
                    let at = c.object_position(o);
                    cmd.mouse_over(Some(o), &at, &mut ui, &mut c);
                    cmd.evaluate_context_command(Some(o), &at, CommandEvaluateType::EvaluateOnly, &mut ui, &mut c);
                    asked += 2;
                }
                for (x, y) in ground {
                    cmd.mouse_over(None, &Coord3D { x, y, z: 0.0 }, &mut ui, &mut c);
                    asked += 1;
                }
            }
            crcs.push(c.world.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).expect("the CRC"));
        }
        eprintln!("C2d: {asked} questions over {frames} frames, {} hints, {} sent; selected {:?}", c.hints.len(), c.sent.len(), ui.all_selected_drawables());
        assert!(c.sent.is_empty(), "a question sent a message: {:?}", c.sent);
        assert!(!ui.all_selected_drawables().is_empty());
        let first = crcs.iter().zip(&plain).position(|(a, b)| a != b);
        assert_eq!(first, None, "the CRCs part at frame {first:?}");
        assert_eq!(crcs.len(), plain.len());
    }
}
