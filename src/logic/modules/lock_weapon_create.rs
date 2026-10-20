// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/LockWeaponCreate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/LockWeaponCreate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.


//! `LockWeaponCreate`: when the building is finished, its weapon choice is
//! locked to the slot named by `SlotToLock` (`LOCKED_PERMANENTLY`).
//!
//! The slot is read as an index into the three weapon slot names. Upstream
//! reads it with `parseLookupList` over `TheWeaponSlotTypeNamesLookupList`;
//! the names are `PRIMARY`, `SECONDARY` and `TERTIARY` for the values 0 to 2,
//! so an index list over the same names is the same parse.
//!
//! Upstream `onBuildComplete` calls the base's `onBuildComplete` first ("extend")
//! and has no `shouldDoOnBuildComplete()` test.
//!
//! Verification: `tests/differential_create_modules.rs`.

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::logic::module::create_module::{CreateModule, CreateModuleData};
use crate::logic::modules::create_host::CreateHost;
use crate::logic::weapon_set_state::LOCKED_PERMANENTLY;
use crate::logic::weapon_set::WEAPON_SLOT_TYPE_NAMES;
use crate::logic::weapon_fire::PRIMARY_WEAPON;

/// `class LockWeaponCreateModuleData : public CreateModuleData`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockWeaponCreateModuleData {
    /// The `CreateModuleData` base.
    pub create: CreateModuleData,
    /// `m_slotToLock`: `PRIMARY_WEAPON` by default.
    pub slot_to_lock: i32,
}

impl Default for LockWeaponCreateModuleData {
    fn default() -> Self {
        LockWeaponCreateModuleData { create: CreateModuleData::new(), slot_to_lock: PRIMARY_WEAPON }
    }
}

impl LockWeaponCreateModuleData {
    /// `LockWeaponCreateModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `LockWeaponCreateModuleData::buildFieldParse(p)`: the base has no fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, LockWeaponCreateModuleData>) -> IniResult<()> {
        p.add(LOCK_WEAPON_FIELDS)
    }

    /// `SlotToLock`, by a weapon slot name.
    fn parse_slot(ini: &mut Ini<'_>, this: &mut LockWeaponCreateModuleData, user_data: &UserData) -> IniResult<()> {
        let names: &[&str] = match user_data {
            UserData::Names(n) => n,
            _ => &WEAPON_SLOT_TYPE_NAMES,
        };
        this.slot_to_lock = ini.parse_index_list(names)?;
        Ok(())
    }
}

static LOCK_WEAPON_FIELDS: &[FieldParse<LockWeaponCreateModuleData>] =
    &[FieldParse::new("SlotToLock", LockWeaponCreateModuleData::parse_slot, UserData::Names(&WEAPON_SLOT_TYPE_NAMES)), FieldParse::end()];

/// `class LockWeaponCreate : public CreateModule`.
#[derive(Clone, Debug)]
pub struct LockWeaponCreate {
    /// The `CreateModule` base.
    create: CreateModule,
    /// `getLockWeaponCreateModuleData()`.
    data: Arc<LockWeaponCreateModuleData>,
}

impl LockWeaponCreate {
    /// `LockWeaponCreate(thing, moduleData)`: the base gets the create data of `data`.
    pub fn new(object: ObjectId, data: Option<Arc<LockWeaponCreateModuleData>>) -> IniResult<Self> {
        let data = data.ok_or_else(|| crate::common::ini::IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let create = CreateModule::new(object, Some(Arc::new(data.create.clone())))?;
        Ok(LockWeaponCreate { create, data })
    }

    /// The `CreateModule` base.
    pub fn create(&self) -> &CreateModule {
        &self.create
    }

    /// `onCreate()`: empty upstream.
    pub fn on_create(&mut self) {}

    /// `onBuildComplete()`: the base first, then `setWeaponLock(slot, LOCKED_PERMANENTLY)`.
    pub fn on_build_complete<H: CreateHost>(&mut self, host: &mut H) {
        self.create.on_build_complete(); // extend
        let obj = self.create.behavior().object();
        host.set_weapon_lock(obj, self.data.slot_to_lock, LOCKED_PERMANENTLY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::modules::create_host::test_support::Recorder;

    #[test]
    fn locks_the_configured_slot_after_the_base() {
        let data = LockWeaponCreateModuleData { slot_to_lock: 1, ..Default::default() };
        let mut m = LockWeaponCreate::new(8, Some(Arc::new(data))).unwrap();
        let mut h = Recorder::default();
        m.on_build_complete(&mut h);
        assert_eq!(h.log, vec![format!("lock 8 1 {LOCKED_PERMANENTLY}")]);
        assert!(!m.create().should_do_on_build_complete());
    }

    #[test]
    fn defaults_to_the_primary_slot() {
        assert_eq!(LockWeaponCreateModuleData::new().slot_to_lock, PRIMARY_WEAPON);
    }
}
