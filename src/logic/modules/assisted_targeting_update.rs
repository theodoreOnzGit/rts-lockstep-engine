// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AssistedTargetingUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AssistedTargetingUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `AssistedTargetingUpdate`: a unit that fires at what another designates (the Patriot battery's assisted targeting). It is free to assist
//! when it can attack and its current weapon is ready; `assistAttack` locks `AssistingWeaponSlot` and attacks the victim for
//! `AssistingClipSize` shots, and draws the feedback lasers: from the requesting object to itself, and from itself to the victim (each a new
//! object of the laser's template on the controlling player's default team, at the start, whose `LaserUpdate` client module is told its ends;
//! a laser object without one is destroyed).
//!
//! # Upstream facts kept
//!
//! - Both laser templates are looked up by `LaserFromAssisted`'s name (in `update`, which then sleeps for ever, and in `loadPostProcess`):
//!   `LaserToTarget` is read and never used.
//! - The templates are not saved (`xfer` is the version and the update base).
//! - A laser object without a drawable is read through `NULL` (`Undefined::NullDrawable`).
//!
//! `p1w13-sonnet`, batch R59, 2026-10-25.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_set::WEAPON_SLOT_TYPE_NAMES;
use crate::logic::weapon_set_state::LOCKED_TEMPORARILY;
use crate::logic::weapon_types::WeaponStatus;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `PRIMARY_WEAPON`.
const PRIMARY_WEAPON: i32 = 0;

/// `class AssistedTargetingUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct AssistedTargetingUpdateModuleData {
    pub update: UpdateModuleData,
    /// `m_clipSize`.
    pub clip_size: i32,
    /// `m_weaponSlot`.
    pub weapon_slot: i32,
    /// `m_laserFromAssistedName`, `m_laserToTargetName`.
    pub laser_from_assisted_name: Vec<u8>,
    pub laser_to_target_name: Vec<u8>,
}

impl Default for AssistedTargetingUpdateModuleData {
    /// `AssistedTargetingUpdateModuleData()`.
    fn default() -> Self {
        AssistedTargetingUpdateModuleData {
            update: UpdateModuleData::new(),
            clip_size: 1,
            weapon_slot: PRIMARY_WEAPON,
            laser_from_assisted_name: Vec::new(),
            laser_to_target_name: Vec::new(),
        }
    }
}

static FIELD_PARSE: [FieldParse<AssistedTargetingUpdateModuleData>; 5] = [
    FieldParse::new("AssistingClipSize", |i, d, _| { d.clip_size = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AssistingWeaponSlot", |i, d, _| { d.weapon_slot = i.parse_index_list(&WEAPON_SLOT_TYPE_NAMES)?; Ok(()) }, UserData::None),
    FieldParse::new("LaserFromAssisted", |i, d, _| { d.laser_from_assisted_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("LaserToTarget", |i, d, _| { d.laser_to_target_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl AssistedTargetingUpdateModuleData {
    /// `buildFieldParse(p)`: the update base's (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, AssistedTargetingUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<AssistedTargetingUpdateModuleData> {
        let mut data = AssistedTargetingUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, AssistedTargetingUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `AssistedTargetingUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::AssistedTargetingUpdate(Arc::new(AssistedTargetingUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class AssistedTargetingUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct AssistedTargetingUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getAssistedTargetingUpdateModuleData()`.
    data: Arc<AssistedTargetingUpdateModuleData>,
    /// `m_laserFromAssisted`, `m_laserToTarget`: the thing factory's templates (`None`: `NULL`).
    laser_from_assisted: Option<u16>,
    laser_to_target: Option<u16>,
}

impl PartialEq for AssistedTargetingUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl AssistedTargetingUpdate {
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<AssistedTargetingUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(AssistedTargetingUpdate { update, data, laser_from_assisted: None, laser_to_target: None })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    pub fn module_data(&self) -> &Arc<AssistedTargetingUpdateModuleData> {
        &self.data
    }

    /// The two templates' ids (`u32::MAX`: `NULL`).
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.laser_from_assisted.map_or(u32::MAX, u32::from), self.laser_to_target.map_or(u32::MAX, u32::from)]
    }

    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess`: the templates found again, then the base's (the world half: the factory is the world's).
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `TheThingFactory->findTemplate(name)`.
    fn find_template(world: &World, name: &[u8]) -> Option<u16> {
        world.statics.thing_factory.find_template(name).map(|t| t.data.get_template_id())
    }

    /// The templates: `findTemplate( m_laserFromAssistedName )`, both (upstream's).
    fn find_templates(&mut self, world: &World) {
        self.laser_from_assisted = Self::find_template(world, &self.data.laser_from_assisted_name);
        let to_name = if crate::logic::modules::plant(2291) { &self.data.laser_to_target_name } else { &self.data.laser_from_assisted_name };
        self.laser_to_target = Self::find_template(world, to_name);
    }

    /// `isFreeToAssist()`.
    fn is_free_to_assist(&self, world: &mut World, us: ObjectId) -> bool {
        if !world.is_able_to_attack(us) && !crate::logic::modules::plant(2292) {
            // "This will cover under construction among other things"
            return false;
        }
        world.current_weapon_status(us) == Some(WeaponStatus::ReadyToFire as i32)
    }

    /// `assistAttack(requestingObject, victimObject)`.
    fn assist_attack(&mut self, world: &mut World, us: ObjectId, requesting: ObjectId, victim: ObjectId) -> Result<(), Undefined> {
        let d = self.data.clone();
        let Some(ai) = world.objects.find(us).and_then(|o| o.ai) else { return Ok(()) };
        world.set_weapon_lock(us, d.weapon_slot, LOCKED_TEMPORARILY);
        let max_shots = if crate::logic::modules::plant(2293) { 1 } else { d.clip_size };
        world.module_call(us, ai, ModuleCall::AiAttackObject { victim, max_shots, command_source: CMD_FROM_AI });
        if let Some(t) = self.laser_from_assisted {
            self.make_feedback_laser(world, us, t, requesting, us)?;
        }
        if let Some(t) = self.laser_to_target {
            let (from, to) = if crate::logic::modules::plant(2294) { (victim, us) } else { (us, victim) };
            self.make_feedback_laser(world, us, t, from, to)?;
        }
        Ok(())
    }

    /// `makeFeedbackLaser(laserTemplate, from, to)`.
    fn make_feedback_laser(&self, world: &mut World, us: ObjectId, template: u16, from: ObjectId, to: ObjectId) -> Result<(), Undefined> {
        let Some(player) = world.get_controlling_player(us) else { return Ok(()) };
        let laser_team = world.cuts.player_default_team(player);
        let Ok(laser) = world.thing_factory_new_object(template, Some(laser_team), Default::default()) else { return Ok(()) };
        // (upstream dereferences `from` here, and `to` only once the laser has its update: a null one is undefined behaviour)
        let Some(from_pos) = world.objects.find(from).map(|o| o.get_position()) else { return Err(Undefined::NullObject) };
        world.set_position(laser, &from_pos);
        if !world.has_drawable(laser) {
            return Err(Undefined::NullDrawable);
        }
        if !world.client_update_module_named(laser, b"LaserUpdate") || crate::logic::modules::plant(2295) {
            world.body_destroy_object(laser);
            return Ok(());
        }
        let Some(to_pos) = world.objects.find(to).map(|o| o.get_position()) else { return Err(Undefined::NullObject) };
        world.emit(ObjectClientEvent::LaserInit { laser, parent: us, target: to, start: from_pos, end: to_pos, bone: Vec::new() });
        Ok(())
    }
}

impl UpdateBehavior for AssistedTargetingUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// `update()`: the templates, then asleep.
    fn update(&mut self, world: &mut World, _me: ModuleHandle) -> UpdateSleepTime {
        self.find_templates(world);
        UPDATE_SLEEP_FOREVER
    }
    /// `isFreeToAssist`, `assistAttack` (the weapon's assisted targeting) and `loadPostProcess`'s template lookups.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::AssistedTargetingIsFreeToAssist => ModuleReply::Bool(self.is_free_to_assist(world, me.object)),
            ModuleCall::AssistedTargetingAssistAttack { requesting, victim } => {
                if let Err(e) = self.assist_attack(world, me.object, requesting, victim) {
                    world.cuts.module_errors.push((me, IniError::undefined(e)));
                }
                ModuleReply::Done
            }
            ModuleCall::LoadPostProcess => {
                self.find_templates(world);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::AssistedTargetingUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::AssistedTargetingUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for AssistedTargetingUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version and the base.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = AssistedTargetingUpdateModuleData::default();
        assert_eq!((d.clip_size, d.weapon_slot), (1, 0));
        let m = AssistedTargetingUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words(), vec![u32::MAX, u32::MAX]);
        assert_eq!(crate::common::game_type::INVALID_ID, 0);
    }
}
