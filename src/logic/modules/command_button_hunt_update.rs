// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CommandButtonHuntUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/CommandButtonHuntUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CommandButtonHuntUpdate`: a unit a script has told to hunt with one of its command buttons (`setCommandButton(name)`, `ScriptActions`'
//! call). The module sleeps until then. Setting a button finds it by name in the object's command set, idles the AI, runs one `update()`
//! and wakes the module. Each update hunts for as long as the AI's last command came from the AI (a command from a script or the player
//! ends the hunt and forgets the button):
//!
//! - a weapon button (fire or switch weapon) sends an idle AI hunting and locks the button's weapon slot temporarily, every frame;
//! - a special power button, every `ScanRate` frames while the AI is idle and the object's special ability update of that power (if the
//!   button has a power) is not active, scans for the best target the power can be used at and uses the button at it;
//! - an enter button (hijack, convert to car bomb, sabotage) does the same with the first object in range it may enter that way.
//!
//! The scan (`scanClosestTarget`) is the partition manager's, nearest first, of the living objects of the same map status that are not
//! stealthed and undetected, of the enemies (the neutrals for a car bomb; any relationship for a building capture). With a special power
//! module for the button's power, a target scores `ScanRange - distance` (or the AI's attack priority for its template when the AI has an
//! attack info), less one per `m_attackPriorityDistanceModifier` of distance (at least 1), and the highest score wins (on a tie, the
//! higher unmodified priority). A disabled target is skipped by the vehicle hack; the building capture skips its own player's objects and
//! its allies'.
//!
//! The command buttons and sets are the client's (not translated): `cuts.command_buttons` and `cuts.command_sets`. The action manager's
//! tests are [`World::action_manager_can`] (it is not on `World` yet), the scan [`World::partition_iterate_hunt_targets`], the distance
//! [`World::body_get_distance_squared`] and `doCommandButtonAtObject` a [`CutCall::ObjectDoCommandButtonAtObject`] record.
//!
//! # Upstream facts kept
//!
//! - `isPlaceExplosive` is set only inside `if (isCaptureBuilding)`, where the power is the building capture, so it is never true and the
//!   test for one of the player's own mines near the target never runs.
//! - An enter button whose command has a special power template and whose object has a special power module for it takes the special
//!   power branch of the scan, not the enter branch.
//! - The score's conversions to `Int` (`m_scanRange - dist`, `dist / m_attackPriorityDistanceModifier`) are undefined out of range
//!   ([`Undefined::FloatToIntOutOfRange`](crate::common::ini::Undefined::FloatToIntOutOfRange): a zero modifier, or a huge scan range).
//!
//! [`CutCall::ObjectDoCommandButtonAtObject`]: crate::logic::world::object_cuts::CutCall::ObjectDoCommandButtonAtObject
//!
//! `p1w13-sonnet`, batch R69, 2026-10-26.

use std::sync::Arc;

use crate::common::game_common::{Relationship, LOGICFRAMES_PER_SECOND};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::logic::world::object::ModuleIndex;
use crate::common::special_power_type::{SpecialPowerType, SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK, SPECIAL_INFANTRY_CAPTURE_BUILDING};
use crate::common::special_power_type::{SPECIAL_TANKHUNTER_TNT_ATTACK, SPECIAL_TIMED_CHARGES};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::auto_deposit_update::real_to_int;
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_set_state::LOCKED_TEMPORARILY;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `MAX_COMMANDS_PER_SET` (`ControlBar.h`).
pub const MAX_COMMANDS_PER_SET: usize = 18;

/// `GUICommandType` values the module tests (`ControlBar.h`, `ALLOW_SURRENDER` not defined).
pub const GUI_COMMAND_FIRE_WEAPON: i32 = 20;
/// `GUI_COMMAND_SPECIAL_POWER`.
pub const GUI_COMMAND_SPECIAL_POWER: i32 = 21;
/// `GUI_COMMAND_SWITCH_WEAPON`.
pub const GUI_COMMAND_SWITCH_WEAPON: i32 = 26;
/// `GUICOMMANDMODE_HIJACK_VEHICLE`.
pub const GUICOMMANDMODE_HIJACK_VEHICLE: i32 = 27;
/// `GUICOMMANDMODE_CONVERT_TO_CARBOMB`.
pub const GUICOMMANDMODE_CONVERT_TO_CARBOMB: i32 = 28;
/// `GUICOMMANDMODE_SABOTAGE_BUILDING`.
pub const GUICOMMANDMODE_SABOTAGE_BUILDING: i32 = 29;

/// `PartitionFilterRelationship::ALLOW_ENEMIES`.
pub const ALLOW_ENEMIES: i32 = 1;
/// `PartitionFilterRelationship::ALLOW_NEUTRAL`.
pub const ALLOW_NEUTRAL: i32 = 4;

/// The kinds of [`World::action_manager_can`]: `canDoSpecialPowerAtObject`.
pub const CAN_DO_SPECIAL_POWER: i32 = 0;
/// `canHijackVehicle`.
pub const CAN_HIJACK_VEHICLE: i32 = 1;
/// `canSabotageBuilding`.
pub const CAN_SABOTAGE_BUILDING: i32 = 2;
/// `canConvertObjectToCarBomb`.
pub const CAN_CONVERT_TO_CAR_BOMB: i32 = 3;

/// `class CommandButtonHuntUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandButtonHuntUpdateModuleData {
    /// The base (`UpdateModuleData`, which adds nothing to `ModuleData`).
    pub update: UpdateModuleData,
    /// `m_scanFrames` (`ScanRate`).
    pub scan_frames: u32,
    /// `m_scanRange`.
    pub scan_range: f32,
}

impl Default for CommandButtonHuntUpdateModuleData {
    /// `CommandButtonHuntUpdateModuleData()`: a scan a second, range 9999.
    fn default() -> Self {
        CommandButtonHuntUpdateModuleData { update: UpdateModuleData::default(), scan_frames: LOGICFRAMES_PER_SECOND as u32, scan_range: 9999.0 }
    }
}

/// `CommandButtonHuntUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<CommandButtonHuntUpdateModuleData>; 3] = [
    FieldParse::new("ScanRate", |ini, d, _| { d.scan_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScanRange", |ini, d, _| { d.scan_range = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl CommandButtonHuntUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (no fields of its own besides the behaviour base's), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CommandButtonHuntUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CommandButtonHuntUpdateModuleData> {
        let mut data = CommandButtonHuntUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CommandButtonHuntUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class CommandButtonHuntUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct CommandButtonHuntUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getCommandButtonHuntUpdateModuleData()`.
    data: Arc<CommandButtonHuntUpdateModuleData>,
    /// `m_commandButtonName`.
    command_button_name: Vec<u8>,
    /// `m_commandButton`: the button's index in `cuts.command_buttons`, `None` for `NULL`.
    command_button: Option<usize>,
}

impl PartialEq for CommandButtonHuntUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same button.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.command_button_name == other.command_button_name
            && self.command_button == other.command_button
    }
}

impl CommandButtonHuntUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data), no button.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<CommandButtonHuntUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(CommandButtonHuntUpdate { update, data, command_button_name: Vec::new(), command_button: None })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getCommandButtonHuntUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<CommandButtonHuntUpdateModuleData> {
        &self.data
    }

    /// The button's name (`m_commandButtonName`) and the button (`m_commandButton`: its index in `cuts.command_buttons` plus 1, 0 for `NULL`),
    /// for the tests that compare them with upstream's.
    pub fn state(&self) -> (Vec<u8>, u32) {
        (self.command_button_name.clone(), self.command_button.map_or(0, |b| b as u32 + 1))
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: asleep forever.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls: `setCommandButton`.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, that half, the button's name.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The button the object's command set holds under `name`, the way `setCommandButton` looks for it: the first slot whose button has a
    /// name and that name.
    fn find_button(world: &World, me: ObjectId, name: &[u8], skip_empty: bool) -> Option<usize> {
        let set = world.control_bar_find_command_set(&world.object_get_command_set_string(me))?;
        (0..MAX_COMMANDS_PER_SET).filter_map(|i| world.command_set_get_command_button(set, i)).find(|&b| {
            let n = &world.cuts.command_buttons[b].name;
            (!skip_empty || !n.is_empty()) && n.as_slice() == name
        })
    }

    /// The end of a load (`xfer`'s `XFER_LOAD` half, which needs the world): the button of the loaded name in the object's command set,
    /// if the name is not empty. [`World::xfer_module_slot`] runs it after a load that read the name.
    pub fn find_loaded_button(&mut self, world: &World, me: ObjectId) {
        self.command_button = None;
        if !self.command_button_name.is_empty() {
            self.command_button = Self::find_button(world, me, &self.command_button_name, false);
        }
    }

    /// `setCommandButton(buttonName)`.
    pub fn set_command_button(&mut self, world: &mut World, me: ModuleHandle, button_name: &[u8]) {
        self.command_button_name = button_name.to_vec();
        self.command_button = Self::find_button(world, me.object, button_name, !crate::logic::modules::plant(2391));
        if self.command_button.is_none() {
            return;
        }
        let Some(ai) = world.objects.find(me.object).and_then(|o| o.ai) else { return };
        // "Stop whatever we're doing."
        world.module_call(me.object, ai, ModuleCall::AiIdleFrom { command_source: CMD_FROM_AI });
        self.update_inner(world, me);
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }

    /// The special power template of the button, `None` for `NULL` (or a template the store does not hold).
    fn button_power(&self, world: &World) -> Option<(u32, SpecialPowerType)> {
        let id = world.cuts.command_buttons[self.command_button?].special_power?;
        world.statics.special_power_store.find_special_power_template_by_id(id).map(|t| (t.id(), t.special_power_type()))
    }

    /// `update()`.
    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let ai = world.objects.find(me.object).and_then(|o| o.ai);
        let (Some(ai), Some(button)) = (ai, self.command_button) else { return UPDATE_SLEEP_FOREVER };
        let source = world.module_call(me.object, ai, ModuleCall::AiGetLastCommandSource);
        if !matches!(source, ModuleReply::Int(s) if s == CMD_FROM_AI) && !crate::logic::modules::plant(2392) {
            // "If a script or the player (in this case should only be script, but either way) we quit hunting."
            self.command_button = None;
            self.command_button_name.clear();
            return UPDATE_SLEEP_FOREVER;
        }
        let r = match world.cuts.command_buttons[button].command_type {
            GUI_COMMAND_SPECIAL_POWER => self.hunt_special_power(world, me.object, ai),
            GUI_COMMAND_SWITCH_WEAPON | GUI_COMMAND_FIRE_WEAPON => Ok(self.hunt_weapon(world, me.object, ai, button)),
            GUICOMMANDMODE_HIJACK_VEHICLE | GUICOMMANDMODE_CONVERT_TO_CARBOMB | GUICOMMANDMODE_SABOTAGE_BUILDING => self.hunt_enter(world, me.object, ai),
            _ => Ok(UPDATE_SLEEP_FOREVER),
        };
        r.unwrap_or_else(|e| {
            world.cuts.module_errors.push((me, e));
            UPDATE_SLEEP_FOREVER
        })
    }

    /// `huntWeapon(ai)`.
    fn hunt_weapon(&self, world: &mut World, me: ObjectId, ai: ModuleIndex, button: usize) -> UpdateSleepTime {
        if world.module_call(me, ai, ModuleCall::AiIsIdle).as_bool() {
            world.module_call(me, ai, ModuleCall::AiHunt { command_source: CMD_FROM_AI });
        }
        let weapon_slot = world.cuts.command_buttons[button].weapon_slot;
        // "lock it just till the weapon is empty or the attack is "done""
        if !crate::logic::modules::plant(2393) {
            world.set_weapon_lock(me, weapon_slot, LOCKED_TEMPORARILY);
        }
        UPDATE_SLEEP_NONE
    }

    /// `huntSpecialPower(ai)`.
    fn hunt_special_power(&self, world: &mut World, me: ObjectId, ai: ModuleIndex) -> Result<UpdateSleepTime, IniError> {
        let scan = update_sleep(self.data.scan_frames);
        if !world.module_call(me, ai, ModuleCall::AiIsIdle).as_bool() {
            return Ok(scan);
        }
        if let Some((_, power_type)) = self.button_power(world) {
            let Some(sa) = world.find_special_ability_update(me, power_type) else { return Ok(UPDATE_SLEEP_FOREVER) };
            if world.module_call(me, sa, ModuleCall::SpecialAbilityIsActive).as_bool() && !crate::logic::modules::plant(2394) {
                return Ok(scan);
            }
        }
        // "Periodic scanning (expensive)"
        if let Some(victim) = self.scan_closest_target(world, me)? {
            self.do_command_button_at_object(world, me, victim);
        }
        Ok(scan)
    }

    /// `huntEnter(ai)`: sabotage, convert to car bomb, hijack.
    fn hunt_enter(&self, world: &mut World, me: ObjectId, ai: ModuleIndex) -> Result<UpdateSleepTime, IniError> {
        let scan = update_sleep(self.data.scan_frames);
        if !world.module_call(me, ai, ModuleCall::AiIsIdle).as_bool() {
            return Ok(scan);
        }
        if let Some(victim) = self.scan_closest_target(world, me)? {
            self.do_command_button_at_object(world, me, victim);
        }
        Ok(scan)
    }

    /// `obj->doCommandButtonAtObject(m_commandButton, victim, CMD_FROM_AI)`.
    fn do_command_button_at_object(&self, world: &mut World, me: ObjectId, victim: ObjectId) {
        let button = self.command_button.map(|b| world.cuts.command_buttons[b].name.clone()).unwrap_or_default();
        world.cut(CutCall::ObjectDoCommandButtonAtObject { object: me, button, victim, source: CMD_FROM_AI });
    }

    /// `scanClosestTarget()`: the target, `None` for `NULL`.
    fn scan_closest_target(&self, world: &mut World, me: ObjectId) -> Result<Option<ObjectId>, IniError> {
        let d = self.data.clone();
        let Some(button) = self.command_button else { return Ok(None) };
        let command_type = world.cuts.command_buttons[button].command_type;
        // "only consider enemies (unless it's convert to carbomb)."
        let mut relationship = ALLOW_ENEMIES;
        let mut is_enter = false;
        match command_type {
            GUICOMMANDMODE_CONVERT_TO_CARBOMB => {
                relationship = if crate::logic::modules::plant(2395) { ALLOW_ENEMIES } else { ALLOW_NEUTRAL };
                is_enter = true;
            }
            GUICOMMANDMODE_HIJACK_VEHICLE | GUICOMMANDMODE_SABOTAGE_BUILDING => is_enter = true,
            _ => {}
        }
        let mut team_filter = true;
        let mut is_black_lotus_vehicle_hack = false;
        let mut is_capture_building = false;
        let mut is_place_explosive = false;
        let power = self.button_power(world);
        if !is_enter {
            let Some((_, power_type)) = power else { return Ok(None) };
            is_black_lotus_vehicle_hack = power_type == SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK;
            is_capture_building = power_type == SPECIAL_INFANTRY_CAPTURE_BUILDING;
            if is_capture_building {
                // "It's ok (in fact necessary for oil derricks) to capture special buildings."
                team_filter = crate::logic::modules::plant(2396);
                if power_type == SPECIAL_TIMED_CHARGES || power_type == SPECIAL_TANKHUNTER_TNT_ATTACK {
                    is_place_explosive = true;
                }
            }
        }
        let candidates = world.partition_iterate_hunt_targets(me, d.scan_range, relationship, team_filter);
        let ai = world.objects.find(me).and_then(|o| o.ai);
        let template = power.map_or(0, |(id, _)| id);
        let mut best_target = None;
        let (mut effective_priority, mut actual_priority) = (0i32, 0i32);
        if world.get_special_power_module(me, power.map(|(id, _)| id)).is_some() {
            for other in candidates {
                let disabled = world.objects.find(other).is_some_and(|o| o.is_disabled());
                if disabled && is_black_lotus_vehicle_hack && !crate::logic::modules::plant(2397) {
                    // "The hack disables the vehicle, so we don't want to do it again."
                    continue;
                }
                if is_capture_building {
                    if world.get_controlling_player(me) == world.get_controlling_player(other) {
                        continue; // "no need to capture our own buildings."
                    }
                    if world.get_relationship(me, other) == Relationship::Allies {
                        continue; // "It's not polite to capture allies buildings."
                    }
                }
                if world.action_manager_can(CAN_DO_SPECIAL_POWER, me, other, CMD_FROM_AI, template) {
                    if is_place_explosive {
                        // "Don't target things near explosives... It's just not a good idea." (never reached: see the module's doc)
                        let range = power.and_then(|(id, _)| world.statics.special_power_store.find_special_power_template_by_id(id)).map_or(0.0, |t| t.view_object_range());
                        if world.partition_has_closest_own_mine(other, range) {
                            continue;
                        }
                    }
                    let dist_sqr = world.body_get_distance_squared(me, other);
                    let dist = dist_sqr.sqrt();
                    let mut cur_priority = real_to_int(d.scan_range - dist)?;
                    if let Some(ai) = ai {
                        if let ModuleReply::Int(p) = world.module_call(me, ai, ModuleCall::AiGetAttackPriority { other }) {
                            cur_priority = p;
                        }
                    }
                    if cur_priority == 0 && !crate::logic::modules::plant(2398) {
                        continue; // "don't attack 0 priority targets."
                    }
                    let modifier = real_to_int(dist / world.cuts.attack_priority_distance_modifier)?;
                    let mut mod_priority = cur_priority.wrapping_sub(modifier);
                    if mod_priority < 1 {
                        mod_priority = 1;
                    }
                    if mod_priority > effective_priority {
                        effective_priority = mod_priority;
                        actual_priority = cur_priority;
                        best_target = Some(other);
                    }
                    if mod_priority == effective_priority && cur_priority > actual_priority && !crate::logic::modules::plant(2399) {
                        effective_priority = mod_priority;
                        actual_priority = cur_priority;
                        best_target = Some(other);
                    }
                }
            }
        } else if is_enter {
            let mut valid = false;
            for other in candidates {
                match command_type {
                    GUICOMMANDMODE_HIJACK_VEHICLE => valid = world.action_manager_can(CAN_HIJACK_VEHICLE, me, other, CMD_FROM_AI, 0),
                    GUICOMMANDMODE_SABOTAGE_BUILDING => valid = world.action_manager_can(CAN_SABOTAGE_BUILDING, me, other, CMD_FROM_AI, 0),
                    GUICOMMANDMODE_CONVERT_TO_CARBOMB => valid = world.action_manager_can(CAN_CONVERT_TO_CAR_BOMB, me, other, CMD_FROM_AI, 0),
                    _ => {}
                }
                if valid {
                    return Ok(Some(other));
                }
            }
        }
        Ok(best_target)
    }
}

impl UpdateBehavior for CommandButtonHuntUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        self.update_inner(world, me)
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// The field after the base: the button's name, and on a load the button found again by it.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        if crate::logic::modules::plant(2390) {
            return Ok(());
        }
        xfer.xfer_ascii_string(&mut self.command_button_name)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            // "initialize to no command button"; the world finds it again by name ([`CommandButtonHuntUpdate::find_loaded_button`])
            self.command_button = None;
        }
        Ok(())
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::CommandButtonHuntSetCommandButton(name) => {
                self.set_command_button(world, me, &name);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CommandButtonHuntUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CommandButtonHuntUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for CommandButtonHuntUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the field.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `CommandButtonHuntUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CommandButtonHuntUpdate(Arc::new(CommandButtonHuntUpdateModuleData::friend_new_module_data(ini)?)))
}
