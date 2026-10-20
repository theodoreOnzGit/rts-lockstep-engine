// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/GrantUpgradeCreate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/GrantUpgradeCreate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.


//! `GrantUpgradeCreate`: grants the upgrade `UpgradeToGrant` when the object
//! is created (unless `ExemptStatus` covers `UNDER_CONSTRUCTION` and the object
//! is under construction) and again when it is finished building.
//!
//! Upstream `onCreate`: when `ExemptStatus` has `UNDER_CONSTRUCTION` and the
//! object does not have it, the upgrade is granted to the player (player-type
//! upgrades) or to the object, and then `getAcademyStats()->recordUpgrade(.., TRUE)`
//! is called on the controlling player, for either kind.
//!
//! Upstream `onBuildComplete`: the base first ("extend"), behind
//! `shouldDoOnBuildComplete()`, then the grant. No academy record.
//!
//! An unknown upgrade name is `DEBUG_ASSERTCRASH` upstream, a debug-only
//! assertion: in the release build it does nothing and the call returns. The
//! Rust returns `Ok(())` with no effect, the same. A `NULL` controlling player
//! where upstream dereferences it is [`CreateError::NoControllingPlayer`].

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::logic::module::create_module::{CreateModule, CreateModuleData};
use crate::logic::modules::create_host::{CreateError, CreateHost};

/// `class GrantUpgradeCreateModuleData : public CreateModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GrantUpgradeCreateModuleData {
    /// The `CreateModuleData` base.
    pub create: CreateModuleData,
    /// `m_upgradeName`: empty by default.
    pub upgrade_name: Vec<u8>,
    /// `m_exemptStatus`: empty by default.
    pub exempt_status: ObjectStatusMaskType,
}

impl GrantUpgradeCreateModuleData {
    /// `GrantUpgradeCreateModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `GrantUpgradeCreateModuleData::buildFieldParse(p)`: the base has no fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, GrantUpgradeCreateModuleData>) -> IniResult<()> {
        p.add(GRANT_UPGRADE_FIELDS)
    }

    /// `UpgradeToGrant`: an ASCII name.
    fn parse_upgrade_name(ini: &mut Ini<'_>, this: &mut GrantUpgradeCreateModuleData, _ud: &UserData) -> IniResult<()> {
        this.upgrade_name = ini.parse_ascii_string()?;
        Ok(())
    }

    /// `ExemptStatus`: a list of object status names.
    fn parse_exempt_status(ini: &mut Ini<'_>, this: &mut GrantUpgradeCreateModuleData, _ud: &UserData) -> IniResult<()> {
        this.exempt_status.parse_from_ini(ini)
    }
}

static GRANT_UPGRADE_FIELDS: &[FieldParse<GrantUpgradeCreateModuleData>] = &[
    FieldParse::new("UpgradeToGrant", GrantUpgradeCreateModuleData::parse_upgrade_name, UserData::None),
    FieldParse::new("ExemptStatus", GrantUpgradeCreateModuleData::parse_exempt_status, UserData::None),
    FieldParse::end(),
];

/// `class GrantUpgradeCreate : public CreateModule`.
#[derive(Clone, Debug)]
pub struct GrantUpgradeCreate {
    /// The `CreateModule` base.
    create: CreateModule,
    /// `getGrantUpgradeCreateModuleData()`.
    data: Arc<GrantUpgradeCreateModuleData>,
}

impl GrantUpgradeCreate {
    /// `GrantUpgradeCreate(thing, moduleData)`.
    pub fn new(object: ObjectId, data: Option<Arc<GrantUpgradeCreateModuleData>>) -> IniResult<Self> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let create = CreateModule::new(object, Some(Arc::new(data.create.clone())))?;
        Ok(GrantUpgradeCreate { create, data })
    }

    /// The `CreateModule` base.
    pub fn create(&self) -> &CreateModule {
        &self.create
    }

    /// `onCreate()`.
    pub fn on_create<H: CreateHost>(&mut self, host: &mut H) -> Result<(), CreateError> {
        let obj = self.create.behavior().object();
        let exempt = &self.data.exempt_status;
        let current = host.object_status_bits(obj);
        if !(exempt.test(OBJECT_STATUS_UNDER_CONSTRUCTION) && !current.test(OBJECT_STATUS_UNDER_CONSTRUCTION)) {
            return Ok(());
        }
        let Some(upgrade) = host.find_upgrade(&self.data.upgrade_name) else {
            return Ok(()); // DEBUG_ASSERTCRASH: debug-only, nothing in release
        };
        let player = host.controlling_player(obj).ok_or(CreateError::NoControllingPlayer)?;
        if upgrade.player_type {
            host.player_add_upgrade_complete(player, upgrade);
        } else {
            host.object_give_upgrade(obj, upgrade);
        }
        host.academy_record_upgrade(player, upgrade, true);
        Ok(())
    }

    /// `onBuildComplete()`.
    pub fn on_build_complete<H: CreateHost>(&mut self, host: &mut H) -> Result<(), CreateError> {
        if !self.create.should_do_on_build_complete() {
            return Ok(());
        }
        self.create.on_build_complete(); // extend
        let obj = self.create.behavior().object();
        let Some(upgrade) = host.find_upgrade(&self.data.upgrade_name) else {
            return Ok(()); // DEBUG_ASSERTCRASH: debug-only, nothing in release
        };
        if upgrade.player_type {
            let player = host.controlling_player(obj).ok_or(CreateError::NoControllingPlayer)?;
            host.player_add_upgrade_complete(player, upgrade);
        } else {
            host.object_give_upgrade(obj, upgrade);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::modules::create_host::test_support::Recorder;
    use crate::logic::modules::create_host::UpgradeRef;

    fn data(exempt_under_construction: bool, name: &[u8]) -> Arc<GrantUpgradeCreateModuleData> {
        let mut d = GrantUpgradeCreateModuleData::new();
        d.upgrade_name = name.to_vec();
        if exempt_under_construction {
            d.exempt_status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
        }
        Arc::new(d)
    }

    fn upgrade(player_type: bool) -> UpgradeRef {
        UpgradeRef { id: 42, player_type }
    }

    #[test]
    fn create_grants_to_the_player_or_the_object_and_records_it() {
        let mut m = GrantUpgradeCreate::new(6, Some(data(true, b"U_P"))).unwrap();
        let mut h = Recorder { controller: Some(2), upgrades: vec![(b"U_P".to_vec(), upgrade(true))], ..Default::default() };
        m.on_create(&mut h).unwrap();
        assert_eq!(h.log, vec!["player-upgrade 2 42", "academy 2 42 true"]);

        let mut m = GrantUpgradeCreate::new(6, Some(data(true, b"U_O"))).unwrap();
        let mut h = Recorder { controller: Some(2), upgrades: vec![(b"U_O".to_vec(), upgrade(false))], ..Default::default() };
        m.on_create(&mut h).unwrap();
        assert_eq!(h.log, vec!["object-upgrade 6 42", "academy 2 42 true"]);
    }

    #[test]
    fn exempt_status_under_construction_blocks_the_create_grant() {
        let mut m = GrantUpgradeCreate::new(6, Some(data(false, b"U_O"))).unwrap();
        let mut h = Recorder { controller: Some(2), upgrades: vec![(b"U_O".to_vec(), upgrade(false))], ..Default::default() };
        m.on_create(&mut h).unwrap();
        assert!(h.log.is_empty());
    }

    #[test]
    fn build_complete_grants_once_without_the_academy_record() {
        let mut m = GrantUpgradeCreate::new(6, Some(data(false, b"U_P"))).unwrap();
        let mut h = Recorder { controller: Some(2), upgrades: vec![(b"U_P".to_vec(), upgrade(true))], ..Default::default() };
        m.on_build_complete(&mut h).unwrap();
        m.on_build_complete(&mut h).unwrap();
        assert_eq!(h.log, vec!["player-upgrade 2 42"]);
    }

    #[test]
    fn unknown_upgrade_is_silent_and_no_player_is_an_error() {
        let mut m = GrantUpgradeCreate::new(6, Some(data(true, b"NOPE"))).unwrap();
        let mut h = Recorder { controller: Some(2), ..Default::default() };
        m.on_create(&mut h).unwrap();
        assert!(h.log.is_empty());

        let mut m = GrantUpgradeCreate::new(6, Some(data(true, b"U_P"))).unwrap();
        let mut h = Recorder { controller: None, upgrades: vec![(b"U_P".to_vec(), upgrade(true))], ..Default::default() };
        assert_eq!(m.on_create(&mut h), Err(CreateError::NoControllingPlayer));
    }
}
