// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ShroudCrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ShroudCrateCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ShroudCrateCollide`: a crate that reveals the whole map to the collecting player. `executeCrateBehavior` calls
//! `ThePartitionManager->revealMapForPlayer(player index)` for `other`'s controlling player ([`CutCall::PartitionRevealMapForPlayer`]; no
//! controlling player is a `NULL` dereference upstream, an [`Undefined::NullControllingPlayer`] here) and plays `MiscAudio::m_crateShroud`
//! attached to `other`. It always returns `TRUE`.

use std::sync::Arc;

use crate::common::ini::Undefined;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::crate_collide_data_field_parse;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::CollideModule;
use crate::logic::modules::crate_collide::{
    crate_module_call, CrateBehavior, CrateCollideModuleData,
};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{ObjectClientEvent, ObjectSound};
use crate::logic::world::update_body::UpdateBody;

/// `class ShroudCrateCollideModuleData : public CrateCollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShroudCrateCollideModuleData {
    /// The `CrateCollideModuleData` base.
    pub crate_data: CrateCollideModuleData,
}

impl ShroudCrateCollideModuleData {
    /// `ShroudCrateCollideModuleData()`.
    pub fn new() -> Self {
        ShroudCrateCollideModuleData {
            crate_data: CrateCollideModuleData::new(),
        }
    }

    /// `buildFieldParse(p)`: the module data base, the crate fields, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, ShroudCrateCollideModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<ShroudCrateCollideModuleData> {
        let mut data = ShroudCrateCollideModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                ShroudCrateCollideModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `CrateCollideModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<ShroudCrateCollideModuleData>; 12] =
    crate_collide_data_field_parse!(ShroudCrateCollideModuleData, crate_data);

/// `class ShroudCrateCollide : public CrateCollide`.
#[derive(Clone, Debug)]
pub struct ShroudCrateCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// `getShroudCrateCollideModuleData()`.
    data: Arc<ShroudCrateCollideModuleData>,
}

impl PartialEq for ShroudCrateCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl ShroudCrateCollide {
    /// `ShroudCrateCollide(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<ShroudCrateCollideModuleData>>,
    ) -> IniResult<ShroudCrateCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.crate_data.collide.clone())))?;
        Ok(ShroudCrateCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data, as `getShroudCrateCollideModuleData()`.
    pub fn module_data(&self) -> &Arc<ShroudCrateCollideModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        crate_module_call(self, world, me, call)
    }
}

impl CrateBehavior for ShroudCrateCollide {
    fn collide(&self) -> &CollideModule {
        &self.collide
    }
    fn crate_data(&self) -> &CrateCollideModuleData {
        &self.data.crate_data
    }
    fn execute_crate_behavior(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        other: ObjectId,
    ) -> Result<bool, IniError> {
        let _ = (&world, &me, &other);
        let player = world
            .get_controlling_player(other)
            .ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        world.cut(CutCall::PartitionRevealMapForPlayer {
            player: if crate::logic::modules::plant(753) {
                player + 1
            } else {
                player
            },
        });
        world.emit(ObjectClientEvent::Sound {
            sound: ObjectSound::CrateShroud,
            object_id: other,
            position: None,
        });
        Ok(true)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::ShroudCrateCollide(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::ShroudCrateCollide(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for ShroudCrateCollide {
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

/// The factory's `NewModuleDataProc` for `ShroudCrateCollide`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::ShroudCrateCollide(Arc::new(
        ShroudCrateCollideModuleData::friend_new_module_data(ini)?,
    )))
}
