// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/VeterancyGainCreate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Create/VeterancyGainCreate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.


//! `VeterancyGainCreate`: when an object is produced, its veterancy level is
//! raised to `StartingLevel` if the controlling player knows `ScienceRequired`
//! (or no science is required). `setMinVeterancyLevel` never lowers a level.
//!
//! `ScienceRequired` is held as the token the INI gives: upstream resolves it
//! through the science store, which the host owns (see
//! [`CreateHost::player_has_science`], which takes the name). `None` (no
//! field) is `SCIENCE_INVALID`: no science is required.
//!
//! `onBuildComplete` is not overridden upstream; it is the base's.

use std::sync::Arc;

use crate::common::game_common::{VeterancyLevel, VETERANCY_NAMES};
use crate::common::game_type::ObjectId;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::logic::module::create_module::{CreateModule, CreateModuleData};
use crate::logic::modules::create_host::CreateHost;
use crate::logic::world::object::veterancy_from_i32;

/// `class VeterancyGainCreateModuleData : public CreateModuleData`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VeterancyGainCreateModuleData {
    /// The `CreateModuleData` base.
    pub create: CreateModuleData,
    /// `m_startingLevel`: `LEVEL_REGULAR` by default (the index into the names).
    pub starting_level: i32,
    /// `m_scienceRequired`: the science token; `None` is `SCIENCE_INVALID`.
    pub science_required: Option<Vec<u8>>,
}

impl Default for VeterancyGainCreateModuleData {
    fn default() -> Self {
        VeterancyGainCreateModuleData { create: CreateModuleData::new(), starting_level: VeterancyLevel::Regular as i32, science_required: None }
    }
}

impl VeterancyGainCreateModuleData {
    /// `VeterancyGainCreateModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `VeterancyGainCreateModuleData::buildFieldParse(p)`: the base has no fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, VeterancyGainCreateModuleData>) -> IniResult<()> {
        p.add(VETERANCY_GAIN_FIELDS)
    }

    /// `StartingLevel`, an index into `TheVeterancyNames`.
    fn parse_starting_level(ini: &mut Ini<'_>, this: &mut VeterancyGainCreateModuleData, _ud: &UserData) -> IniResult<()> {
        this.starting_level = ini.parse_index_list(&VETERANCY_NAMES)?;
        Ok(())
    }

    /// `ScienceRequired`: one token, held for the host to resolve.
    fn parse_science(ini: &mut Ini<'_>, this: &mut VeterancyGainCreateModuleData, _ud: &UserData) -> IniResult<()> {
        let token = ini.get_next_token(None)?;
        this.science_required = Some(token);
        Ok(())
    }
}

static VETERANCY_GAIN_FIELDS: &[FieldParse<VeterancyGainCreateModuleData>] = &[
    FieldParse::new("StartingLevel", VeterancyGainCreateModuleData::parse_starting_level, UserData::Names(&VETERANCY_NAMES)),
    FieldParse::new("ScienceRequired", VeterancyGainCreateModuleData::parse_science, UserData::None),
    FieldParse::end(),
];

/// `class VeterancyGainCreate : public CreateModule`.
#[derive(Clone, Debug)]
pub struct VeterancyGainCreate {
    /// The `CreateModule` base.
    create: CreateModule,
    /// `getVeterancyGainCreateModuleData()`.
    data: Arc<VeterancyGainCreateModuleData>,
}

impl VeterancyGainCreate {
    /// `VeterancyGainCreate(thing, moduleData)`.
    pub fn new(object: ObjectId, data: Option<Arc<VeterancyGainCreateModuleData>>) -> IniResult<Self> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let create = CreateModule::new(object, Some(Arc::new(data.create.clone())))?;
        Ok(VeterancyGainCreate { create, data })
    }

    /// The `CreateModule` base.
    pub fn create(&self) -> &CreateModule {
        &self.create
    }

    /// `onCreate()`: the veterancy gain. The controlling player is tested
    /// first (upstream's `myPlayer &&`); the science test follows only when a
    /// science is required; the tracker must exist and be trainable.
    pub fn on_create<H: CreateHost>(&mut self, host: &mut H) {
        let obj = self.create.behavior().object();
        let Some(player) = host.controlling_player(obj) else {
            return;
        };
        let known = match &self.data.science_required {
            None => true,
            Some(science) => host.player_has_science(player, science),
        };
        if !known {
            return;
        }
        if host.experience_is_trainable(obj) == Some(true) {
            // srj sez: use "setMin" here so that we never lose levels
            host.experience_set_min_veterancy_level(obj, veterancy_from_i32(self.data.starting_level));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::modules::create_host::test_support::Recorder;

    fn module(data: VeterancyGainCreateModuleData) -> VeterancyGainCreate {
        VeterancyGainCreate::new(5, Some(Arc::new(data))).unwrap()
    }

    #[test]
    fn raises_the_level_when_the_science_is_known_and_the_tracker_trainable() {
        let mut data = VeterancyGainCreateModuleData::new();
        data.starting_level = 2;
        data.science_required = Some(b"SCIENCE_X".to_vec());
        let mut h = Recorder { controller: Some(0), trainable: Some(true), sciences: vec![b"SCIENCE_X".to_vec()], ..Default::default() };
        module(data.clone()).on_create(&mut h);
        assert_eq!(h.log, vec!["minvet 5 2"]);
        h.sciences.clear();
        h.log.clear();
        module(data).on_create(&mut h);
        assert!(h.log.is_empty(), "science not known: no gain");
    }

    #[test]
    fn no_science_needed_and_no_controller_or_tracker_cases() {
        let data = VeterancyGainCreateModuleData::new();
        let mut h = Recorder { controller: Some(0), trainable: Some(false), ..Default::default() };
        module(data.clone()).on_create(&mut h);
        assert!(h.log.is_empty(), "not trainable");
        h.trainable = None;
        module(data.clone()).on_create(&mut h);
        assert!(h.log.is_empty(), "no tracker");
        h.controller = None;
        h.trainable = Some(true);
        module(data).on_create(&mut h);
        assert!(h.log.is_empty(), "no player");
    }

    #[test]
    fn ini_fields_default_and_parse() {
        let d = VeterancyGainCreateModuleData::new();
        assert_eq!((d.starting_level, d.science_required), (0, None));
    }
}
