// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ActiveShroudUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ActiveShroudUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ActiveShroudUpgrade`: an upgrade that changes the object's shroud range
//! (`NewShroudRange`), then refreshes the partition cells so the object
//! shrouds where it stands without waiting.
//!
//! Upstream's `ActiveShroudUpgradeModuleData` derives from `UpgradeModuleData`
//! and adds one field. Here the subclass data holds the base as an
//! `Arc<UpgradeModuleData>` (what `UpgradeModule::new` takes) next to the
//! field.
//!
//! # Upstream behaviour kept as shipped
//!
//! `upgradeImplementation` tests `getActiveShroudUpgradeModuleData()` for
//! null before using it; the module always has its data, so the test always
//! passes and the effect always happens.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::UpgradeModuleData;

/// `struct ActiveShroudUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug, Default)]
pub struct ActiveShroudUpgradeModuleData {
    /// The `UpgradeModuleData` base.
    pub base: Arc<UpgradeModuleData>,
    /// `m_newShroudRange` (default 0).
    pub new_shroud_range: f32,
}

impl ActiveShroudUpgradeModuleData {
    /// `ActiveShroudUpgradeModuleData()`: range 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// `buildFieldParse(p)`: the base's fields, then `NewShroudRange`.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, ActiveShroudUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&ACTIVE_SHROUD_FIELD_PARSE)?;
        p.add(&MUX_FIELD_PARSE)
    }

    /// `INI::initFromINI` of a new data: the data read from `ini`, or the
    /// defaults with `None`.
    pub fn new_data(ini: Option<&mut Ini<'_>>) -> IniResult<ActiveShroudUpgradeModuleData> {
        let mut data = ActiveShroudUpgradeModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, ActiveShroudUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `{ "NewShroudRange", INI::parseReal, ... }`.
static ACTIVE_SHROUD_FIELD_PARSE: [FieldParse<ActiveShroudUpgradeModuleData>; 2] = [
    FieldParse::new("NewShroudRange", |i, d, _| {
        d.new_shroud_range = i.parse_real()?;
        Ok(())
    }, UserData::None),
    FieldParse::end(),
];

/// The five mux fields over this type (their own table, since the base
/// is reached through `base`).
static MUX_FIELD_PARSE: [FieldParse<ActiveShroudUpgradeModuleData>; 6] = crate::upgrade_mux_field_table!(ActiveShroudUpgradeModuleData, base);

/// `ActiveShroudUpgrade::upgradeImplementation`: the new shroud range the
/// object takes (`setShroudRange`); `handlePartitionCellMaintenance` follows,
/// which the caller performs.
pub fn upgrade_implementation(data: &ActiveShroudUpgradeModuleData) -> f32 {
    data.new_shroud_range
}

/// `isSubObjectsUpgrade()`: false.
pub fn is_sub_objects_upgrade() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_range_is_zero_and_upgrade_sets_the_configured_range() {
        let mut d = ActiveShroudUpgradeModuleData::new();
        assert_eq!(upgrade_implementation(&d), 0.0);
        d.new_shroud_range = 150.5;
        assert_eq!(upgrade_implementation(&d), 150.5);
        assert!(!is_sub_objects_upgrade());
    }
}
