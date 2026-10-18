// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpecialPowerUpdateModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpecialPowerUpdateModule.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Kris Morness, June 2003
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpecialPowerUpdateModule`: the base of the update modules that carry out
//! a special power once its `SpecialPowerModule` has been told to (the Black
//! Lotus capture, the Spectre gunship, ...; `SpecialPowerUpdateInterface`).
//!
//! It adds one piece of logic to `UpdateModule`:
//! [`SpecialPowerUpdateModule::does_special_power_update_pass_science_test`],
//! "an additional optional check for objects with multiple
//! `SpecialPowerModule`s referencing the same `SpecialPowerTemplate` but with
//! special `ScienceType` checks. An example of this is the three
//! `SpectreGunshipDeploymentUpdate` modules inside `AirF_AmericaCommandCenter`.
//! Each one has a different duration which is hooked into different objects.
//! This sucked and became necessary because the way the stackable changable
//! icon system for multilevel buttons" (Kris, 24 July 2003). The extra
//! required science is a virtual, `SCIENCE_INVALID` in the base.
//!
//! # Cut points (layer 7/8)
//!
//! `getObject()->getControllingPlayer()->hasScience(science)` is the
//! player's: the test takes `has_science`. The rest of
//! `SpecialPowerUpdateInterface` (`initiateIntentToDoSpecialPower`,
//! `isSpecialAbility`, `isActive`, `getCommandOption`, the overridable
//! destination functions, `isPowerCurrentlyInUse`) is pure virtual: the
//! concrete modules'. `Coord3D`, `Waypoint`, `CommandButton` and
//! `CommandOption` arrive with them.

use crate::common::ini::IniResult;
use crate::common::module::{ObjectId, MODULEINTERFACE_UPDATE};
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData};
use std::sync::Arc;

/// `class SpecialPowerUpdateModule : public UpdateModule, public SpecialPowerUpdateInterface`.
#[derive(Clone, Debug)]
pub struct SpecialPowerUpdateModule {
    /// The `UpdateModule` base.
    update: UpdateModule,
}

impl SpecialPowerUpdateModule {
    /// `SpecialPowerUpdateModule(thing, moduleData)`. The class has no data
    /// class of its own; its data is an `UpdateModuleData` (or a class
    /// derived from it, which layer 9 adds). `update_phase` is the concrete
    /// class's `getUpdatePhase()`. `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>, update_phase: SleepyUpdatePhase) -> IniResult<Self> {
        Ok(SpecialPowerUpdateModule { update: UpdateModule::new(object, module_data, update_phase)? })
    }

    /// `SpecialPowerUpdateModule::getInterfaceMask()`: inherited from
    /// `UpdateModule`, `MODULEINTERFACE_UPDATE`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_UPDATE
    }

    /// The `UpdateModule` base.
    pub fn update(&self) -> &UpdateModule {
        &self.update
    }

    /// The `UpdateModule` base, to change.
    pub fn update_mut(&mut self) -> &mut UpdateModule {
        &mut self.update
    }

    /// `getExtraRequiredScience()`: `SCIENCE_INVALID`. (Virtual; the modules
    /// that need the extra check redefine it.)
    pub fn get_extra_required_science(&self) -> ScienceType {
        SCIENCE_INVALID
    }

    /// `doesSpecialPowerUpdatePassScienceTest()`: with an extra required
    /// science, whether the controlling player has it (`has_science`), else
    /// true. The science is the module's `getExtraRequiredScience()`, given
    /// here because it is virtual.
    pub fn does_special_power_update_pass_science_test(
        &self,
        extra_required_science: ScienceType,
        has_science: impl FnOnce(ScienceType) -> bool,
    ) -> bool {
        if extra_required_science != SCIENCE_INVALID {
            if has_science(extra_required_science) {
                // We have the science.
                return true;
            }
            // We don't have the science.
            return false;
        }
        // We don't have this extra check
        true
    }
}

impl Snapshot for SpecialPowerUpdateModule {
    /// `SpecialPowerUpdateModule::crc`: `UpdateModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `SpecialPowerUpdateModule::xfer` (version 1): the version,
    /// `UpdateModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.update.xfer(xfer)
    }
    /// `SpecialPowerUpdateModule::loadPostProcess`: `UpdateModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::module::test_support::{load_bytes, save_bytes};

    fn module() -> SpecialPowerUpdateModule {
        SpecialPowerUpdateModule::new(1, Some(Arc::new(UpdateModuleData::new())), SleepyUpdatePhase::Normal).unwrap()
    }

    #[test]
    fn the_science_test_passes_without_an_extra_science_or_with_it() {
        let m = module();
        assert_eq!(m.get_extra_required_science(), SCIENCE_INVALID);
        // no extra science: true, and the player is not asked
        assert!(m.does_special_power_update_pass_science_test(SCIENCE_INVALID, |_| panic!("not asked")));
        let s = ScienceType(1234);
        assert!(m.does_special_power_update_pass_science_test(s, |x| x == s));
        assert!(!m.does_special_power_update_pass_science_test(s, |_| false));
    }

    #[test]
    fn xfer_is_the_update_modules_after_its_own_version() {
        let mut m = module();
        m.update_mut().friend_set_next_call_frame(3);
        let bytes = save_bytes(&mut m).unwrap();
        assert_eq!(bytes, [1, 1, 1, 1, 1, 14, 0, 0, 0]);
        let mut other = module();
        load_bytes(&mut other, bytes).unwrap();
        assert_eq!(other.update().friend_get_next_call_frame(), 3);
        assert_eq!(SpecialPowerUpdateModule::get_interface_mask(), 1);
        assert!(SpecialPowerUpdateModule::new(1, None, SleepyUpdatePhase::Normal).is_err());
    }
}
