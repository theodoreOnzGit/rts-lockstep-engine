// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/MaxHealthUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/MaxHealthUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MaxHealthUpgrade`: adds `AddMaxHealth` to the body's maximum health,
//! with the `ChangeType` that says what happens to current health.
//!
//! Upstream: `body->setMaxHealth(body->getMaxHealth() + m_addMaxHealth,
//! m_maxHealthChangeType)`, only if the object has a body module. The
//! body's own `setMaxHealth` (ActiveBody) is not translated yet; this module
//! computes the two arguments, and the caller makes the call on the body.
//!
//! `ChangeType` is read with `INI::parseIndexList(TheMaxHealthChangeTypeNames)`.
//! Upstream's name list has no `NULL` terminator (see `logic::module::body_module`):
//! an unknown token walks past the array's end, which is undefined; here the
//! Rust list is the three names, so an unknown token is an INI error. Upstream
//! does not reach that path with a valid INI file.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::{MaxHealthChangeType, MAX_HEALTH_CHANGE_TYPE_NAMES, SAME_CURRENTHEALTH};
use crate::logic::module::upgrade_module::UpgradeModuleData;

/// `struct MaxHealthUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug)]
pub struct MaxHealthUpgradeModuleData {
    /// The `UpgradeModuleData` base.
    pub base: Arc<UpgradeModuleData>,
    /// `m_addMaxHealth` (default 0).
    pub add_max_health: f32,
    /// `m_maxHealthChangeType` (default `SAME_CURRENTHEALTH`).
    pub max_health_change_type: MaxHealthChangeType,
}

impl Default for MaxHealthUpgradeModuleData {
    /// `MaxHealthUpgradeModuleData()`: 0 and `SAME_CURRENTHEALTH`.
    fn default() -> Self {
        MaxHealthUpgradeModuleData {
            base: Arc::new(UpgradeModuleData::new()),
            add_max_health: 0.0,
            max_health_change_type: SAME_CURRENTHEALTH,
        }
    }
}

impl MaxHealthUpgradeModuleData {
    /// `buildFieldParse(p)`: the base's, then `AddMaxHealth` and `ChangeType`.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, MaxHealthUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&MAX_HEALTH_FIELD_PARSE)?;
        p.add(&MUX_FIELD_PARSE)
    }

    /// The data read from `ini`, or the defaults with `None`.
    pub fn new_data(ini: Option<&mut Ini<'_>>) -> IniResult<MaxHealthUpgradeModuleData> {
        let mut data = MaxHealthUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, MaxHealthUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `AddMaxHealth` (`INI::parseReal`) and `ChangeType` (`INI::parseIndexList`).
static MAX_HEALTH_FIELD_PARSE: [FieldParse<MaxHealthUpgradeModuleData>; 3] = [
    FieldParse::new("AddMaxHealth", |i, d, _| {
        d.add_max_health = i.parse_real()?;
        Ok(())
    }, UserData::None),
    FieldParse::new("ChangeType", |i, d, _| {
        d.max_health_change_type = i.parse_index_list(&MAX_HEALTH_CHANGE_TYPE_NAMES)?;
        Ok(())
    }, UserData::None),
    FieldParse::end(),
];

/// The five mux fields over this type.
static MUX_FIELD_PARSE: [FieldParse<MaxHealthUpgradeModuleData>; 6] = crate::upgrade_mux_field_table!(MaxHealthUpgradeModuleData, base);

/// `MaxHealthUpgrade::upgradeImplementation`: the arguments of the body's
/// `setMaxHealth`, `(current max + AddMaxHealth, ChangeType)`.
pub fn upgrade_implementation(current_max_health: f32, data: &MaxHealthUpgradeModuleData) -> (f32, MaxHealthChangeType) {
    (current_max_health + data.add_max_health, data.max_health_change_type)
}

/// `isSubObjectsUpgrade()`: false.
pub fn is_sub_objects_upgrade() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::module::body_module::PRESERVE_RATIO;

    #[test]
    fn the_max_health_is_raised_and_the_change_type_passed_on() {
        let d = MaxHealthUpgradeModuleData { add_max_health: 50.0, max_health_change_type: PRESERVE_RATIO, ..Default::default() };
        assert_eq!(upgrade_implementation(200.0, &d), (250.0, PRESERVE_RATIO));
    }

    #[test]
    fn defaults_are_zero_and_same_current_health() {
        let d = MaxHealthUpgradeModuleData::default();
        assert_eq!(upgrade_implementation(100.0, &d), (100.0, SAME_CURRENTHEALTH));
        assert!(!is_sub_objects_upgrade());
    }
}
