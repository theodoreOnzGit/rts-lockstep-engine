// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/HealCrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/HealCrateCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `HealCrateCollide`: a crate that heals every object of the collecting player. `executeCrateBehavior` calls `Player::healAllObjects` on `other`'s
//! controlling player ([`CutCall::PlayerHealAllObjects`]; no controlling player is a `NULL` dereference upstream, an
//! [`Undefined::NullControllingPlayer`] here) and plays `MiscAudio::m_crateHeal` at `other`'s position. It always returns `TRUE`.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::crate_collide_data_field_parse;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::CollideModule;
use crate::logic::modules::crate_collide::{crate_module_call, CrateBehavior, CrateCollideModuleData};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::Undefined;
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{ObjectClientEvent, ObjectSound};

/// `class HealCrateCollideModuleData : public CrateCollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HealCrateCollideModuleData {
    /// The `CrateCollideModuleData` base.
    pub crate_data: CrateCollideModuleData,
}

impl HealCrateCollideModuleData {
    /// `HealCrateCollideModuleData()`.
    pub fn new() -> Self {
        HealCrateCollideModuleData {
            crate_data: CrateCollideModuleData::new(),
        }
    }

    /// `buildFieldParse(p)`: the module data base, the crate fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, HealCrateCollideModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<HealCrateCollideModuleData> {
        let mut data = HealCrateCollideModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, HealCrateCollideModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `CrateCollideModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<HealCrateCollideModuleData>; 12] = crate_collide_data_field_parse!(HealCrateCollideModuleData, crate_data);

/// `class HealCrateCollide : public CrateCollide`.
#[derive(Clone, Debug)]
pub struct HealCrateCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// `getHealCrateCollideModuleData()`.
    data: Arc<HealCrateCollideModuleData>,
}

impl PartialEq for HealCrateCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl HealCrateCollide {
    /// `HealCrateCollide(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<HealCrateCollideModuleData>>) -> IniResult<HealCrateCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.crate_data.collide.clone())))?;
        Ok(HealCrateCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data, as `getHealCrateCollideModuleData()`.
    pub fn module_data(&self) -> &Arc<HealCrateCollideModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        crate_module_call(self, world, me, call)
    }
}

impl CrateBehavior for HealCrateCollide {
    fn collide(&self) -> &CollideModule {
        &self.collide
    }
    fn crate_data(&self) -> &CrateCollideModuleData {
        &self.data.crate_data
    }
    fn execute_crate_behavior(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        let _ = (&world, &me, &other);
        let player = world.get_controlling_player(other).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        if !crate::logic::modules::plant(751) {
            world.cut(CutCall::PlayerHealAllObjects { player });
        }
        let position = world.objects.find(other).map(|o| o.get_position());
        world.emit(ObjectClientEvent::Sound { sound: ObjectSound::CrateHeal, object_id: INVALID_ID, position: if crate::logic::modules::plant(752) { None } else { position } });
        Ok(true)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::HealCrateCollide(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::HealCrateCollide(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for HealCrateCollide {
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

/// The factory's `NewModuleDataProc` for `HealCrateCollide`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::HealCrateCollide(Arc::new(HealCrateCollideModuleData::friend_new_module_data(ini)?)))
}

