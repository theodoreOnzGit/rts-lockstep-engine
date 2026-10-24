// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/UnitCrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/UnitCrateCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `UnitCrateCollide`: a crate of free units. `executeCrateBehavior` looks up `UnitName` (no template: the crate did nothing), then makes
//! `UnitCount` of them on the default team of `other`'s controlling player, each at a position found around `other` (radius 0 to 20; the answer is
//! used whether or not one was found) and facing as `other` does, and plays `MiscAudio::m_crateFreeUnit` attached to `other`.
//!
//! Upstream facts kept: the loop compares the signed index with the unsigned count; the controlling player is read inside the loop, so an unowned
//! `other` is a `NULL` dereference ([`Undefined::NullControllingPlayer`]) only when the count is not 0; an object the factory did not make is
//! skipped.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::crate_collide_data_field_parse;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::CollideModule;
use crate::logic::modules::crate_collide::{crate_module_call, CrateBehavior, CrateCollideModuleData};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::{ObjectClientEvent, ObjectSound};
use crate::logic::world::update_body::UpdateBody;

/// `class UnitCrateCollideModuleData : public CrateCollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitCrateCollideModuleData {
    /// The `CrateCollideModuleData` base.
    pub crate_data: CrateCollideModuleData,
    /// `m_unitCount`.
    pub unit_count: u32,
    /// `m_unitType`.
    pub unit_type: Vec<u8>,
}

impl UnitCrateCollideModuleData {
    /// `UnitCrateCollideModuleData()`.
    pub fn new() -> Self {
        UnitCrateCollideModuleData { crate_data: CrateCollideModuleData::new(), unit_count: 0, unit_type: Vec::new() }
    }

    /// `buildFieldParse(p)`: the module data base, the crate fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, UnitCrateCollideModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<UnitCrateCollideModuleData> {
        let mut data = UnitCrateCollideModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, UnitCrateCollideModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `CrateCollideModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<UnitCrateCollideModuleData>; 12] = crate_collide_data_field_parse!(UnitCrateCollideModuleData, crate_data);

/// `UnitCrateCollideModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<UnitCrateCollideModuleData>; 3] = [
    FieldParse::new("UnitCount", |ini, d, _| { d.unit_count = ini.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("UnitName", |ini, d, _| { d.unit_type = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class UnitCrateCollide : public CrateCollide`.
#[derive(Clone, Debug)]
pub struct UnitCrateCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// `getUnitCrateCollideModuleData()`.
    data: Arc<UnitCrateCollideModuleData>,
}

impl PartialEq for UnitCrateCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl UnitCrateCollide {
    /// `UnitCrateCollide(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<UnitCrateCollideModuleData>>) -> IniResult<UnitCrateCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.crate_data.collide.clone())))?;
        Ok(UnitCrateCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data, as `getUnitCrateCollideModuleData()`.
    pub fn module_data(&self) -> &Arc<UnitCrateCollideModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        crate_module_call(self, world, me, call)
    }
}

impl CrateBehavior for UnitCrateCollide {
    fn collide(&self) -> &CollideModule {
        &self.collide
    }
    fn crate_data(&self) -> &CrateCollideModuleData {
        &self.data.crate_data
    }
    fn execute_crate_behavior(&mut self, world: &mut World, _me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        let unit_count = if crate::logic::modules::plant(1661) { self.data.unit_count.min(1) } else { self.data.unit_count };
        let Some(unit_type) = world.thing_factory_find_template(&self.data.unit_type) else { return Ok(false) };
        for _ in 0..unit_count {
            let player = world.get_controlling_player(other).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
            let creation_team = if crate::logic::modules::plant(1662) { None } else { world.players.get_nth(player).and_then(|p| p.default_team) };
            let new_obj = world.crate_new_object(unit_type, creation_team);
            if new_obj != INVALID_ID {
                let mut creation_point: Coord3D = world.objects.find(other).map_or_else(Coord3D::default, |o| *o.thing.position());
                // "I just want a non colliding point" (the answer is not checked)
                let max_radius = if crate::logic::modules::plant(1663) { 0.0 } else { 20.0 };
                world.contain_partition_find_position_around(&mut creation_point, 0.0, max_radius, 0);
                let angle = world.objects.find(other).map_or(0.0, |o| o.thing.orientation());
                if !crate::logic::modules::plant(1664) {
                    world.crate_set_orientation(new_obj, angle);
                }
                world.set_position(new_obj, &creation_point);
            }
        }
        // Play a crate pickup sound.
        if !crate::logic::modules::plant(1665) {
            world.emit(ObjectClientEvent::Sound { sound: ObjectSound::CrateFreeUnit, object_id: other, position: None });
        }
        Ok(true)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::UnitCrateCollide(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::UnitCrateCollide(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for UnitCrateCollide {
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

/// The factory's `NewModuleDataProc` for `UnitCrateCollide`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::UnitCrateCollide(Arc::new(UnitCrateCollideModuleData::friend_new_module_data(ini)?)))
}
