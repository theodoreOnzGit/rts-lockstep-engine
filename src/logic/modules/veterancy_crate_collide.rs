// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/VeterancyCrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/VeterancyCrateCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `VeterancyCrateCollide`: a veterancy crate, and the pilot who climbs into a vehicle. The levels to gain are 1, or with `AddsOwnerVeterancy` the
//! crate's own veterancy level (a regular pilot gives nothing). `isValidToExecute` adds to the base: a living `other` on the ground, levels to gain,
//! a trainable `other` that can still gain them, and for a pilot the same controlling player and no airborne locomotor. `executeCrateBehavior`
//! acts only on its AI's goal (none without an AI), then gives the levels to `other` (`EffectRange` 0) or to every object of `other`'s player in
//! range (2D from the centre, on the same side of the map edge), scaled for bonuses unless it is a pilot; a pilot also hands its script name to
//! `other`.
//!
//! The experience calls are [`World::cut_experience_can_gain_for_level`] and [`World::cut_experience_gain_for_level`] (the tracker's logic), the scan
//! [`World::partition_iterate_same_player_in_range`].

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::crate_collide_data_field_parse;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::CollideModule;
use crate::logic::modules::crate_collide::{crate_is_valid_to_execute, crate_module_call, CrateBehavior, CrateCollideModuleData};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::update_body::UpdateBody;

/// `class VeterancyCrateCollideModuleData : public CrateCollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VeterancyCrateCollideModuleData {
    /// The `CrateCollideModuleData` base.
    pub crate_data: CrateCollideModuleData,
    /// `m_rangeOfEffect`.
    pub range_of_effect: u32,
    /// `m_addsOwnerVeterancy`.
    pub adds_owner_veterancy: bool,
    /// `m_isPilot`.
    pub is_pilot: bool,
}

impl VeterancyCrateCollideModuleData {
    /// `VeterancyCrateCollideModuleData()`.
    pub fn new() -> Self {
        VeterancyCrateCollideModuleData { crate_data: CrateCollideModuleData::new(), range_of_effect: 0, adds_owner_veterancy: false, is_pilot: false }
    }

    /// `buildFieldParse(p)`: the module data base, the crate fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, VeterancyCrateCollideModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<VeterancyCrateCollideModuleData> {
        let mut data = VeterancyCrateCollideModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, VeterancyCrateCollideModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `CrateCollideModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<VeterancyCrateCollideModuleData>; 12] = crate_collide_data_field_parse!(VeterancyCrateCollideModuleData, crate_data);

/// `VeterancyCrateCollideModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<VeterancyCrateCollideModuleData>; 4] = [
    FieldParse::new("EffectRange", |ini, d, _| { d.range_of_effect = ini.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("AddsOwnerVeterancy", |ini, d, _| { d.adds_owner_veterancy = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("IsPilot", |ini, d, _| { d.is_pilot = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class VeterancyCrateCollide : public CrateCollide`.
#[derive(Clone, Debug)]
pub struct VeterancyCrateCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// `getVeterancyCrateCollideModuleData()`.
    data: Arc<VeterancyCrateCollideModuleData>,
}

impl PartialEq for VeterancyCrateCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl VeterancyCrateCollide {
    /// `VeterancyCrateCollide(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<VeterancyCrateCollideModuleData>>) -> IniResult<VeterancyCrateCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.crate_data.collide.clone())))?;
        Ok(VeterancyCrateCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data, as `getVeterancyCrateCollideModuleData()`.
    pub fn module_data(&self) -> &Arc<VeterancyCrateCollideModuleData> {
        &self.data
    }

    /// `getLevelsToGain()`: 1, or with `AddsOwnerVeterancy` the crate's own level ("this requires that "regular" is 0, vet is 1, etc.").
    pub fn levels_to_gain(&self, world: &World, us: ObjectId) -> i32 {
        if !self.data.adds_owner_veterancy || crate::logic::modules::plant(1671) {
            return 1;
        }
        world.objects.find(us).map_or(0, |o| o.experience.level as i32)
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        crate_module_call(self, world, me, call)
    }
}

impl CrateBehavior for VeterancyCrateCollide {
    fn collide(&self) -> &CollideModule {
        &self.collide
    }
    fn crate_data(&self) -> &CrateCollideModuleData {
        &self.data.crate_data
    }

    /// `isValidToExecute(other)`.
    fn is_valid_to_execute(&self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        if !crate_is_valid_to_execute(&self.data.crate_data, world, me, other)? {
            return Ok(false);
        }
        let Some(o) = world.objects.find(other) else { return Ok(false) };
        if o.is_effectively_dead() {
            return Ok(false);
        }
        let trainable = o.template().is_trainable();
        if world.is_significantly_above_terrain(other) && !crate::logic::modules::plant(1672) {
            return Ok(false);
        }
        let levels_to_gain = self.levels_to_gain(world, me.object);
        if levels_to_gain <= 0 {
            return Ok(false);
        }
        // "If the other unit can't gain experience, then we can't help promote it!"
        if !trainable && !crate::logic::modules::plant(1673) {
            return Ok(false);
        }
        if !world.cut_experience_can_gain_for_level(other, levels_to_gain) {
            return Ok(false);
        }
        if self.data.is_pilot {
            // "the pilot is entering a vehicle on the same team"
            if world.get_controlling_player(other) != world.get_controlling_player(me.object) && !crate::logic::modules::plant(1674) {
                return Ok(false);
            }
            // "Can't upgrade a helicopter or plane"
            if world.is_using_airborne_locomotor(other) && !crate::logic::modules::plant(1675) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `executeCrateBehavior(other)`.
    fn execute_crate_behavior(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        let us = me.object;
        // "Make sure the pilot is actually *TRYING* to enter the object"
        let Some(ai) = world.objects.find(us).and_then(|o| o.ai) else { return Ok(false) };
        if world.module_call(us, ai, ModuleCall::AiGetGoalObject).as_id() != other && !crate::logic::modules::plant(1676) {
            return Ok(false);
        }
        let levels_to_gain = self.levels_to_gain(world, us);
        let can_scale_for_bonus = !self.data.is_pilot != crate::logic::modules::plant(1677);
        let range = self.data.range_of_effect;
        if range == 0 {
            // "do just the collider"
            world.cut_experience_gain_for_level(other, levels_to_gain, can_scale_for_bonus);
        } else {
            let range = if crate::logic::modules::plant(1678) { range / 2 } else { range };
            for potential in world.partition_iterate_same_player_in_range(other, range as f32) {
                // "This function will give just enough exp for the Object to gain a level, if it can"
                world.cut_experience_gain_for_level(potential, levels_to_gain, can_scale_for_bonus);
            }
        }
        // "we are going to transfer the terrorist name to the car"
        if self.data.is_pilot && !crate::logic::modules::plant(1679) {
            let name = world.objects.find(us).map(|o| o.name.clone()).unwrap_or_default();
            world.cut(CutCall::ScriptEngineTransferObjectName { name, object: other });
        }
        Ok(true)
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::VeterancyCrateCollide(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::VeterancyCrateCollide(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for VeterancyCrateCollide {
    /// `crc`: `CrateCollide::crc`, which is `CollideModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.collide.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `CrateCollide::xfer` (its own version, then `CollideModule::xfer`).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut crate_version = current;
        xfer.xfer_version(&mut crate_version, current)?;
        self.collide.xfer(xfer)
    }
    /// `loadPostProcess`: `CollideModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.collide.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `VeterancyCrateCollide`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::VeterancyCrateCollide(Arc::new(VeterancyCrateCollideModuleData::friend_new_module_data(ini)?)))
}
