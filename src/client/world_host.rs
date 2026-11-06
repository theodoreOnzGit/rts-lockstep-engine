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
//! **The control bar's questions (#33).** Every question `getCommandAvailability` asks is answered from the World: the
//! AI (`isMoving`, the dozer's pending build task, the hack internet state), the weapons, the contain, the dock, the
//! special power modules, special ability and battle plan updates, overcharge, the player's upgrades in production and
//! its template's purchase science sets. Most are the modules' own getters (a module call that only reads). Two of
//! upstream's getters write, and are read here without the write: `Weapon::getStatus()` refreshes `m_status` (saved, not
//! CRC'd: `Weapon::peek_status`), and a shared special power's `isReady()`/`getPercentReady()` start the player's timer
//! (`getOrStartSpecialPowerReadyFrame`, saved: `special_power_body::peek_ready_and_percent`).
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
    /// `TheInGameUI->getAllSelectedDrawables()`, as their objects (#33: the switch weapon buttons ask for their current weapons).
    pub selection: Vec<ObjectId>,
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
    /// #51: the unit voices the translators played (`addAudioEvent`), for the client's audio to take.
    pub voices: Vec<crate::client::unit_voice::UnitVoice>,
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
            voices: Vec::new(),
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
        self.view.selection = ui.all_selected_drawables().to_vec();
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

/// #51: the unit voices' reads. Each is a getter of the World: the object's template, the AI module's `isMoving` (the module's
/// own answer, as the control bar's [`ControlBarStateHost::is_moving`](crate::client::control_bar_command::ControlBarStateHost))
/// and its `isWaitingForPath` flag (read where the AI lies, `World::ai_peek`), `canCrushOrSquish` (a `&self` read of both
/// objects), the weapon set's weapons and the store's templates, the special power module's data (`getInitiateSound`, a
/// module call that only reads), `isUsingAirborneLocomotor`, the player's completed upgrades, and the logic's copy of the
/// audio tables (`GameStatics::audio`, read from the same files as the client's). None writes.
impl crate::client::unit_voice::UnitVoiceHost for WorldClient {
    fn voice_template_name(&self, obj: ObjectId) -> Vec<u8> {
        self.object(obj).map_or_else(Vec::new, |o| o.template().get_name().to_vec())
    }
    fn template_audio(&self, obj: ObjectId, t: crate::common::thing_template::ThingTemplateAudioType) -> Vec<u8> {
        self.object(obj).map_or_else(Vec::new, |o| o.template().get_audio(t).event_name.clone())
    }
    fn template_per_unit_sound(&self, obj: ObjectId, name: &[u8]) -> Vec<u8> {
        self.object(obj).map_or_else(Vec::new, |o| o.template().get_per_unit_sound(name).event_name.clone())
    }
    fn ai_effectively_moving(&mut self, obj: ObjectId) -> Option<bool> {
        self.object(obj)?.ai?;
        let moving = self.world.ai_ask(obj, ModuleCall::AiIsMoving).is_some_and(|r| r.as_bool());
        let waiting = self.world.ai_peek(obj).is_some_and(|a| a.is_waiting_for_path());
        Some(moving || waiting)
    }
    fn can_crush_or_squish(&self, obj: ObjectId, target: ObjectId) -> bool {
        use crate::logic::world::object_damage::CrushSquishTestType;
        self.world.can_crush_or_squish(obj, Some(target), CrushSquishTestType::CrushOrSquish)
    }
    fn is_valid_audio_event(&self, name: &[u8]) -> bool {
        !name.is_empty() && self.world.statics.audio.find_audio_event_info(name).is_some()
    }
    fn has_worker_shoes(&self, obj: ObjectId) -> bool {
        // `TheUpgradeCenter->findUpgrade("Upgrade_GLAWorkerShoes")`, by name without making a name key (a missing upgrade is
        // `NULL` upstream, which `hasUpgradeComplete` dereferences: taken as not complete)
        let Some(p) = self.world.get_controlling_player(obj) else { return false };
        let Some(u) = self.world.statics.upgrade_center.templates().find(|u| u.upgrade_name() == b"Upgrade_GLAWorkerShoes") else { return false };
        self.world.player_has_upgrade_complete(p, &u.upgrade_mask())
    }
    fn voice_weapon(&self, obj: ObjectId, slot: Option<i32>) -> Option<crate::client::unit_voice::VoiceWeapon> {
        let set = &self.object(obj)?.weapon_set;
        let w = match slot {
            Some(s) => set.get_weapon_in_weapon_slot(s)?,
            // `getCurrentWeapon()`: `NULL` without any weapon
            None if !set.has_any_weapon() => return None,
            None => set.get_cur_weapon()?,
        };
        let t = self.world.statics.weapon_store.templates().get(w.template())?;
        Some(crate::client::unit_voice::VoiceWeapon { damage_type: t.damage_type, slot: w.weapon_slot(), name: t.name.clone() })
    }
    fn special_power_initiate_sound(&mut self, obj: ObjectId, sp: SpecialPowerType) -> Option<Vec<u8>> {
        let idx = self.world.find_special_power_module_interface(obj, sp)?;
        match self.world.module_call(obj, idx, ModuleCall::GetSpecialPowerInitiateSound) {
            crate::logic::world::module_call::ModuleReply::Sound(s) => Some(s.unwrap_or_default()),
            _ => Some(Vec::new()),
        }
    }
    fn is_using_airborne_locomotor(&self, obj: ObjectId) -> bool {
        self.world.is_using_airborne_locomotor(obj)
    }
    fn terrorist_voice(&self, which: crate::client::unit_voice::TerroristVoice) -> Vec<u8> {
        use crate::client::unit_voice::TerroristVoice;
        let m = &self.world.statics.audio.misc;
        match which {
            TerroristVoice::Attack => m.terrorist_in_car_attack_voice.event_name.clone(),
            TerroristVoice::Move => m.terrorist_in_car_move_voice.event_name.clone(),
            TerroristVoice::Select => m.terrorist_in_car_select_voice.event_name.clone(),
        }
    }
    fn play_unit_voice(&mut self, v: crate::client::unit_voice::UnitVoice) {
        self.voices.push(v);
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

// --- #12: the control bar's questions (client::control_bar_command), answered from the World, read only ---

impl WorldClient {
    /// The production queue of `obj` for the build queue window: (a unit?, its production id or upgrade key, its button
    /// image's name, its percent complete), in order; empty without a production update.
    pub fn production_queue(&self, obj: ObjectId) -> Vec<(bool, u32, Option<Vec<u8>>, f32)> {
        use crate::logic::modules::production_update::PRODUCTION_UNIT;
        use crate::logic::world::module_call::ModuleHandle;
        let Some(x) = self.world.find_object(obj) else { return Vec::new() };
        let n = x.modules.len();
        for i in 0..n {
            if let Some(crate::logic::world::update_body::UpdateBody::ProductionUpdate(p)) = self.world.peek_module_body(ModuleHandle { object: obj, index: i as _ }) {
                return p
                    .queue()
                    .iter()
                    .map(|e| {
                        let unit = e.production_type == PRODUCTION_UNIT;
                        let (id, image) = if unit {
                            let image = e.object_to_produce.and_then(|t| self.world.statics.thing_factory.find_by_template_id(t)).map(|t| t.data.button_image_name().to_vec());
                            (e.production_id as u32, image.filter(|n| !n.is_empty()))
                        } else {
                            let key = e.upgrade_to_research.map_or(0, |k| k.0);
                            let image = self.world.statics.upgrade_center.find_upgrade_by_key(NameKeyType(key)).map(|u| u.button_image_name().to_vec());
                            (key, image.filter(|n| !n.is_empty()))
                        };
                        (unit, id, image, e.percent_complete)
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    /// The local player's money and (production, consumption) of power.
    pub fn money_and_power(&self) -> (u32, (i32, i32)) {
        let frame = self.world.frame();
        self.world.players.get_nth(self.player).map_or((0, (0, 0)), |p| (p.money.count_money(), (p.energy.production(frame), p.energy.consumption())))
    }

    fn object(&self, obj: ObjectId) -> Option<&crate::logic::world::object::Object> {
        self.world.find_object(obj)
    }
}

impl crate::client::control_bar_command::ControlBarStateHost for WorldClient {
    fn command_set_buttons(&self, obj: ObjectId) -> Option<[Option<usize>; crate::logic::command_button::MAX_COMMANDS_PER_SET]> {
        let w = &self.world;
        let set = w.control_bar_find_command_set(&w.object_get_command_set_string(obj))?;
        Some(std::array::from_fn(|i| w.command_set_get_command_button(set, i)))
    }
    fn button(&self, index: usize) -> Option<crate::logic::command_button::CommandButton> {
        let f = self.world.cuts.command_buttons.get(index)?;
        if let Some(b) = self.world.statics.command_buttons.find_command_button(&f.name) {
            return Some(b.clone());
        }
        // (a button the World has only as facts, as the tests' and the demo's: no images, no labels)
        let mut b = crate::logic::command_button::CommandButton::new(&f.name);
        b.command = f.command_type;
        b.options = f.options;
        b.weapon_slot = f.weapon_slot;
        b.max_shots_to_fire = f.max_shots_to_fire;
        b.special_power = f.special_power;
        b.thing_template = f.thing_template;
        b.upgrade = f.upgrade.map(NameKeyType);
        Some(b)
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        use crate::common::science::GameTextFetch;
        self.world.statics.game_text.fetch(label).str().to_vec()
    }
    fn shortcut_source(&mut self, button: &crate::logic::command_button::CommandButton) -> Option<ObjectId> {
        let sp = button.special_power.and_then(|id| self.world.statics.special_power_store.find_special_power_template_by_id(id).map(|t| t.special_power_type()))?;
        self.most_ready_shortcut_special_power(sp)
    }
    fn script_disabled_or_unpowered(&self, obj: ObjectId) -> bool {
        use crate::logic::object_script_status_bits::{OBJECT_STATUS_SCRIPT_DISABLED, OBJECT_STATUS_SCRIPT_UNPOWERED};
        self.object(obj).is_some_and(|o| o.test_script_status_bit(OBJECT_STATUS_SCRIPT_DISABLED) || o.test_script_status_bit(OBJECT_STATUS_SCRIPT_UNPOWERED))
    }
    fn disabled_unmanned(&self, obj: ObjectId) -> bool {
        self.object(obj).is_some_and(|o| o.is_disabled_by_type(crate::common::disabled_types::DISABLED_UNMANNED))
    }
    fn disabled_subdued(&self, obj: ObjectId) -> bool {
        self.object(obj).is_some_and(|o| o.is_disabled_by_type(crate::common::disabled_types::DISABLED_SUBDUED))
    }
    fn single_use_used(&self, _obj: ObjectId) -> bool {
        // (the flag is never set by this client: see `mark_single_use_command_used`)
        false
    }
    fn is_moving(&mut self, obj: ObjectId) -> bool {
        // ~~**Not answered yet (#12 gap):** the AI's `isMoving` needs the AI host; buttons with `MUST_BE_STOPPED` show as
        // available while moving.~~ **Corrected 2026-11-06 (#33):** `getAI()->isMoving()`, the AI module's own answer (it reads
        // the AI and its machine; `isIdle` of a Chinook also asks its contain, a getter).
        self.world.ai_ask(obj, ModuleCall::AiIsMoving).is_some_and(|r| r.as_bool())
    }
    fn disabled(&self, obj: ObjectId) -> (bool, bool) {
        self.object(obj).map_or((false, false), |o| {
            let only = o.is_disabled_by_type(crate::common::disabled_types::DISABLED_UNDERPOWERED) && o.disabled.count() == 1;
            (o.is_disabled(), only)
        })
    }
    fn upgrade_is_player(&self, upgrade: u32) -> Option<bool> {
        self.world.statics.upgrade_center.find_upgrade_by_key(NameKeyType(upgrade)).map(|u| u.upgrade_type() == crate::common::upgrade::UpgradeType::Player)
    }
    fn player_has_upgrade_complete(&self, obj: ObjectId, upgrade: u32) -> bool {
        let p = self.world.get_controlling_player(obj).unwrap_or(self.player);
        self.upgrade_mask(upgrade).is_some_and(|m| self.world.player_has_upgrade_complete(p, &m))
    }
    fn player_has_upgrade_in_production(&self, obj: ObjectId, upgrade: u32) -> bool {
        // ~~**Not answered yet (#12 gap):** the player's upgrade list's in-production state.~~ **Corrected 2026-11-06 (#33):**
        // `player->hasUpgradeInProduction(u)` of the object's controlling player, the World's read of the player's upgrade list.
        let p = self.world.get_controlling_player(obj).unwrap_or(self.player);
        self.upgrade_mask(upgrade).is_some_and(|m| self.world.player_has_upgrade_in_production(p, &m))
    }
    fn obj_has_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool {
        ControlBarHost::has_upgrade(self, obj, upgrade)
    }
    fn affected_by_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool {
        ControlBarHost::affected_by_upgrade(self, obj, upgrade)
    }
    fn production_count(&mut self, obj: ObjectId) -> Option<i32> {
        let pu = self.world.get_production_update_interface(obj)?;
        Some(self.world.module_call(obj, pu, ModuleCall::ProductionGetCount).as_int(0))
    }
    fn is_upgrade_in_queue(&mut self, obj: ObjectId, upgrade: u32) -> bool {
        let Some(pu) = self.world.get_production_update_interface(obj) else { return false };
        self.world.module_call(obj, pu, ModuleCall::ProductionIsUpgradeInQueue { upgrade: Some(upgrade) }).as_int(0) != 0
    }
    fn buildable(&self, template: u16) -> crate::client::control_bar_command::Buildable {
        use crate::client::control_bar_command::Buildable;
        match self.world.statics.thing_factory.find_by_template_id(template).map(|t| t.data.buildable()) {
            Some(1) => Buildable::IgnorePrerequisites,
            Some(2) => Buildable::No,
            Some(3) => Buildable::OnlyByAi,
            _ => Buildable::Yes,
        }
    }
    fn player_is_computer(&self, obj: ObjectId) -> bool {
        self.world.get_controlling_player(obj).and_then(|p| self.world.players.get_nth(p)).is_some_and(|p| p.player_type == crate::common::game_common::PlayerType::Computer)
    }
    fn is_dozer(&self, obj: ObjectId) -> bool {
        self.object(obj).is_some_and(|o| o.is_kind_of(crate::common::kind_of::KINDOF_DOZER))
    }
    fn dozer_build_pending(&mut self, obj: ObjectId) -> Option<bool> {
        // ~~**Not answered yet (#12 gap):** the dozer AI's pending build task; the dozer is taken as idle.~~ **Corrected
        // 2026-11-06 (#33):** `getDozerAIInterface()->isTaskPending(DOZER_TASK_BUILD)`, read from the AI where it lies
        // (`World::ai_peek`): `m_task[DOZER_TASK_BUILD].m_targetObjectID != INVALID_ID`. `None` without a dozer AI.
        use crate::logic::ai::ai_update::AiSubclass;
        use crate::logic::ai::dozer_ai_update::DOZER_TASK_BUILD;
        let m = match &self.world.ai_peek(obj)?.subclass {
            AiSubclass::Dozer(d) => &d.m,
            AiSubclass::Worker(w) => &w.dozer.m,
            _ => return None,
        };
        Some(m.task[DOZER_TASK_BUILD as usize].target_object_id != INVALID_ID)
    }
    fn player_can_build(&mut self, obj: ObjectId, template: Option<u16>) -> bool {
        let p = self.world.get_controlling_player(obj).unwrap_or(self.player);
        crate::logic::world::player_objects::player_can_build(&mut self.world, p, template.map(Into::into)).unwrap_or(false)
    }
    fn player_can_afford_build(&mut self, obj: ObjectId, template: Option<u16>) -> bool {
        let p = self.world.get_controlling_player(obj).unwrap_or(self.player);
        let Some(t) = template else { return false };
        crate::logic::world::player_objects::player_can_afford_build(&mut self.world, p, t.into()).unwrap_or(false)
    }
    fn can_make_unit(&mut self, obj: ObjectId, template: Option<u16>) -> CanMakeType {
        ControlBarHost::can_make_unit(self, Some(obj), template)
    }
    fn can_afford_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool {
        let p = self.world.get_controlling_player(obj).unwrap_or(self.player);
        let cost = self.world.statics.upgrade_center.find_upgrade_by_key(NameKeyType(upgrade)).map(|t| t.cost_to_build());
        cost.is_some_and(|c| self.world.upgrade_center_can_afford_upgrade(Some(p), c))
    }
    fn player_has_science(&mut self, _obj: ObjectId, st: i32) -> bool {
        ControlBarHost::has_science(self, st)
    }
    fn unsellable(&self, obj: ObjectId) -> bool {
        self.object(obj).is_some_and(|o| o.test_script_status_bit(crate::logic::object_script_status_bits::OBJECT_STATUS_SCRIPT_UNSELLABLE))
    }
    fn weapon(&mut self, obj: ObjectId, slot: i32) -> Option<crate::client::control_bar_command::WeaponState> {
        // ~~**Not answered yet (#12 gap):** the weapon's status; a fire weapon button shows as ready.~~ **Corrected 2026-11-06
        // (#33):** `getWeaponInWeaponSlot(slot)` and its `getClipReloadTime(obj)`, `getStatus()`, `getPossibleNextShotFrame()`
        // and `getPercentReadyToFire()`. Upstream's `getStatus()` writes `m_status` (a lazy refresh; saved, not CRC'd): the
        // status and the percent are read with `Weapon::peek_status` and `peek_percent_ready_to_fire`, the same answers
        // without the write. `getClipReloadTime` computes the bonus (the object's and its container's bonus flags, getters)
        // on a copy of the weapon, as `getLargestWeaponRange` does.
        use crate::logic::weapon_types::WeaponStatus;
        let w = self.object(obj)?.weapon_set.get_weapon_in_weapon_slot(slot)?.clone();
        let now = self.world.frame();
        let status = w.peek_status(now);
        let clip = w.get_clip_reload_time(&mut self.world, obj);
        Some(crate::client::control_bar_command::WeaponState {
            clip_reload_time: clip.max(0) as u32,
            ready_to_fire: status == WeaponStatus::ReadyToFire,
            reloading_clip: status == WeaponStatus::ReloadingClip,
            possible_next_shot_frame: w.possible_next_shot_frame(),
            percent_ready: w.peek_percent_ready_to_fire(now),
        })
    }
    fn frame(&self) -> u32 {
        self.world.frame()
    }
    fn mine_clearing_weapon_set(&self, obj: ObjectId) -> bool {
        // (#33) `testWeaponSetFlag(WEAPONSET_MINE_CLEARING_DETAIL)`
        self.object(obj).is_some_and(|o| o.test_weapon_set_flag(crate::logic::weapon_types::WEAPONSET_MINE_CLEARING_DETAIL))
    }
    fn rappeller_count(&mut self, obj: ObjectId) -> i32 {
        // ~~**Not answered yet (#12 gap):** the contained rappellers; combat drop shows as restricted.~~ **Corrected 2026-11-06
        // (#33):** `getRappellerCount(obj)`: the contain's `getContainedItemsList()` (a getter) counted by `KINDOF_CAN_RAPPEL`.
        let Some(c) = self.world.get_contain(obj) else { return 0 };
        let ModuleReply::Ids(items) = self.world.module_call(obj, c, ModuleCall::GetContainedItems) else { return 0 };
        items.iter().filter(|&&i| self.is_kind_of(i, crate::common::kind_of::KINDOF_CAN_RAPPEL)).count() as i32
    }
    fn contain_count(&mut self, obj: ObjectId) -> Option<u32> {
        // ~~**Not answered yet (#12 gap):** the contain module's count; evacuate shows as restricted.~~ **Corrected 2026-11-06
        // (#33):** `getContain()->getContainCount()`, the contain module's getter.
        let c = self.world.get_contain(obj)?;
        Some(self.world.module_call(obj, c, ModuleCall::GetContainCount).as_int(0).max(0) as u32)
    }
    fn has_ai_update(&self, obj: ObjectId) -> bool {
        self.object(obj).is_some_and(|o| o.ai.is_some())
    }
    fn dock_open(&mut self, obj: ObjectId) -> Option<bool> {
        // (#33) `getDockUpdateInterface()->isDockOpen()`, the dock's getter
        let d = self.world.get_dock_update_interface(obj)?;
        Some(self.world.module_call(obj, d, ModuleCall::DockIsOpen).as_bool())
    }
    fn special_power(&mut self, obj: ObjectId, button: &crate::logic::command_button::CommandButton) -> crate::client::control_bar_command::SpecialPowerState {
        // ~~**Not answered yet (#12 gap):** the special power module's readiness and the clock; powers show as ready.~~
        // **Corrected 2026-11-06 (#33):** `getSpecialPowerModule(template)`; its `isReady()` and `getPercentReady()`; else
        // `findSpecialAbilityUpdate(type)->isPowerCurrentlyInUse(command)`; else, for `SPECIAL_CHANGE_BATTLE_PLANS`, the
        // `BattlePlanUpdate`'s `getCommandOption() & command->getOptions()`. **A side effect avoided:** for a shared power the
        // module's `isReady` starts the player's timer (`getOrStartSpecialPowerReadyFrame`), so the readiness is read with
        // `special_power_body::peek_ready_and_percent` from the module's body where it lies.
        use crate::client::control_bar_command::SpecialPowerState;
        use crate::logic::modules::special_power_body::{peek_ready_and_percent, power_of_body};
        use crate::logic::world::module_call::ModuleHandle;
        let Some(m) = self.world.get_special_power_module(obj, button.special_power) else { return SpecialPowerState::NoModule };
        let me = ModuleHandle { object: obj, index: m };
        let read = self.world.peek_module_body(me).and_then(power_of_body).map(|p| peek_ready_and_percent(p, &self.world, me));
        // (a module that is not one of the game's special powers, or a `NULL` template upstream would crash on: no clock)
        let Some(Ok((ready, percent))) = read else { return SpecialPowerState::NoModule };
        if !ready {
            return SpecialPowerState::Charging(percent);
        }
        let sp_type = button.special_power.and_then(|id| self.world.statics.special_power_store.find_special_power_template_by_id(id)).map(|t| t.special_power_type());
        if let Some(t) = sp_type {
            if let Some(u) = self.world.find_special_ability_update(obj, t) {
                let in_use = self.world.module_call(obj, u, ModuleCall::IsPowerCurrentlyInUse).as_bool();
                return SpecialPowerState::Ready { ability_in_use: Some(in_use), battle_plan_option_match: false };
            }
        }
        if sp_type == Some(crate::common::special_power_type::SPECIAL_CHANGE_BATTLE_PLANS) {
            if let Some(b) = self.world.find_module(obj, b"BattlePlanUpdate") {
                let option = self.world.module_call(obj, b, ModuleCall::SpecialPowerUpdateGetCommandOption).as_int(0) as u32;
                return SpecialPowerState::Ready { ability_in_use: None, battle_plan_option_match: option & button.options != 0 };
            }
        }
        SpecialPowerState::Ready { ability_in_use: None, battle_plan_option_match: false }
    }
    fn overcharge_active(&mut self, obj: ObjectId) -> bool {
        // (#33) each behavior module's `getOverchargeBehaviorInterface()->isOverchargeActive()`
        crate::client::control_bar_popup::PopupHost::overcharge_interfaces(self, obj).into_iter().any(|a| a)
    }
    fn selected_current_weapon_slots(&mut self) -> Vec<i32> {
        // (#33) the selected drawables' objects that are locally controlled and have a current weapon: its slot
        use crate::logic::world::object_weapons::get_current_weapon;
        let sel = self.view.selection.clone();
        sel.into_iter()
            .filter(|&o| self.is_locally_controlled(o))
            .filter_map(|o| {
                let c = get_current_weapon(&self.world, o);
                c.slot.filter(|_| c.has_weapon)
            })
            .collect()
    }
    fn hacking_packing_or_unpacking(&mut self, obj: ObjectId) -> bool {
        // (#33) `getHackInternetAIInterface()->isHackingPackingOrUnpacking()`: the AI machine's state is one of the three
        use crate::logic::ai::hack_internet_ai_update::{HACK_INTERNET, PACKING, UNPACKING};
        if !self.world.ai_peek(obj).is_some_and(|a| matches!(a.subclass, crate::logic::ai::ai_update::AiSubclass::HackInternet(_))) {
            return false;
        }
        match self.world.ai_ask(obj, ModuleCall::AiGetAiStateType) {
            Some(ModuleReply::Int(s)) => [UNPACKING, HACK_INTERNET, PACKING].contains(&(s as _)),
            _ => false,
        }
    }
    fn battle_plan_is_bombardment(&mut self, obj: ObjectId) -> Option<bool> {
        // (#33) `findUpdateModule(BattlePlanUpdate)->getActiveBattlePlan() == PLANSTATUS_BOMBARDMENT`
        let b = self.world.find_module(obj, b"BattlePlanUpdate")?;
        let plan = self.world.module_call(obj, b, ModuleCall::BattlePlanGetActiveBattlePlan).as_int(0);
        Some(plan == crate::logic::modules::battle_plan_update::PLANSTATUS_BOMBARDMENT)
    }
    fn purchase_science_buttons(&self, obj: ObjectId) -> Option<Vec<usize>> {
        // ~~**Not answered yet (#12 gap):** the player template's purchase science sets.~~ **Corrected 2026-11-06 (#33):** the
        // controlling player's template's `PurchaseScienceCommandSetRank1`, `3` and `8` (none empty, every set found), their
        // first `MAX_PURCHASE_SCIENCE_RANK_1` (4), `_3` (15) and `_8` (4) buttons, in that order (`populateCommand`'s loops).
        let w = &self.world;
        let p = w.get_controlling_player(obj)?;
        let t = w.players.get_nth(p)?.template?;
        let pt = w.statics.player_template_store.templates().get(t as usize)?;
        let sets = [(pt.purchase_science_command_set_rank1(), 4), (pt.purchase_science_command_set_rank3(), 15), (pt.purchase_science_command_set_rank8(), 4)];
        if sets.iter().any(|(n, _)| n.is_empty()) {
            return None;
        }
        let found: Vec<(usize, usize)> = sets.iter().map(|(n, max)| w.control_bar_find_command_set(n).map(|s| (s, *max))).collect::<Option<_>>()?;
        Some(found.into_iter().flat_map(|(set, max)| (0..max).filter_map(move |i| w.command_set_get_command_button(set, i))).collect())
    }
    fn special_power_required_science(&self, button: &crate::logic::command_button::CommandButton) -> Option<i32> {
        let t = self.world.statics.special_power_store.find_special_power_template_by_id(button.special_power?)?;
        let st = t.required_science().0;
        (st >= 0).then_some(st)
    }
}

// --- #35: the portrait and the popup description (client::control_bar_popup), answered from the World, read only ---

impl crate::client::control_bar_popup::PortraitHost for WorldClient {
    fn portrait_is_kind_of(&self, obj: ObjectId, k: KindOfType) -> bool {
        self.is_kind_of(obj, k)
    }
    fn portrait_is_locally_controlled(&self, obj: ObjectId) -> bool {
        self.is_locally_controlled(obj)
    }
    fn portrait_template(&self, obj: ObjectId) -> Option<u16> {
        self.object(obj).map(|o| o.template().get_template_id())
    }
    fn portrait_controlling_player(&self, obj: ObjectId) -> Option<i32> {
        self.world.get_controlling_player(obj)
    }
    fn portrait_disguised_enemy(&mut self, obj: ObjectId) -> Option<(u16, Option<i32>)> {
        // `draw->getStealthLook() == STEALTHLOOK_DISGUISED_ENEMY`: the look the object's `StealthUpdate` gives its drawable
        // for the local player (`calcStealthedStatusForPlayer`, read where the module lies), with the disguise's template
        use crate::logic::modules::stealth_update::stealth_look;
        use crate::logic::world::module_call::ModuleHandle;
        use crate::logic::world::update_body::UpdateBody;
        let index = self.object(obj)?.find_module(b"StealthUpdate")?;
        let Some(UpdateBody::StealthUpdate(s)) = self.world.peek_module_body(ModuleHandle { object: obj, index }) else { return None };
        if s.calc_stealthed_status_for_local_player(&self.world, obj)? != stealth_look::DISGUISED_ENEMY {
            return None;
        }
        let t = s.get_disguised_template()?.get_template_id();
        Some((t, s.is_disguised().then(|| s.get_disguised_player_index())))
    }
    fn template_is_kind_of(&self, template: u16, k: KindOfType) -> bool {
        self.world.statics.thing_factory.find_by_template_id(template).is_some_and(|t| t.data.is_kind_of(k))
    }
    fn template_selected_portrait(&self, template: u16) -> Vec<u8> {
        self.world.statics.thing_factory.find_by_template_id(template).map(|t| t.data.selected_portrait_image_name().to_vec()).unwrap_or_default()
    }
    fn template_upgrade_cameo_name(&self, template: u16, i: usize) -> Vec<u8> {
        self.world.statics.thing_factory.find_by_template_id(template).map(|t| t.data.get_upgrade_cameo_name(i).to_vec()).unwrap_or_default()
    }
    fn find_upgrade(&self, name: &[u8]) -> Option<(u32, Vec<u8>)> {
        self.world.statics.upgrade_center.templates().find(|u| u.upgrade_name() == name).map(|u| (u.upgrade_name_key().0, u.button_image_name().to_vec()))
    }
    fn portrait_obj_has_upgrade(&mut self, obj: ObjectId, upgrade: u32) -> bool {
        ControlBarHost::has_upgrade(self, obj, upgrade)
    }
    fn portrait_player_has_upgrade_complete(&self, player: i32, upgrade: u32) -> bool {
        self.upgrade_mask(upgrade).is_some_and(|m| self.world.player_has_upgrade_complete(player, &m))
    }
    fn veterancy_level(&self, obj: ObjectId) -> i32 {
        self.object(obj).map_or(0, |o| o.get_veterancy_level() as i32)
    }
}

impl crate::client::control_bar_popup::PopupHost for WorldClient {
    fn first_selected_object(&self) -> Option<ObjectId> {
        self.view.selection.first().copied().filter(|&o| self.world.find_object(o).is_some())
    }
    fn local_has_science(&mut self, st: i32) -> bool {
        ControlBarHost::has_science(self, st)
    }
    fn local_has_upgrade_in_production(&mut self, upgrade: u32) -> bool {
        self.upgrade_mask(upgrade).is_some_and(|m| self.world.player_has_upgrade_in_production(self.player, &m))
    }
    fn local_has_upgrade_complete(&mut self, upgrade: u32) -> bool {
        self.upgrade_mask(upgrade).is_some_and(|m| self.world.player_has_upgrade_complete(self.player, &m))
    }
    fn local_can_afford_upgrade(&mut self, upgrade: u32) -> bool {
        let cost = self.world.statics.upgrade_center.find_upgrade_by_key(NameKeyType(upgrade)).map(|t| t.cost_to_build());
        cost.is_some_and(|c| self.world.upgrade_center_can_afford_upgrade(Some(self.player), c))
    }
    fn thing_cost_to_build(&mut self, template: u16) -> i32 {
        self.world.template_calc_cost_to_build(template, self.player)
    }
    fn thing_is_kind_of(&self, template: u16, k: KindOfType) -> bool {
        crate::client::control_bar_popup::PortraitHost::template_is_kind_of(self, template, k)
    }
    fn thing_prereqs(&mut self, template: u16) -> Vec<(Vec<crate::client::control_bar_popup::PrereqUnit>, bool)> {
        // each prerequisite's `calcNumPrereqUnitsOwned(player, counts)` (`countObjectsByThingTemplate`) and sciences
        use crate::common::production_prerequisite::{MAX_PREREQ, UNIT_OR_WITH_PREV};
        use crate::logic::world::player_ops::{player_count_objects_by_thing_template, player_has_science};
        let w = &self.world;
        let Some(t) = w.statics.thing_factory.find_by_template_id(template) else { return Vec::new() };
        let p = self.player;
        (0..t.data.get_prereq_count().max(0) as usize)
            .filter_map(|i| t.data.get_nth_prereq(i))
            .map(|pre| {
                let units = pre.prereq_units();
                let cnt = units.len().min(MAX_PREREQ);
                let things: Vec<Option<_>> = units[..cnt].iter().map(|u| u.unit.map(Into::into)).collect();
                let mut own = [0i32; MAX_PREREQ];
                let _ = player_count_objects_by_thing_template(w, p, &things, false, &mut own[..cnt], true);
                let list = units[..cnt]
                    .iter()
                    .zip(own)
                    .map(|(u, owned)| crate::client::control_bar_popup::PrereqUnit {
                        display_name: u.unit.and_then(|id| w.statics.thing_factory.find_by_template_id(id)).map(|t| t.data.get_display_name().str().to_vec()).unwrap_or_default(),
                        or_with_prev: u.flags & UNIT_OR_WITH_PREV != 0,
                        owned,
                    })
                    .collect();
                let has_sciences = pre.prereq_sciences().iter().all(|s| player_has_science(w, p, *s).unwrap_or(false));
                (list, has_sciences)
            })
            .collect()
    }
    fn upgrade_cost_to_build(&self, upgrade: u32) -> i32 {
        self.world.statics.upgrade_center.find_upgrade_by_key(NameKeyType(upgrade)).map_or(0, |t| t.cost_to_build())
    }
    fn science_name_and_description(&self, st: i32) -> Option<(Vec<u16>, Vec<u16>)> {
        self.world.statics.science_store.get_name_and_description(ScienceType(st)).map(|(n, d)| (n.str().to_vec(), d.str().to_vec()))
    }
    fn science_cost(&self, st: i32) -> i32 {
        self.world.statics.science_store.get_science_purchase_cost(ScienceType(st))
    }
    fn overcharge_interfaces(&mut self, obj: ObjectId) -> Vec<bool> {
        let mods: Vec<_> = self.world.all_modules(obj).into_iter().filter(|&i| self.world.module_slot(obj, i).is_some_and(|m| m.name == b"OverchargeBehavior")).collect();
        mods.into_iter().map(|i| self.world.module_call(obj, i, ModuleCall::IsOverchargeActive).as_bool()).collect()
    }
    fn local_energy(&self) -> Option<(i32, i32)> {
        let frame = self.world.frame();
        self.world.players.get_nth(self.player).map(|p| (p.energy.production(frame), p.energy.consumption()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::command_xlat::{CommandEvaluateType, CommandTranslator};
    use crate::logic::world::gate_scenario::{gate_world, run_gate_scenario};

    /// #12, the rule of `AGENTS.md` for the control bar: `populateCommand`'s questions (`getCommandAvailability` for
    /// every button of every player-1 object, which asks production modules for their counts and queues, the build
    /// assistant, the upgrade center and the players), the build queue and the money and power, asked every frame, leave
    /// the logic's CRCs equal, frame by frame for 300 frames, to a run without them. Both runs give every player-1 object
    /// of the gate the same set of five buttons (unit build, dozer construct, sell, stop, an object upgrade; the World takes
    /// the control bar's data as a table). Predicted: equal CRCs (the questions only read), and five windows a frame per
    /// object, so 300 x 5 x the objects shown at most.
    ///
    /// **#33 (2026-11-06):** both runs also hold [`EXTRA33`]'s objects with their own command sets, so the questions now
    /// read the AI (`isMoving`, the dozer's task, the hack state), the weapons (a gunner in a fight: its clip, status and
    /// reload), the contain (a bus with two rappellers), the production (a barracks training a recruit and researching a
    /// player upgrade, queued in both runs), the special powers (a shared one never started, a charging one) and the
    /// selection (every player-1 object selected on the client, for the switch weapon buttons). Predicted: the CRCs stay
    /// equal; the tally of the answers shows a fire weapon clock (`NOT_READY` with a clock) at least once, evacuate and
    /// combat drop available, the drill upgrade `CANT_AFFORD` (in production) and the charger's power `NOT_READY` with a
    /// clock; the shared timer list stays empty; nothing is sent. The gunner's set also asks the questions of modules it lacks
    /// (overcharge, hack internet, a battle plan's stop, a railed transport): `AVAILABLE`, `AVAILABLE`, `AVAILABLE` and
    /// `RESTRICTED` (no dock) every frame (held: 300 of each).
    ///
    /// *Results (2026-11-06):* equal CRCs on all 300 frames; 24,300 windows shown; the fire button 58 frames with a reload
    /// clock, 13 not ready between shots, 229 available; the charger's power 179 frames with a clock, then available (its
    /// 180-frame charge); evacuate and combat drop available on the full bus and restricted on the empty one; the drill
    /// `CANT_AFFORD` throughout; the primary switch button `ACTIVE`; the timer list empty; nothing sent.
    #[test]
    fn the_control_bars_questions_leave_the_crcs_alone() {
        use crate::client::control_bar_command::CommandAvailability as A;
        use crate::logic::world::object_cuts::CommandButtonFacts;
        use crate::logic::world::object_command_button::gui as g;
        let frames = 300;
        let setup = || {
            let (mut w, x) = world33();
            let depot = w.statics.thing_factory.find_template(b"Depot").map(|t| t.data.get_template_id());
            let first = w.cuts.command_buttons.len();
            for (i, (t, tt)) in [(g::UNIT_BUILD, depot), (g::DOZER_CONSTRUCT, depot), (g::SELL, None), (g::STOP, None), (g::OBJECT_UPGRADE, None)].into_iter().enumerate() {
                w.cuts.command_buttons.push(CommandButtonFacts {
                    name: format!("Gate12Button{i}").into_bytes(),
                    command_type: t,
                    weapon_slot: 0,
                    special_power: None,
                    thing_template: tt,
                    upgrade: None,
                    ready_for: Default::default(),
                    options: 0,
                    max_shots_to_fire: 0,
                });
            }
            w.cuts.command_sets.push((b"Gate12Set".to_vec(), (first..first + 5).map(Some).collect()));
            for o in w.objects.ids_in_order() {
                if w.get_controlling_player(o) == Some(1) && w.object_get_command_set_string(o).is_empty() {
                    w.objects.find_mut(o).expect("an object").command_set_string_override = b"Gate12Set".to_vec();
                }
            }
            // (#33: the barracks trains a recruit and researches the drill, in both runs)
            let pu = w.get_production_update_interface(x.barracks).expect("the factory");
            let recruit = w.statics.thing_factory.find_template(b"Recruit").map(|t| t.data.get_template_id());
            assert!(w.module_call(x.barracks, pu, ModuleCall::ProductionQueueCreateUnit { template: recruit, production_id: 1 }).as_bool());
            let drill = w.cuts.command_buttons.iter().find(|b| b.name == b"Command_Drill").and_then(|b| b.upgrade);
            assert!(w.module_call(x.barracks, pu, ModuleCall::ProductionQueueUpgrade { upgrade: drill }).as_bool());
            (w, x)
        };
        let (mut plain, _) = setup();
        let (w, x) = setup();
        let mut c = WorldClient::new(w, 1);
        c.view.selection = c.local_player_objects();
        let mut shown = 0usize;
        let mut tally: std::collections::BTreeMap<(Vec<u8>, String), u32> = Default::default();
        for f in 0..frames {
            plain.logic_update().expect("a frame");
            c.world.logic_update().expect("a frame");
            for o in c.local_player_objects() {
                let slots = crate::client::control_bar_command::populate_command(o, &mut c);
                for s in slots.iter().flatten() {
                    shown += 1;
                    let name = c.world.cuts.command_buttons[s.button].name.clone();
                    *tally.entry((name, format!("{:?} {}", s.availability, if s.clock.is_some() { "clock" } else { "" }))).or_default() += 1;
                }
                let _ = c.production_queue(o);
            }
            let _ = c.money_and_power();
            let a = c.world.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).expect("the CRC");
            let b = plain.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).expect("the CRC");
            assert_eq!(a, b, "the CRCs part at frame {f}");
        }
        eprintln!("#12: {shown} command windows shown over {frames} frames");
        for ((n, a), k) in &tally {
            eprintln!("#33:   {:<24} {a:<20} {k}", String::from_utf8_lossy(n));
        }
        let seen = |n: &[u8], a: A, clock: bool| tally.get(&(n.to_vec(), format!("{a:?} {}", if clock { "clock" } else { "" }))).copied().unwrap_or(0);
        assert!(c.sent.is_empty());
        assert!(shown > 0);
        assert!(seen(b"Command_Fire", A::NotReady, true) > 0, "a reload clock");
        assert!(seen(b"Command_Evac", A::Available, false) > 0);
        assert!(seen(b"Command_CombatDrop", A::Available, false) > 0);
        assert!(seen(b"Command_Drill", A::CantAfford, false) > 0);
        assert!(seen(b"Command_Own", A::NotReady, true) > 0);
        for (n, a) in [(&b"Command_Overcharge"[..], A::Available), (b"Command_Hack", A::Available), (b"Command_PlanStop", A::Available), (b"Command_Rail", A::Restricted)] {
            assert_eq!(seen(n, a, false), frames, "{}", String::from_utf8_lossy(n));
        }
        let p = c.world.get_controlling_player(x.caster).expect("the player");
        assert_eq!(c.world.players.get_nth(p).map(|pl| pl.special_power_timers.len()), Some(0));
    }

    /// #51: the unit voices' questions (`pickAndPlayUnitVoiceResponse` for every message type it knows, with and without an
    /// air target, a target drawable, a weapon slot and each special power type of the game), asked every frame with every
    /// player-1 object of [`world33`] selected (a gunner in a fight, buses, rappellers, a barracks training, a caster and a
    /// charger with special power modules), leave the logic's CRCs equal, frame by frame for 300 frames, to a run without
    /// them. The reads are the AI's `isMoving` (a module call) and `isWaitingForPath`, `canCrushOrSquish`, the weapon set,
    /// the special power modules' `getInitiateSound` (a module call), `isUsingAirborneLocomotor` and the upgrades.
    ///
    /// *Predictions, written before the first run:* the CRCs equal on all 300 frames; nothing sent; every call plays one
    /// voice (the templates have no voice names, so each is the no-sound event, which upstream still copies and "plays")
    /// except `MSG_DO_SPECIAL_POWER*` without a matching module (none) and the waypoint skip (not in waypoint mode: none),
    /// so voices are found for the special power types of the caster (`SPECIAL_CARPET_BOMB`) and the charger
    /// (`SPECIAL_PARADROP_AMERICA`) as `InitiateSound` sources; the gunner's weapon is found (`DAMAGE_ARMOR_PIERCING`).
    #[test]
    fn the_unit_voices_questions_leave_the_crcs_alone() {
        use crate::client::unit_voice::{pick_and_play_unit_voice_response, PickAndPlayInfo, VoiceSource};
        use crate::common::message_types::*;
        use crate::common::special_power_type::{SPECIAL_CARPET_BOMB, SPECIAL_INVALID, SPECIAL_PARADROP_AMERICA};
        let frames = 300;
        let (mut plain, _) = world33();
        let (w, x) = world33();
        let mut c = WorldClient::new(w, 1);
        let mut ui = InGameUiSelection::new();
        for o in c.local_player_objects() {
            ui.select_drawable(o, &mut c);
        }
        let msgs = [
            MSG_DOCK, MSG_CREATE_SELECTED_GROUP, MSG_SELECT_TEAM3, MSG_EVACUATE, MSG_DO_REPAIR, MSG_COMBATDROP_AT_LOCATION, MSG_ENTER,
            MSG_DO_MOVETO, MSG_DO_ATTACKMOVETO, MSG_GET_REPAIRED, MSG_GET_HEALED, MSG_DO_SALVAGE, MSG_RESUME_CONSTRUCTION,
            MSG_DOZER_CONSTRUCT, MSG_DO_FORCE_ATTACK_GROUND, MSG_SWITCH_WEAPONS, MSG_DO_FORCE_ATTACK_OBJECT, MSG_DO_ATTACK_OBJECT,
            MSG_DO_WEAPON_AT_OBJECT, MSG_DO_WEAPON_AT_LOCATION, MSG_DO_GUARD_POSITION, MSG_DO_SPECIAL_POWER,
            MSG_DO_SPECIAL_POWER_AT_LOCATION, MSG_INTERNET_HACK,
        ];
        let (mut asked, mut sources) = (0u32, std::collections::BTreeSet::new());
        for f in 0..frames {
            plain.logic_update().expect("a frame");
            c.world.logic_update().expect("a frame");
            let list = ui.all_selected_drawables().to_vec();
            for &m in &msgs {
                for (air, target, slot, sp) in [
                    (false, None, None, SPECIAL_INVALID),
                    (true, Some(x.bus), Some(0), SPECIAL_CARPET_BOMB),
                    (false, Some(x.barracks), Some(1), SPECIAL_PARADROP_AMERICA),
                ] {
                    let info = PickAndPlayInfo { air, draw_target: target, weapon_slot: slot, special_power_type: sp };
                    pick_and_play_unit_voice_response(&list, m, Some(&info), &ui, &mut c);
                    asked += 1;
                }
            }
            for v in c.voices.drain(..) {
                sources.insert(format!("{:?}", v.source));
            }
            let a = c.world.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).expect("the CRC");
            let b = plain.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).expect("the CRC");
            assert_eq!(a, b, "the CRCs part at frame {f}");
        }
        eprintln!("#51: {asked} voice responses asked over {frames} frames; sources {sources:?}");
        assert!(c.sent.is_empty());
        assert!(sources.contains(&format!("{:?}", VoiceSource::InitiateSound(SPECIAL_CARPET_BOMB))));
        assert!(sources.contains(&format!("{:?}", VoiceSource::InitiateSound(SPECIAL_PARADROP_AMERICA))));
        let gun = crate::client::unit_voice::UnitVoiceHost::voice_weapon(&c, x.gunner, None).expect("the gunner's weapon");
        assert_eq!((gun.slot, gun.name.as_slice()), (0, &b"ClipGun"[..]));
    }

    /// #33: the game's own templates for the control bar's questions, read after the gate's INI (the gate's are unchanged).
    /// A gunner whose weapon has a two-shot clip that reloads in 1 s (30 frames) and a fire, a switch and a stopped-only
    /// fire button; a bus carrying vehicles, with evacuate and combat drop; riders that can rappel; a barracks that trains
    /// recruits and researches a player upgrade; a caster with a shared (`SharedSyncedTimer`) power and no create module, so
    /// no timer is started; a charger whose non-shared power (6 s, 180 frames) starts recharging when it is built.
    const EXTRA33: &str = "\
Weapon ClipGun\n PrimaryDamage = 50\n PrimaryDamageRadius = 0\n AttackRange = 100\n DamageType = ARMOR_PIERCING\n DeathType = NORMAL\n WeaponSpeed = 999999\n DelayBetweenShots = 100\n ClipSize = 2\n ClipReloadTime = 1000\n AutoReloadsClip = Yes\nEnd\n\
Upgrade Upgrade_Drill\n Type = PLAYER\n BuildCost = 100\n BuildTime = 10.0\n ButtonImage = SUDrill\nEnd\n\
ObjectCreationList OCL_Nothing\n CreateObject\n  ObjectNames = Depot\n  Count = 1\n End\nEnd\n\
SpecialPower SharedDrop\n Enum = SPECIAL_CARPET_BOMB\n ReloadTime = 6000\n SharedSyncedTimer = Yes\nEnd\n\
SpecialPower OwnDrop\n Enum = SPECIAL_PARADROP_AMERICA\n ReloadTime = 6000\nEnd\n\
Object Gunner\n KindOf = VEHICLE\n CommandSet = GunnerSet\n SelectPortrait = SNGunner_L\n UpgradeCameo1 = Upgrade_Drill\n VisionRange = 150\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 WeaponSet\n  Conditions = None\n  Weapon = PRIMARY ClipGun\n End\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n  AutoAcquireEnemiesWhenIdle = Yes\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n\
Object Rappeller\n KindOf = VEHICLE CAN_RAPPEL\n TransportSlotCount = 1\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 50\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n\
Object Bus\n KindOf = VEHICLE TRANSPORT\n CommandSet = BusSet\n Geometry = CYLINDER\n GeometryMajorRadius = 8\n GeometryHeight = 6\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 200\n  InitialHealth = 200\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = TransportContain ModuleTag_Contain\n  Slots = 2\n  AllowInsideKindOf = VEHICLE\n  AllowAlliesInside = Yes\n  ExitDelay = 0\n End\n\
 Behavior = PhysicsBehavior ModuleTag_Physics\n  Mass = 100\n End\n Locomotor = SET_NORMAL GateTreads\nEnd\n\
Object Recruit\n KindOf = INFANTRY\n BuildCost = 300\n BuildTime = 2.0\n Geometry = CYLINDER\n GeometryMajorRadius = 3\n GeometryHeight = 3\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 10\n  InitialHealth = 10\n End\nEnd\n\
Object Barracks\n KindOf = STRUCTURE\n CommandSet = BarracksSet\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = ProductionUpdate ModuleTag_Production\n  MaxQueueEntries = 3\n End\nEnd\n\
Object Caster\n KindOf = VEHICLE\n CommandSet = CasterSet\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = OCLSpecialPower ModuleTag_Drop\n  SpecialPowerTemplate = SharedDrop\n  OCL = OCL_Nothing\n  CreateLocation = CREATE_AT_LOCATION\n End\nEnd\n\
Object Charger\n KindOf = VEHICLE\n CommandSet = ChargerSet\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
 Behavior = AIUpdateInterface ModuleTag_AI\n End\n\
 Behavior = OCLSpecialPower ModuleTag_Drop\n  SpecialPowerTemplate = OwnDrop\n  OCL = OCL_Nothing\n  CreateLocation = CREATE_AT_LOCATION\n End\n\
 Behavior = SpecialPowerCreate ModuleTag_Create\n End\nEnd\n\
CommandButton Command_Fire\n Command = FIRE_WEAPON\n WeaponSlot = PRIMARY\nEnd\n\
CommandButton Command_FireStopped\n Command = FIRE_WEAPON\n WeaponSlot = PRIMARY\n Options = MUST_BE_STOPPED\nEnd\n\
CommandButton Command_Switch\n Command = SWITCH_WEAPON\n WeaponSlot = PRIMARY\nEnd\n\
CommandButton Command_SwitchSecondary\n Command = SWITCH_WEAPON\n WeaponSlot = SECONDARY\nEnd\n\
CommandButton Command_Evac\n Command = EVACUATE\nEnd\n\
CommandButton Command_CombatDrop\n Command = COMBATDROP\nEnd\n\
CommandButton Command_Recruit\n Command = UNIT_BUILD\n Object = Recruit\n TextLabel = CONTROLBAR:Recruit\n DescriptLabel = CONTROLBAR:RecruitDesc\nEnd\n\
CommandButton Command_Drill\n Command = PLAYER_UPGRADE\n Upgrade = Upgrade_Drill\n TextLabel = CONTROLBAR:Drill\n DescriptLabel = CONTROLBAR:DrillDesc\nEnd\n\
CommandButton Command_Shared\n Command = SPECIAL_POWER\n SpecialPower = SharedDrop\nEnd\n\
CommandButton Command_Own\n Command = SPECIAL_POWER\n SpecialPower = OwnDrop\nEnd\n\
CommandButton Command_Overcharge\n Command = TOGGLE_OVERCHARGE\nEnd\n\
CommandButton Command_Hack\n Command = HACK_INTERNET\nEnd\n\
CommandButton Command_PlanStop\n Command = STOP\n Options = OPTION_ONE\nEnd\n\
CommandButton Command_Rail\n Command = EXECUTE_RAILED_TRANSPORT\nEnd\n\
CommandSet GunnerSet\n 1 = Command_Fire\n 2 = Command_FireStopped\n 3 = Command_Switch\n 4 = Command_SwitchSecondary\n 5 = Command_Overcharge\n 6 = Command_Hack\n 7 = Command_PlanStop\n 8 = Command_Rail\nEnd\n\
CommandSet BusSet\n 1 = Command_Evac\n 2 = Command_CombatDrop\nEnd\n\
CommandSet BarracksSet\n 1 = Command_Recruit\n 2 = Command_Drill\nEnd\n\
CommandSet CasterSet\n 1 = Command_Shared\nEnd\n\
CommandSet ChargerSet\n 1 = Command_Own\nEnd\n";

    /// The objects of [`EXTRA33`] in the gate's game (player 1's, `team1`): a gunner 80 from player 2's nearer soldier
    /// (within its range of 100), a bus with two rappellers inside, an empty bus, a barracks with money to spend, a caster
    /// and a charger.
    struct Extra33 {
        gunner: ObjectId,
        bus: ObjectId,
        empty_bus: ObjectId,
        barracks: ObjectId,
        caster: ObjectId,
        charger: ObjectId,
        made_on: u32,
    }

    fn world33() -> (World, Extra33) {
        use crate::logic::world::gate_scenario::{gate_world_with, make};
        use crate::logic::world::team_ops::factory_find_team;
        let mut w = gate_world_with(EXTRA33);
        let team = factory_find_team(&mut w, b"team1").expect("the team").expect("team1");
        let at = |x: f32, y: f32| Coord3D { x, y, z: 0.0 };
        let gunner = make(&mut w, b"Gunner", team, at(170.0, 380.0));
        let bus = make(&mut w, b"Bus", team, at(60.0, 200.0));
        let empty_bus = make(&mut w, b"Bus", team, at(60.0, 240.0));
        for y in [180.0, 190.0] {
            let r = make(&mut w, b"Rappeller", team, at(40.0, y));
            let c = w.get_contain(bus).expect("the bus's contain");
            w.module_call(bus, c, ModuleCall::AddToContain(r));
        }
        let barracks = make(&mut w, b"Barracks", team, at(320.0, 260.0));
        let p = w.get_controlling_player(barracks).expect("the player");
        w.cut(crate::logic::world::object_cuts::CutCall::PlayerMoneyDeposit { player: p, amount: 1000 });
        let caster = make(&mut w, b"Caster", team, at(320.0, 320.0));
        let made_on = w.frame();
        let charger = make(&mut w, b"Charger", team, at(340.0, 320.0));
        (w, Extra33 { gunner, bus, empty_bus, barracks, caster, charger, made_on })
    }

    /// The availability and the clock of the button named `name` for `obj`.
    fn availability(c: &mut WorldClient, obj: ObjectId, name: &[u8]) -> (crate::client::control_bar_command::CommandAvailability, Option<i32>) {
        use crate::client::control_bar_command::ControlBarStateHost;
        let i = c.world.cuts.command_buttons.iter().position(|b| b.name == name).expect("the button");
        let b = c.button(i).expect("the button");
        crate::client::control_bar_command::get_command_availability(&b, Some(obj), false, c)
    }

    /// #33, the fire weapon button's clock. Predicted before the run, from `Weapon.cpp` and `getCommandAvailability`: the
    /// gunner acquires player 2's soldier (80 away, in range) and fires its two-shot clip; the second shot empties it and
    /// starts the reload (`AutoReloadsClip`), `RELOADING_CLIP` until 30 frames later (`ClipReloadTime = 1000`, 30 logic frames
    /// a second). 15 frames into the reload the button is `NOT_READY` with the clock at `getPercentReadyToFire() * 100` =
    /// 15 / 30 = **50**; the clip's reload time is 30, not 0. Reading leaves the weapon equal to a copy taken before (its
    /// `m_status` cell included). Once both enemies are dead (50 damage a shot, 100 health each) and the gunner is idle,
    /// the weapon's cell is stale: `getStatus()` would rewrite it to `READY_TO_FIRE`, which `peek_status` answers without
    /// the write (the hazard the pure read avoids).
    ///
    /// *Results (2026-11-06):* the reload started on frame 7; on frame 22 the weapon read clip reload 30, reloading,
    /// next shot 37, percent 0.5, and the button `(NOT_READY, 50)`, as predicted. The stale cell came **earlier than
    /// predicted**: on frame 37, the frame the reload ended, before the AI fired again (`RELOADING_CLIP` in the cell,
    /// `READY_TO_FIRE` read), not only after the fight.
    #[test]
    fn a_unit_that_just_fired_shows_its_reload_clock() {
        use crate::client::control_bar_command::{CommandAvailability, ControlBarStateHost};
        use crate::logic::weapon_types::WeaponStatus;
        let (w, x) = world33();
        let mut c = WorldClient::new(w, 1);
        let slot = crate::logic::weapon_fire::PRIMARY_WEAPON;
        let weapon = |c: &WorldClient| c.world.find_object(x.gunner).and_then(|o| o.weapon_set.get_weapon_in_weapon_slot(slot)).cloned().expect("the clip gun");
        let mut reload_started = None;
        for _ in 0..200 {
            c.world.logic_update().expect("a frame");
            let wpn = weapon(&c);
            if wpn.peek_status(c.world.frame()) == WeaponStatus::ReloadingClip {
                reload_started = Some(wpn.last_reload_started_frame());
                break;
            }
        }
        let started = reload_started.expect("the gunner emptied its clip");
        while c.world.frame() < started + 15 {
            c.world.logic_update().expect("a frame");
        }
        let before = weapon(&c);
        let state = c.weapon(x.gunner, slot).expect("a weapon");
        let got = availability(&mut c, x.gunner, b"Command_Fire");
        eprintln!("#33: reload started on frame {started}; at {} the weapon reads {state:?}, the button {got:?}", c.world.frame());
        assert_eq!(state.clip_reload_time, 30);
        assert!(state.reloading_clip);
        assert_eq!(got, (CommandAvailability::NotReady, Some(50)));
        assert_eq!(weapon(&c), before, "the read left the weapon as it was");
        // the stale cell, once the fight is over
        let mut stale = None;
        for _ in 0..600 {
            c.world.logic_update().expect("a frame");
            let wpn = weapon(&c);
            let now = c.world.frame();
            // (every frame: the pure reads answer as upstream's getters, on a copy, do)
            let probe = wpn.clone();
            assert_eq!(probe.get_percent_ready_to_fire(now).to_bits(), wpn.peek_percent_ready_to_fire(now).to_bits(), "frame {now}");
            if wpn.peek_status(now) != wpn.raw_status() {
                stale = Some((now, wpn.raw_status(), wpn.peek_status(now)));
                let copy = wpn.clone();
                assert_eq!(copy.get_status(now), wpn.peek_status(now), "the pure read answers what getStatus() answers");
                assert_ne!(copy, wpn, "upstream's getStatus() writes the cell");
                break;
            }
        }
        eprintln!("#33: the first stale status cell: {stale:?}");
        assert!(stale.is_some_and(|(_, _, peek)| peek == WeaponStatus::ReadyToFire));
    }

    /// #33, a special power's clock and the shared timer. Predicted before the run, from `SpecialPowerModule.cpp` and
    /// `getCommandAvailability`: the charger's own power (6 s = 180 frames) starts recharging when it is built
    /// (`SpecialPowerCreate`), so 90 frames later its button is `NOT_READY` with the clock at
    /// `(1 - 90 / 180) * 100` = **50**. The caster's power is shared and no timer was started (no create module): the
    /// player's `getOrStartSpecialPowerReadyFrame` would answer now, so the power is ready and the button `AVAILABLE`, and
    /// the read leaves the player's timer list **empty**. The ablation: the module's own `isReady` (upstream's call) adds
    /// one timer to the list.
    ///
    /// *Results (2026-11-06):* as predicted: built on frame 0, at frame 90 the charger's button `(NOT_READY, 50)`, the
    /// caster's `(AVAILABLE, none)`; the timer list stayed empty, and the module's own `isReady` added one.
    #[test]
    fn a_power_shows_its_charge_and_a_shared_one_starts_no_timer() {
        use crate::client::control_bar_command::CommandAvailability;
        let (w, x) = world33();
        let mut c = WorldClient::new(w, 1);
        while c.world.frame() < x.made_on + 90 {
            c.world.logic_update().expect("a frame");
        }
        let p = c.world.get_controlling_player(x.caster).expect("the player");
        let timers = |c: &WorldClient| c.world.players.get_nth(p).map(|pl| pl.special_power_timers.len()).expect("the player");
        assert_eq!(timers(&c), 0);
        let charging = availability(&mut c, x.charger, b"Command_Own");
        let shared = availability(&mut c, x.caster, b"Command_Shared");
        eprintln!("#33: at frame {} (built on {}): own power {charging:?}, shared {shared:?}", c.world.frame(), x.made_on);
        assert_eq!(charging, (CommandAvailability::NotReady, Some(50)));
        assert_eq!(shared, (CommandAvailability::Available, None));
        assert_eq!(timers(&c), 0, "the client's read started no timer");
        // the ablation: upstream's getter starts it
        let t = c.world.statics.special_power_store.find_special_power_template(b"SharedDrop").map(|t| t.id());
        let m = c.world.get_special_power_module(x.caster, t).expect("the caster's power");
        c.world.module_call(x.caster, m, ModuleCall::IsReady);
        assert_eq!(timers(&c), 1, "the module's isReady starts the shared timer");
    }

    /// #33, the contain, production and AI answers. Predicted before the run, from `getCommandAvailability`:
    /// - the bus with two rappellers: evacuate `AVAILABLE` (count 2) and combat drop `AVAILABLE` (2 that can rappel); the
    ///   empty bus: both `RESTRICTED`;
    /// - the barracks: the player upgrade `AVAILABLE` (1000 to spend, it costs 100); once queued, `CANT_AFFORD` (in
    ///   production: the swap upstream's comment shows);
    /// - a runner of the gate, ordered across the map, is moving at frame 5: the gunner's stopped-only fire button on it is
    ///   `RESTRICTED`; on the idle gun (no command set needed: the button is asked directly) the AI is not moving, so the
    ///   answer falls to the fire weapon case: the gun has no weapon in its primary slot, so `RESTRICTED` as well; ~~the
    ///   gunner (stopped, weapon ready at frame 5) `AVAILABLE`~~ **missed on the first run (2026-11-06):** the gunner had
    ///   already fired by frame 5 (its reload starts on frame 7, after the second shot: see
    ///   `a_unit_that_just_fired_shows_its_reload_clock`), so between shots it is `NOT_READY` without a clock (upstream draws
    ///   the clock only while the clip reloads); the prediction assumed it had not acquired its target yet;
    /// - the switch weapon buttons: with the gunner selected (its current slot primary) the primary one is `ACTIVE`; the
    ///   secondary one `RESTRICTED` (no weapon there);
    /// - the gate's dozer is idle: `isTaskPending(DOZER_TASK_BUILD)` false.
    #[test]
    fn the_contain_production_and_ai_answers() {
        use crate::client::control_bar_command::{CommandAvailability as A, ControlBarStateHost};
        let (w, x) = world33();
        let mut c = WorldClient::new(w, 1);
        for _ in 0..5 {
            c.world.logic_update().expect("a frame");
        }
        assert_eq!((c.contain_count(x.bus), c.rappeller_count(x.bus)), (Some(2), 2));
        assert_eq!(availability(&mut c, x.bus, b"Command_Evac").0, A::Available);
        assert_eq!(availability(&mut c, x.bus, b"Command_CombatDrop").0, A::Available);
        assert_eq!(availability(&mut c, x.empty_bus, b"Command_Evac").0, A::Restricted);
        assert_eq!(availability(&mut c, x.empty_bus, b"Command_CombatDrop").0, A::Restricted);
        assert_eq!(availability(&mut c, x.barracks, b"Command_Drill").0, A::Available);
        let key = c.world.cuts.command_buttons.iter().find(|b| b.name == b"Command_Drill").and_then(|b| b.upgrade).expect("the upgrade");
        let pu = c.world.get_production_update_interface(x.barracks).expect("the factory");
        assert!(c.world.module_call(x.barracks, pu, ModuleCall::ProductionQueueUpgrade { upgrade: Some(key) }).as_bool(), "queued");
        assert!(c.player_has_upgrade_in_production(x.barracks, key));
        assert_eq!(availability(&mut c, x.barracks, b"Command_Drill").0, A::CantAfford);
        let runner = c.world.objects.ids_in_order().into_iter().find(|&o| c.world.find_object(o).is_some_and(|ob| ob.template().get_name() == b"Runner")).expect("a runner");
        let gun = c.world.objects.ids_in_order().into_iter().find(|&o| c.world.find_object(o).is_some_and(|ob| ob.template().get_name() == b"Gun")).expect("the gun");
        assert!(c.is_moving(runner));
        assert!(!c.is_moving(gun));
        assert_eq!(availability(&mut c, runner, b"Command_FireStopped").0, A::Restricted);
        assert_eq!(availability(&mut c, gun, b"Command_FireStopped").0, A::Restricted);
        assert_eq!(availability(&mut c, x.gunner, b"Command_FireStopped"), (A::NotReady, None));
        c.view.selection = vec![x.gunner];
        assert_eq!(availability(&mut c, x.gunner, b"Command_Switch").0, A::Active);
        assert_eq!(availability(&mut c, x.gunner, b"Command_SwitchSecondary").0, A::Restricted);
        let dozer = c.world.objects.ids_in_order().into_iter().find(|&o| c.is_dozer(o)).expect("the dozer");
        assert_eq!(c.dozer_build_pending(dozer), Some(false));
        assert_eq!(c.dozer_build_pending(x.gunner), None);
        assert!(c.sent.is_empty());
    }

    /// #35, the portrait and the popup, from the World. Predicted before the run, from `setPortraitByObject` and
    /// `populateBuildTooltipLayout`, with the test's strings (`TOOLTIP:Cost` = "Cost: $%d", the labels' texts, and
    /// `CONTROLBAR:PowerDescription` = "Power %d/%d"):
    /// - nothing selected: the portrait hidden;
    /// - the gunner (regular): `SNGunner_L`, no chevron, the first upgrade cameo `SUDrill` disabled (the player has not
    ///   researched the drill), the other four hidden; once the player has the drill, the cameo enabled;
    /// - the recruit button, the barracks selected with 1000 to spend: name "Recruit", cost "Cost: $300", description
    ///   "Trains a recruit." (it can be made, no prerequisites);
    /// - the drill button: "Drill", "Cost: $100", "Researches drills."; once the player has it: no cost and
    ///   `TOOLTIP:AlreadyUpgradedDefault`'s "Already upgraded.";
    /// - the power window: "Power" and "Power 0/0" (player 1 has no power plant).
    ///
    /// *Results (2026-11-06):* every prediction held.
    #[test]
    fn the_portrait_and_the_popup_read_the_world() {
        use crate::client::control_bar_command::ControlBarStateHost;
        use crate::client::control_bar_popup::{populate_build_tooltip_layout, populate_window_tooltip, set_portrait_by_object, TooltipWindow};
        let (mut w, x) = world33();
        let statics = std::sync::Arc::get_mut(&mut w.statics).expect("the test's own statics");
        for (l, t) in [
            ("TOOLTIP:Cost", "Cost: $%d"),
            ("CONTROLBAR:Recruit", "Recruit"),
            ("CONTROLBAR:RecruitDesc", "Trains a recruit."),
            ("CONTROLBAR:Drill", "Drill"),
            ("CONTROLBAR:DrillDesc", "Researches drills."),
            ("TOOLTIP:AlreadyUpgradedDefault", "Already upgraded."),
            ("CONTROLBAR:Power", "Power"),
            ("CONTROLBAR:PowerDescription", "Power %d/%d"),
        ] {
            let units: Vec<u16> = t.encode_utf16().collect();
            statics.game_text.strings.insert(l.as_bytes().to_vec(), crate::common::unicode_string::UnicodeString::try_from_chars(&units).expect("a string"));
        }
        let mut c = WorldClient::new(w, 1);
        let text = |v: &[u16]| String::from_utf16_lossy(v);
        assert_eq!(set_portrait_by_object(None, &mut c), crate::client::control_bar_popup::Portrait::default());
        let p = set_portrait_by_object(Some(x.gunner), &mut c);
        assert!(p.shown);
        assert_eq!((p.image.as_deref(), p.overlay.as_deref()), (Some(&b"SNGunner_L"[..]), None));
        assert_eq!(p.upgrade_cameos[0], Some((Some(b"SUDrill".to_vec()), false)));
        assert!(p.upgrade_cameos[1..].iter().all(Option::is_none));
        c.view.selection = vec![x.barracks];
        let button = |c: &WorldClient, n: &[u8]| c.button(c.world.cuts.command_buttons.iter().position(|b| b.name == n).expect("the button")).expect("the button");
        let t = populate_build_tooltip_layout(&button(&c, b"Command_Recruit"), &mut c);
        assert_eq!((text(&t.name), t.cost.as_deref().map(text), text(&t.description)), ("Recruit".into(), Some("Cost: $300".into()), "Trains a recruit.".into()));
        let t = populate_build_tooltip_layout(&button(&c, b"Command_Drill"), &mut c);
        assert_eq!((text(&t.name), t.cost.as_deref().map(text), text(&t.description)), ("Drill".into(), Some("Cost: $100".into()), "Researches drills.".into()));
        let key = button(&c, b"Command_Drill").upgrade.expect("the upgrade").0;
        let mask = c.world.statics.upgrade_center.find_upgrade_by_key(NameKeyType(key)).expect("the upgrade").upgrade_mask();
        c.world.player_add_upgrade(1, key, &mask, crate::common::upgrade::UPGRADE_STATUS_COMPLETE);
        let t = populate_build_tooltip_layout(&button(&c, b"Command_Drill"), &mut c);
        assert_eq!((t.cost.is_none(), text(&t.description)), (true, "Already upgraded.".into()));
        let p = set_portrait_by_object(Some(x.gunner), &mut c);
        assert_eq!(p.upgrade_cameos[0], Some((Some(b"SUDrill".to_vec()), true)));
        let t = populate_window_tooltip(TooltipWindow::Power, &mut c);
        eprintln!("#35: the power's popup: {:?} / {:?}", text(&t.name), text(&t.description));
        assert_eq!((text(&t.name), text(&t.description)), ("Power".into(), "Power 0/0".into()));
    }

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
