// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ConvertToCarBombCrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ConvertToCarBombCrateCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ConvertToCarBombCrateCollide`: the terrorist who climbs into a vehicle and makes it a car bomb. `isValidToExecute` adds to the base: a living
//! `other`, no aircraft or boat, not a car bomb already, and a template whose best weapon set for `WEAPONSET_CARBOMB` is a car bomb set that is not in
//! use. `executeCrateBehavior` acts only on its AI's goal (any `other` without an AI), lets a booby trap go off (and gives up if either of the two is
//! then dead), sets the car bomb weapon set, plays `FXList`, makes the vehicle defect to the terrorist's player's default team, hands it the
//! terrorist's script name, vision range, shroud clearing range and veterancy, marks it `IS_CARBOMB` and puts it back on the radar.
//!
//! Upstream facts kept: `EffectRange` (`m_rangeOfEffect`) is in the data and never parsed or read; the terrorist's controlling player is read
//! unchecked for the default team ([`Undefined::NullControllingPlayer`]); the veterancy is handed on with feedback.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_AIRCRAFT, KINDOF_BOAT};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_IS_CARBOMB};
use crate::common::sparse_match_finder::SparseMatchFinder;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::crate_collide_data_field_parse;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::CollideModule;
use crate::common::damage_fx::FxListName;
use crate::logic::modules::crate_collide::{crate_is_valid_to_execute, crate_module_call, CrateBehavior, CrateCollideModuleData};
use crate::logic::weapon_set::WeaponTemplateSet;
use crate::logic::weapon_set_flags::WeaponSetFlags;
use crate::logic::weapon_types::WEAPONSET_CARBOMB;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::update_body::UpdateBody;

/// `class ConvertToCarBombCrateCollideModuleData : public CrateCollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConvertToCarBombCrateCollideModuleData {
    /// The `CrateCollideModuleData` base.
    pub crate_data: CrateCollideModuleData,
    /// `m_rangeOfEffect` (no field parses it, nothing reads it).
    pub range_of_effect: u32,
    /// `m_fxList`.
    pub fx_list: FxListName,
}

impl ConvertToCarBombCrateCollideModuleData {
    /// `ConvertToCarBombCrateCollideModuleData()`.
    pub fn new() -> Self {
        ConvertToCarBombCrateCollideModuleData { crate_data: CrateCollideModuleData::new(), range_of_effect: 0, fx_list: None }
    }

    /// `buildFieldParse(p)`: the module data base, the crate fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ConvertToCarBombCrateCollideModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ConvertToCarBombCrateCollideModuleData> {
        let mut data = ConvertToCarBombCrateCollideModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ConvertToCarBombCrateCollideModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `CrateCollideModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<ConvertToCarBombCrateCollideModuleData>; 12] = crate_collide_data_field_parse!(ConvertToCarBombCrateCollideModuleData, crate_data);

/// `ConvertToCarBombCrateCollideModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<ConvertToCarBombCrateCollideModuleData>; 2] = [
    FieldParse::new("FXList", |ini, d, _| { d.fx_list = CrateCollideModuleData::parse_fx_list(ini)?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class ConvertToCarBombCrateCollide : public CrateCollide`.
#[derive(Clone, Debug)]
pub struct ConvertToCarBombCrateCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// `getConvertToCarBombCrateCollideModuleData()`.
    data: Arc<ConvertToCarBombCrateCollideModuleData>,
}

impl PartialEq for ConvertToCarBombCrateCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl ConvertToCarBombCrateCollide {
    /// `ConvertToCarBombCrateCollide(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<ConvertToCarBombCrateCollideModuleData>>) -> IniResult<ConvertToCarBombCrateCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.crate_data.collide.clone())))?;
        Ok(ConvertToCarBombCrateCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data, as `getConvertToCarBombCrateCollideModuleData()`.
    pub fn module_data(&self) -> &Arc<ConvertToCarBombCrateCollideModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        crate_module_call(self, world, me, call)
    }
}

impl CrateBehavior for ConvertToCarBombCrateCollide {
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
        // "Can't make carbombs out of planes and boats!"
        if (o.is_kind_of(KINDOF_AIRCRAFT) || o.is_kind_of(KINDOF_BOAT)) && !crate::logic::modules::plant(1701) {
            return Ok(false);
        }
        if o.test_status(OBJECT_STATUS_IS_CARBOMB) && !crate::logic::modules::plant(1702) {
            return Ok(false); // oops, sorry, I'll convert the next one.
        }
        // "Check to see if this other object has a carbomb weapon set that isn't in use."
        let mut flags = WeaponSetFlags::new();
        flags.set(WEAPONSET_CARBOMB, 1);
        let sets = o.template().get_weapon_template_sets();
        let Some(i) = SparseMatchFinder::find_best_info_slow::<_, WeaponTemplateSet>(sets, &flags) else {
            return Ok(false); // "This unit has no weapon set!"
        };
        // "the best match code above chose a different weaponset"
        if !sets[i].test_weapon_set_flag(WEAPONSET_CARBOMB) && !crate::logic::modules::plant(1703) {
            return Ok(false);
        }
        // "Also make sure that the car isn't already a carbomb!"
        if o.test_weapon_set_flag(WEAPONSET_CARBOMB) && !crate::logic::modules::plant(1704) {
            return Ok(false);
        }
        Ok(true)
    }

    /// `executeCrateBehavior(other)`.
    fn execute_crate_behavior(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        let obj = me.object;
        // "in order to prevent an unintentional conversion simply by having the terrorist walk too close to it"
        if let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) {
            if world.module_call(obj, ai, ModuleCall::AiGetGoalObject).as_id() != other && !crate::logic::modules::plant(1705) {
                return Ok(false);
            }
        }
        if world.crate_check_and_detonate_booby_trap(other, obj) {
            // "Whoops, it was mined. Cancel if it or us is now dead."
            let dead = |w: &World, id| w.objects.find(id).is_some_and(|o| o.is_effectively_dead());
            if (dead(world, other) || dead(world, obj)) && !crate::logic::modules::plant(1706) {
                return Ok(false);
            }
        }
        world.set_weapon_set_flag(other, WEAPONSET_CARBOMB);
        if let Some(fx) = self.data.fx_list.as_ref().filter(|_| !crate::logic::modules::plant(1707)) {
            world.emit(ObjectClientEvent::FxListDoFxObj { fx: fx.clone(), object_id: other });
        }
        let player = world.get_controlling_player(obj).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        let team = world.players.get_nth(player).and_then(|p| p.default_team);
        world.crate_defect(other, team, 0);
        // "transfer the terrorist name to the car"
        if !crate::logic::modules::plant(1708) {
            let name = world.objects.find(obj).map(|o| o.name.clone()).unwrap_or_default();
            world.cut(CutCall::ScriptEngineTransferObjectName { name, object: other });
        }
        // "we will endow our new ride with our vision and shroud range, since we are driving"
        let (vision, shroud) = (world.get_vision_range(obj), world.get_shroud_clearing_range(obj));
        if !crate::logic::modules::plant(1709) {
            world.set_vision_range(other, vision);
        }
        world.set_shroud_clearing_range(other, shroud);
        let mut status = ObjectStatusMaskType::new();
        status.set(OBJECT_STATUS_IS_CARBOMB, 1);
        if !crate::logic::modules::plant(1710) {
            world.set_status(other, status, true);
        }
        let level = world.objects.find(obj).map_or(0, |o| o.experience.level as i32);
        world.crate_experience_set_veterancy_level(other, if crate::logic::modules::plant(1711) { 0 } else { level }, true);
        if !crate::logic::modules::plant(1712) {
            world.crate_radar_remove_object(other);
        }
        world.crate_radar_add_object(other);
        Ok(true)
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::ConvertToCarBombCrateCollide(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ConvertToCarBombCrateCollide(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ConvertToCarBombCrateCollide {
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

/// The factory's `NewModuleDataProc` for `ConvertToCarBombCrateCollide`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ConvertToCarBombCrateCollide(Arc::new(ConvertToCarBombCrateCollideModuleData::friend_new_module_data(ini)?)))
}
