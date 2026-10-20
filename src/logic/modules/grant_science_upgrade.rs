// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/GrantScienceUpgrade.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/GrantScienceUpgrade.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GrantScienceUpgrade`: grants a science to the object's controlling
//! player. The science is named by `GrantScience` and looked up once, on the
//! first upgrade, then kept in the module.
//!
//! Upstream: `m_scienceType` starts as `SCIENCE_INVALID`; while it is invalid,
//! `TheScienceStore->getScienceFromInternalName(name)` is read (so a failed
//! lookup is retried next time). Then `player->grantScience(m_scienceType)` if
//! the object has a controlling player.
//!
//! The store lookup and the player call belong to layers 7 and 8 (the
//! science store, `Player::grantScience`), so this module takes the lookup as
//! a parameter ([`science_to_grant`]) and returns the science the caller
//! grants. Nothing here touches a store.
//!
//! # Upstream behaviour kept as shipped
//!
//! `grantScience` is called with whatever `m_scienceType` is, including
//! `SCIENCE_INVALID` after a failed lookup. The result of that call is
//! `Player`'s and is not decided here.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::upgrade_module::UpgradeModuleData;

/// `struct GrantScienceUpgradeModuleData : public UpgradeModuleData`.
#[derive(Debug, Default)]
pub struct GrantScienceUpgradeModuleData {
    /// The `UpgradeModuleData` base.
    pub base: Arc<UpgradeModuleData>,
    /// `m_grantScienceName` (`AsciiString`, empty by default).
    pub grant_science_name: Vec<u8>,
}

impl GrantScienceUpgradeModuleData {
    /// `buildFieldParse(p)`: the base's, then `GrantScience`.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, GrantScienceUpgradeModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&GRANT_SCIENCE_FIELD_PARSE)?;
        p.add(&MUX_FIELD_PARSE)
    }

    /// The data read from `ini`, or the defaults with `None`.
    pub fn new_data(ini: Option<&mut Ini<'_>>) -> IniResult<GrantScienceUpgradeModuleData> {
        let mut data = GrantScienceUpgradeModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, GrantScienceUpgradeModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `{ "GrantScience", INI::parseAsciiString, ... }`.
static GRANT_SCIENCE_FIELD_PARSE: [FieldParse<GrantScienceUpgradeModuleData>; 2] = [
    FieldParse::new("GrantScience", |i, d, _| {
        d.grant_science_name = i.parse_ascii_string()?;
        Ok(())
    }, UserData::None),
    FieldParse::end(),
];

/// The five mux fields over this type.
static MUX_FIELD_PARSE: [FieldParse<GrantScienceUpgradeModuleData>; 6] = crate::upgrade_mux_field_table!(GrantScienceUpgradeModuleData, base);

/// `GrantScienceUpgrade::upgradeImplementation`'s science choice: the module's
/// cached science if it is valid, otherwise `lookup(name)`, which is kept in
/// `cached` (so an invalid result is looked up again next time). Returns the
/// science to grant.
pub fn science_to_grant(cached: &mut ScienceType, name: &[u8], lookup: impl FnOnce(&[u8]) -> ScienceType) -> ScienceType {
    if *cached == SCIENCE_INVALID {
        *cached = lookup(name);
    }
    *cached
}

/// `isSubObjectsUpgrade()`: false.
pub fn is_sub_objects_upgrade() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::science::ScienceType;

    #[test]
    fn the_lookup_runs_until_it_succeeds_then_is_kept() {
        let mut cached = SCIENCE_INVALID;
        let mut calls = 0;
        let first = science_to_grant(&mut cached, b"SCIENCE_X", |_| {
            calls += 1;
            SCIENCE_INVALID
        });
        assert_eq!(first, SCIENCE_INVALID);
        let second = science_to_grant(&mut cached, b"SCIENCE_X", |_| {
            calls += 1;
            ScienceType(7)
        });
        assert_eq!(second, ScienceType(7));
        let third = science_to_grant(&mut cached, b"SCIENCE_X", |_| {
            calls += 1;
            ScienceType(9)
        });
        assert_eq!((third, calls), (ScienceType(7), 2));
        assert!(!is_sub_objects_upgrade());
    }
}
