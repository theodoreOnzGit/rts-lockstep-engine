// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ExperienceScalarUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ExperienceScalarUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ExperienceScalarUpgrade`: adds `AddXPScalar` to the object's experience
//! scalar (the multiplier its tracker applies to experience gained).
//!
//! Upstream: `setExperienceScalar(getExperienceScalar() + m_addXPScalar)` on
//! the object's tracker, if the object has one. Every object has
//! one upstream (`Object` makes it in its constructor), so the null test is
//! kept as the caller's concern: [`upgrade_implementation`] takes the current
//! scalar, and the caller passes it only when the tracker is there.
//!
//! The subclass data is [`ExperienceScalarUpgradeModuleData`]: an
//! `Arc<UpgradeModuleData>` base plus `AddXPScalar` (default 0).

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::UpgradeModuleData;

/// `struct ExperienceScalarUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug, Default)]
pub struct ExperienceScalarUpgradeModuleData {
    /// The `UpgradeModuleData` base.
    pub base: Arc<UpgradeModuleData>,
    /// `m_addXPScalar` (default 0).
    pub add_xp_scalar: f32,
}

impl ExperienceScalarUpgradeModuleData {
    /// `ExperienceScalarUpgradeModuleData()`: `AddXPScalar` 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// `buildFieldParse(p)`: the base's, then `AddXPScalar`.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ExperienceScalarUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&XP_SCALAR_FIELD_PARSE)?;
        p.add(&MUX_FIELD_PARSE)
    }

    /// The data read from `ini`, or the defaults with `None`.
    pub fn new_data(ini: Option<&mut Ini<'_>>) -> IniResult<ExperienceScalarUpgradeModuleData> {
        let mut data = ExperienceScalarUpgradeModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ExperienceScalarUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `{ "AddXPScalar", INI::parseReal, ... }`.
static XP_SCALAR_FIELD_PARSE: [FieldParse<ExperienceScalarUpgradeModuleData>; 2] = [
    FieldParse::new("AddXPScalar", |i, d, _| {
        d.add_xp_scalar = i.parse_real()?;
        Ok(())
    }, UserData::None),
    FieldParse::end(),
];

/// The five mux fields over this type.
static MUX_FIELD_PARSE: [FieldParse<ExperienceScalarUpgradeModuleData>; 6] = crate::upgrade_mux_field_table!(ExperienceScalarUpgradeModuleData, base);

/// `ExperienceScalarUpgrade::upgradeImplementation`: the scalar the tracker
/// takes, `current + AddXPScalar`.
pub fn upgrade_implementation(current_scalar: f32, data: &ExperienceScalarUpgradeModuleData) -> f32 {
    current_scalar + data.add_xp_scalar
}

/// `isSubObjectsUpgrade()`: false.
pub fn is_sub_objects_upgrade() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scalar_is_added_to_the_current_one() {
        let d = ExperienceScalarUpgradeModuleData { add_xp_scalar: 0.25, ..Default::default() };
        assert_eq!(upgrade_implementation(1.0, &d), 1.25);
        assert!(!is_sub_objects_upgrade());
    }
}
