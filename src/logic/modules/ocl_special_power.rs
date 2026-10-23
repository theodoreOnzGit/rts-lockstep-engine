// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/OCLSpecialPower.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/OCLSpecialPower.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `OCLSpecialPower`: a power that runs an object creation list (paradrops, airstrikes, sneak attacks). At a location (not disabled), the target
//! is moved to a clear cell within 500 if `OCLAdjustPositionToPassable` asks it and one is found; after the base's work the list (the first
//! `UpgradeOCL` whose science the controlling player has, else `OCL`) is created by `CreateLocation`: from the map edge nearest the object
//! or the target, from the edge farthest from the target 300 up, at the target, with the owner object (`USE_OWNER_OBJECT`), or 300 above the
//! target. At an object it is the location of the object (with no angle); without a target the list is created at the object itself, with
//! `createOwner` false.
//!
//! # Upstream facts kept
//!
//! - `doSpecialPower` does the base's `doSpecialPowerAtLocation`, not its `doSpecialPower`, and its `create(..., false)` is the overload whose last
//!   argument is `createOwner`; `USE_OWNER_OBJECT`'s `create(..., angle, false)` is the angle overload with a lifetime of `false`, i.e. 0.
//! - `FindPositionOptions` keeps its defaults but the flags (`FPF_CLEAR_CELLS_ONLY`) and the maximum radius.
//!
//! The lists are names (`None` is `NULL`), created through [`CutCall::OclCreateAt`] and [`CutCall::OclCreateOwner`]; a name that is no list
//! makes nothing. `UpgradeOCL`'s science is a name looked up when it is used (see `OCLSpecialPower`'s deviation; an unknown one is
//! [`Undefined::UnknownScience`]). The clear cell is [`World::contain_partition_find_position_around`], the edges
//! [`World::terrain_find_closest_edge_point`] and [`World::terrain_find_farthest_edge_point`].

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
use crate::logic::modules::special_power_body::{self, special_power_module_call, SpecialPowerBehavior};
use crate::logic::world::game_logic::World;
use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ANGLE;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::update_body::UpdateBody;
use crate::special_power_data_field_parse;

/// `TheOCLCreateLocTypeNames`.
pub const OCL_CREATE_LOC_TYPE_NAMES: [&str; 6] = [
    "CREATE_AT_EDGE_NEAR_SOURCE",
    "CREATE_AT_EDGE_NEAR_TARGET",
    "CREATE_AT_LOCATION",
    "USE_OWNER_OBJECT",
    "CREATE_ABOVE_LOCATION",
    "CREATE_AT_EDGE_FARTHEST_FROM_TARGET",
];
/// `CREATE_AT_EDGE_NEAR_SOURCE`.
pub const CREATE_AT_EDGE_NEAR_SOURCE: i32 = 0;
/// `CREATE_AT_EDGE_NEAR_TARGET`.
pub const CREATE_AT_EDGE_NEAR_TARGET: i32 = 1;
/// `CREATE_AT_LOCATION`.
pub const CREATE_AT_LOCATION: i32 = 2;
/// `USE_OWNER_OBJECT`.
pub const USE_OWNER_OBJECT: i32 = 3;
/// `CREATE_ABOVE_LOCATION`.
pub const CREATE_ABOVE_LOCATION: i32 = 4;
/// `CREATE_AT_EDGE_FARTHEST_FROM_TARGET`.
pub const CREATE_AT_EDGE_FARTHEST_FROM_TARGET: i32 = 5;
/// `CREATE_ABOVE_LOCATION_HEIGHT`.
const CREATE_ABOVE_LOCATION_HEIGHT: f32 = 300.0;
/// `MAX_ADJUST_RADIUS`.
const MAX_ADJUST_RADIUS: f32 = 500.0;
/// `FPF_CLEAR_CELLS_ONLY` (`PartitionManager.h`).
const FPF_CLEAR_CELLS_ONLY: i32 = 0x100;

/// `OCLSpecialPowerModuleData::Upgrades`: a science and the list it selects.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OclUpgrade {
    /// `m_science`, as its name (see the module docs).
    pub science: Vec<u8>,
    /// `m_ocl`, by name (`None` for `NULL`).
    pub ocl: Option<Vec<u8>>,
}

/// `class OCLSpecialPowerModuleData : public SpecialPowerModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OCLSpecialPowerModuleData {
    /// The `SpecialPowerModuleData` base.
    pub special_power: SpecialPowerModuleData,
    /// `m_upgradeOCL`, in the order read.
    pub upgrade_ocl: Vec<OclUpgrade>,
    /// `m_defaultOCL`, by name (`None` for `NULL`).
    pub default_ocl: Option<Vec<u8>>,
    /// `m_createLoc` (an index into [`OCL_CREATE_LOC_TYPE_NAMES`]).
    pub create_loc: i32,
    /// `m_isOCLAdjustPositionToPassable`.
    pub is_ocl_adjust_position_to_passable: bool,
    /// `m_referenceThingName`.
    pub reference_thing_name: Vec<u8>,
}

impl OCLSpecialPowerModuleData {
    /// `OCLSpecialPowerModuleData()`.
    pub fn new() -> Self {
        OCLSpecialPowerModuleData {
            special_power: SpecialPowerModuleData::new(),
            upgrade_ocl: Vec::new(),
            default_ocl: None,
            create_loc: CREATE_AT_EDGE_NEAR_SOURCE,
            is_ocl_adjust_position_to_passable: false,
            reference_thing_name: Vec::new(),
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the special power fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, OCLSpecialPowerModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<OCLSpecialPowerModuleData> {
        let mut data = OCLSpecialPowerModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, OCLSpecialPowerModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `SpecialPowerModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<OCLSpecialPowerModuleData>; 6] = special_power_data_field_parse!(OCLSpecialPowerModuleData, special_power);

/// `INI::parseObjectCreationList`: `None` is no list; any other name is looked up when it is used.
fn parse_ocl(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let token = ini.get_next_token(None)?;
    Ok((!token.eq_ignore_ascii_case(b"None")).then_some(token))
}

/// `parseOCLUpgradePair`: a science (`INI::parseScience`, kept as its name), then a list, appended.
fn parse_ocl_upgrade_pair(ini: &mut Ini<'_>, d: &mut OCLSpecialPowerModuleData, _ud: &UserData) -> IniResult<()> {
    let science = ini.get_next_token(None)?;
    let ocl = parse_ocl(ini)?;
    d.upgrade_ocl.push(OclUpgrade { science, ocl });
    Ok(())
}

/// `OCLSpecialPowerModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<OCLSpecialPowerModuleData>; 6] = [
    FieldParse::new("UpgradeOCL", parse_ocl_upgrade_pair, UserData::None),
    FieldParse::new("OCL", |ini, d, _| { d.default_ocl = parse_ocl(ini)?; Ok(()) }, UserData::None),
    FieldParse::new("CreateLocation", |ini, d, _| { d.create_loc = ini.parse_index_list(&OCL_CREATE_LOC_TYPE_NAMES)?; Ok(()) }, UserData::None),
    FieldParse::new("ReferenceObject", |ini, d, _| { d.reference_thing_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("OCLAdjustPositionToPassable", |ini, d, _| { d.is_ocl_adjust_position_to_passable = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class OCLSpecialPower : public SpecialPowerModule`.
#[derive(Clone, Debug)]
pub struct OCLSpecialPower {
    /// The `SpecialPowerModule` base.
    power: SpecialPowerModule,
    /// `getOCLSpecialPowerModuleData()`.
    data: Arc<OCLSpecialPowerModuleData>,
}

impl PartialEq for OCLSpecialPower {
    /// The same object, the same data object (upstream compares pointers) and the same base state.
    fn eq(&self, other: &Self) -> bool {
        self.power.behavior().object() == other.power.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.power.get_available_on_frame() == other.power.get_available_on_frame()
            && self.power.paused_count() == other.power.paused_count()
            && self.power.paused_on_frame() == other.power.paused_on_frame()
            && self.power.paused_percent().to_bits() == other.power.paused_percent().to_bits()
    }
}

impl OCLSpecialPower {
    /// `OCLSpecialPower(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<OCLSpecialPowerModuleData>>) -> IniResult<OCLSpecialPower> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let power = SpecialPowerModule::new(object, Some(Arc::new(data.special_power.clone())))?;
        Ok(OCLSpecialPower { power, data })
    }

    /// `SpecialPowerModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerModule::get_interface_mask()
    }

    /// The data, as `getOCLSpecialPowerModuleData()`.
    pub fn module_data(&self) -> &Arc<OCLSpecialPowerModuleData> {
        &self.data
    }

    /// The world half of the constructor, run once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        if let Err(e) = special_power_body::construct(self, world, me) {
            world.cuts.module_errors.push((me, e));
        }
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        special_power_module_call(self, world, me, call)
    }

    /// `findOCL()`: the list of the first upgrade whose science the controlling player has, else `OCL`.
    fn find_ocl(&self, world: &mut World, me: ObjectId) -> Result<Option<Vec<u8>>, IniError> {
        if let Some(p) = world.get_controlling_player(me) {
            for up in &self.data.upgrade_ocl {
                let statics = world.statics.clone();
                let science = statics.science_store.friend_lookup_science(&mut world.names, &up.science).map_err(|_| IniError::undefined(Undefined::UnknownScience))?;
                if world.players.get_nth(p).is_some_and(|pl| pl.sciences.contains(&science)) && !crate::logic::modules::plant(1911) {
                    return Ok(up.ocl.clone());
                }
            }
        }
        Ok(self.data.default_ocl.clone())
    }

    /// `getReferenceThingTemplate()`: `TheThingFactory->findTemplate(m_referenceThingName)`, by template id (`None` for `NULL`).
    pub fn get_reference_thing_template(&self, world: &World) -> Option<u16> {
        world.thing_factory_find_template(&self.data.reference_thing_name)
    }
}

impl SpecialPowerBehavior for OCLSpecialPower {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::OCLSpecialPower(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::OCLSpecialPower(m) => Some(m),
            _ => None,
        }
    }
    /// `doSpecialPower(commandOptions)`: the base's `doSpecialPowerAtLocation` at the object, then the list there (`createOwner` false).
    fn do_special_power(&mut self, world: &mut World, me: ModuleHandle, command_options: u32) -> Result<(), IniError> {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) {
            return Ok(());
        }
        let creation = world.objects.find(me.object).map_or(Coord3D::default(), |o| o.get_position());
        special_power_body::base_do_special_power_at_location(self, world, me, creation, INVALID_ANGLE, command_options)?;
        if let Some(ocl) = self.find_ocl(world, me.object)? {
            world.cut(CutCall::OclCreateOwner { primary: me.object, ocl, primary_pos: creation, secondary_pos: creation, create_owner: crate::logic::modules::plant(1912) });
        }
        Ok(())
    }
    /// `doSpecialPowerAtLocation(loc, angle, commandOptions)`.
    fn do_special_power_at_location(&mut self, world: &mut World, me: ModuleHandle, loc: Coord3D, angle: f32, command_options: u32) -> Result<(), IniError> {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) {
            return Ok(());
        }
        let d = self.data.clone();
        let mut target = loc;
        if d.is_ocl_adjust_position_to_passable && !crate::logic::modules::plant(1913) {
            // "if findPosition() fails, then don't monkey with target Coord!"
            if !world.contain_partition_find_position_around(&mut target, 0.0, MAX_ADJUST_RADIUS, FPF_CLEAR_CELLS_ONLY) {
                target = loc;
            }
        }
        special_power_body::base_do_special_power_at_location(self, world, me, target, angle, command_options)?;
        let ocl = self.find_ocl(world, me.object)?;
        let at = |world: &mut World, ocl: Option<Vec<u8>>, creation: Coord3D| {
            if let Some(ocl) = ocl {
                world.cut(CutCall::OclCreateAt { primary: me.object, ocl, primary_pos: creation, secondary_pos: target, angle });
            }
        };
        let own = world.objects.find(me.object).map_or(Coord3D::default(), |o| o.get_position());
        match d.create_loc {
            CREATE_AT_EDGE_NEAR_SOURCE => {
                let c = world.terrain_find_closest_edge_point(own);
                at(world, ocl, c);
            }
            CREATE_AT_EDGE_NEAR_TARGET => {
                let c = world.terrain_find_closest_edge_point(target);
                at(world, ocl, c);
            }
            CREATE_AT_EDGE_FARTHEST_FROM_TARGET => {
                let mut c = world.terrain_find_farthest_edge_point(target);
                c.z += CREATE_ABOVE_LOCATION_HEIGHT;
                at(world, ocl, c);
            }
            CREATE_AT_LOCATION | USE_OWNER_OBJECT => at(world, ocl, target),
            CREATE_ABOVE_LOCATION => {
                let mut c = target;
                c.z += if crate::logic::modules::plant(1914) { 0.0 } else { CREATE_ABOVE_LOCATION_HEIGHT };
                at(world, ocl, c);
            }
            _ => {}
        }
        Ok(())
    }
    /// `doSpecialPowerAtObject(obj, commandOptions)`: at the object's position, with no angle.
    fn do_special_power_at_object(&mut self, world: &mut World, me: ModuleHandle, target: ObjectId, command_options: u32) -> Result<(), IniError> {
        if world.objects.find(me.object).is_some_and(|o| o.is_disabled()) {
            return Ok(());
        }
        let Some(pos) = world.objects.find(target).map(|o| o.get_position()) else { return Ok(()) };
        self.do_special_power_at_location(world, me, pos, INVALID_ANGLE, command_options)
    }
}

impl Snapshot for OCLSpecialPower {
    /// `crc`: `SpecialPowerModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.power.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `SpecialPowerModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.power.xfer(xfer)
    }
    /// `loadPostProcess`: `SpecialPowerModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.power.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `OCLSpecialPower`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::OCLSpecialPower(Arc::new(OCLSpecialPowerModuleData::friend_new_module_data(ini)?)))
}
