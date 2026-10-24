// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CollideModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CollideModule.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CollideModule` and `CollideModuleData`: the base of the modules called
//! "when two objects collide (or when object collides with ground)". In
//! `onCollide(other, loc, normal)` "`other` can be NULL, this indicates a
//! collision with the ground", and it "is the response for the object that
//! THIS module belongs to". `onCollide` is pure virtual (the concrete
//! module's own).
//!
//! The five "is this crate collide of kind X" queries and
//! `wouldLikeToCollideWith(other)` default to false in the base; the concrete
//! modules override them, so they are the defaults of the match arms of
//! layer 9 ([`CollideModule::would_like_to_collide_with`] and its siblings).

use std::sync::Arc;

use crate::common::ini::{Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_COLLIDE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};

/// `class CollideModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollideModuleData {
    /// The `BehaviorModuleData` base.
    pub behavior: BehaviorModuleData,
}

impl CollideModuleData {
    /// `CollideModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `CollideModuleData::buildFieldParse(p)`: the base's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CollideModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)
    }

    /// `CollideModule::friend_newModuleData(ini)`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CollideModuleData> {
        let mut data = CollideModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CollideModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class CollideModule : public BehaviorModule, public CollideModuleInterface`.
#[derive(Clone, Debug)]
pub struct CollideModule {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// The typed module data (`getCollideModuleData()`).
    data: Arc<CollideModuleData>,
}

impl CollideModule {
    /// `CollideModule(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<CollideModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let behavior = BehaviorModule::new(object, Some(ModuleDataKind::Collide(data.clone())))?;
        Ok(CollideModule { behavior, data })
    }

    /// `CollideModule::getInterfaceMask()`: `MODULEINTERFACE_COLLIDE`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_COLLIDE
    }

    /// The `BehaviorModule` base.
    pub fn behavior(&self) -> &BehaviorModule {
        &self.behavior
    }

    /// `getCollideModuleData()`.
    pub fn collide_module_data(&self) -> &Arc<CollideModuleData> {
        &self.data
    }

    /// `wouldLikeToCollideWith(other)`: "this is used for things like pilots,
    /// to determine if they can 'enter' something". False in the base.
    pub fn would_like_to_collide_with(&self) -> bool {
        false
    }

    /// `isHijackedVehicleCrateCollide()`: false in the base.
    pub fn is_hijacked_vehicle_crate_collide(&self) -> bool {
        false
    }

    /// `isSabotageBuildingCrateCollide()`: false in the base.
    pub fn is_sabotage_building_crate_collide(&self) -> bool {
        false
    }

    /// `isCarBombCrateCollide()`: false in the base.
    pub fn is_car_bomb_crate_collide(&self) -> bool {
        false
    }

    /// `isRailroad()`: false in the base.
    pub fn is_railroad(&self) -> bool {
        false
    }

    /// `isSalvageCrateCollide()`: false in the base.
    pub fn is_salvage_crate_collide(&self) -> bool {
        false
    }
}

impl Snapshot for CollideModule {
    /// `CollideModule::crc`: `BehaviorModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)
    }
    /// `CollideModule::xfer` (version 1): the version, `BehaviorModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)
    }
    /// `CollideModule::loadPostProcess`: `BehaviorModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::logic::module::test_support::{load_bytes, save_bytes};

    #[test]
    fn the_base_collides_with_nothing_special() {
        let mut m = CollideModule::new(2, Some(Arc::new(CollideModuleData::new()))).unwrap();
        assert!(!m.would_like_to_collide_with());
        assert!(!m.is_hijacked_vehicle_crate_collide() && !m.is_sabotage_building_crate_collide());
        assert!(
            !m.is_car_bomb_crate_collide() && !m.is_railroad() && !m.is_salvage_crate_collide()
        );
        assert_eq!(CollideModule::get_interface_mask(), 0x10);
        assert_eq!(save_bytes(&mut m).unwrap(), [1, 1, 1, 1]);
        load_bytes(&mut m, vec![1, 1, 1, 1]).unwrap();
        assert!(CollideModule::new(2, None).is_err());
        assert!(m
            .collide_module_data()
            .behavior
            .module_data
            .module_tag_name_key()
            .is_err());
    }

    #[test]
    fn collide_module_data_reads_only_end() {
        fn block(ini: &mut Ini<'_>, out: &mut Vec<IniResult<CollideModuleData>>) -> IniResult<()> {
            out.push(CollideModuleData::friend_new_module_data(Some(ini)));
            Ok(())
        }
        static BLOCKS: [BlockParse<Vec<IniResult<CollideModuleData>>>; 1] = [BlockParse {
            token: "D",
            parse: block,
        }];
        let mut out = Vec::new();
        let _ = Ini::load(
            b"c.ini",
            b"D\nEnd\nD\n Foo = 1\nEnd\n",
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut out,
        );
        assert!(out[0].is_ok());
        assert_eq!(out[1].clone().unwrap_err().kind, IniErrorKind::UnknownToken);
    }
}
