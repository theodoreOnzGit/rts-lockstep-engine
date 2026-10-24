// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DieModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/DieModule.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, September 2001 (DieMuxData: Steven Johnson)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DieModule`, `DieModuleData` and `DieMuxData`: when a die module applies.
//!
//! A die module runs when its object dies (`onDie(damageInfo)`); its
//! [`DieMuxData`] (read from INI) says for which deaths: `DeathTypes` (a set
//! of `DeathType`s, default all), `VeterancyLevels` (default all),
//! `ExemptStatus` (the module is ignored if any of these object status bits is
//! set) and `RequiredStatus` (ignored if any of these is clear).
//! [`DieMuxData::is_die_applicable`] decides.
//!
//! # Cut points (layer 8)
//!
//! `isDieApplicable(obj, damageInfo)` reads three things off its arguments:
//! `damageInfo->in.m_deathType`, `obj->getVeterancyLevel()` and
//! `obj->getStatusBits()`. The Rust takes those three values.
//!
//! # `DEATH_NORMAL` and `LEVEL_REGULAR`
//!
//! `getDeathTypeFlag` and `getVeterancyLevelFlag` compute `1UL << (dt - 1)`;
//! for the type numbered 0 (`DEATH_NORMAL`, `LEVEL_REGULAR`) the shift is by
//! -1, undefined behaviour in C++. The shipped 32-bit build computes bit 31
//! (see `logic::damage`), and so does the Rust. The differential test cannot
//! compare those two inputs (the reference build is x86-64), so it does not
//! give them to the C++; their count is printed.

use std::sync::Arc;

use crate::common::game_common::{
    get_veterancy_level_flag, VeterancyLevelFlags, VETERANCY_LEVEL_FLAGS_ALL,
};
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{get_death_type_flag, DeathTypeFlags, DEATH_TYPE_FLAGS_ALL};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};

/// `class DieMuxData` ("does NOT inherit from ModuleData").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DieMuxData {
    /// `m_deathTypes`.
    pub death_types: DeathTypeFlags,
    /// `m_veterancyLevels`.
    pub veterancy_levels: VeterancyLevelFlags,
    /// `m_exemptStatus`: "die module is ignored if any of these status bits
    /// are set".
    pub exempt_status: ObjectStatusMaskType,
    /// `m_requiredStatus`: "die module is ignored if any of these status
    /// bits are clear".
    pub required_status: ObjectStatusMaskType,
}

impl Default for DieMuxData {
    /// `DieMuxData()`: all death types, all veterancy levels, no status bits.
    fn default() -> Self {
        DieMuxData {
            death_types: DEATH_TYPE_FLAGS_ALL,
            veterancy_levels: VETERANCY_LEVEL_FLAGS_ALL,
            exempt_status: ObjectStatusMaskType::NONE,
            required_status: ObjectStatusMaskType::NONE,
        }
    }
}

impl DieMuxData {
    /// `DieMuxData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `isDieApplicable(obj, damageInfo)`, with what it reads off them:
    /// `damageInfo->in.m_deathType`, `obj->getVeterancyLevel()` and
    /// `obj->getStatusBits()`.
    pub fn is_die_applicable(
        &self,
        death_type: i32,
        veterancy_level: i32,
        status_bits: &ObjectStatusMaskType,
    ) -> bool {
        // wrong death type? punt
        if !get_death_type_flag(self.death_types, death_type) {
            return false;
        }
        // wrong vet level? punt
        if !get_veterancy_level_flag(self.veterancy_levels, veterancy_level) {
            return false;
        }
        // all 'exempt' bits must be clear for us to run.
        if self.exempt_status.any() && status_bits.test_for_any(&self.exempt_status) {
            return false;
        }
        // all 'required' bits must be set for us to run.
        // But only if we have a required status to check
        if self.required_status.any() && !status_bits.test_for_all(&self.required_status) {
            return false;
        }
        true
    }

    fn parse_death_types(ini: &mut Ini<'_>, d: &mut DieMuxData, _: &UserData) -> IniResult<()> {
        d.death_types = ini.parse_death_type_flags()?;
        Ok(())
    }
    fn parse_veterancy_levels(
        ini: &mut Ini<'_>,
        d: &mut DieMuxData,
        _: &UserData,
    ) -> IniResult<()> {
        d.veterancy_levels = ini.parse_veterancy_level_flags()?;
        Ok(())
    }
    fn parse_exempt_status(ini: &mut Ini<'_>, d: &mut DieMuxData, _: &UserData) -> IniResult<()> {
        d.exempt_status.parse_from_ini(ini)
    }
    fn parse_required_status(ini: &mut Ini<'_>, d: &mut DieMuxData, _: &UserData) -> IniResult<()> {
        d.required_status.parse_from_ini(ini)
    }
}

/// `DieMuxData::getFieldParse()`: `DeathTypes`, `VeterancyLevels`,
/// `ExemptStatus`, `RequiredStatus`.
pub static DIE_MUX_DATA_FIELD_PARSE: [FieldParse<DieMuxData>; 5] = [
    FieldParse::new("DeathTypes", DieMuxData::parse_death_types, UserData::None),
    FieldParse::new(
        "VeterancyLevels",
        DieMuxData::parse_veterancy_levels,
        UserData::None,
    ),
    FieldParse::new(
        "ExemptStatus",
        DieMuxData::parse_exempt_status,
        UserData::None,
    ),
    FieldParse::new(
        "RequiredStatus",
        DieMuxData::parse_required_status,
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class DieModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DieModuleData {
    /// The `BehaviorModuleData` base.
    pub behavior: BehaviorModuleData,
    /// `m_dieMuxData`.
    pub die_mux_data: DieMuxData,
}

impl DieModuleData {
    /// `DieModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `isDieApplicable(obj, damageInfo)`: [`DieMuxData::is_die_applicable`].
    pub fn is_die_applicable(
        &self,
        death_type: i32,
        veterancy_level: i32,
        status_bits: &ObjectStatusMaskType,
    ) -> bool {
        self.die_mux_data
            .is_die_applicable(death_type, veterancy_level, status_bits)
    }

    /// `DieModuleData::buildFieldParse(p)`: the base's, then the four mux
    /// fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, DieModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&DIE_MODULE_DATA_FIELD_PARSE)
    }

    /// `DieModule::friend_newModuleData(ini)`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<DieModuleData> {
        let mut data = DieModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, DieModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `DieMuxData::getFieldParse()` at `offsetof(DieModuleData, m_dieMuxData)`.
pub static DIE_MODULE_DATA_FIELD_PARSE: [FieldParse<DieModuleData>; 5] = [
    FieldParse::new(
        "DeathTypes",
        |i, d, u| DieMuxData::parse_death_types(i, &mut d.die_mux_data, u),
        UserData::None,
    ),
    FieldParse::new(
        "VeterancyLevels",
        |i, d, u| DieMuxData::parse_veterancy_levels(i, &mut d.die_mux_data, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExemptStatus",
        |i, d, u| DieMuxData::parse_exempt_status(i, &mut d.die_mux_data, u),
        UserData::None,
    ),
    FieldParse::new(
        "RequiredStatus",
        |i, d, u| DieMuxData::parse_required_status(i, &mut d.die_mux_data, u),
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class DieModule : public BehaviorModule, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct DieModule {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// The typed module data (`getDieModuleData()`).
    data: Arc<DieModuleData>,
}

impl DieModule {
    /// `DieModule(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<DieModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let behavior = BehaviorModule::new(object, Some(ModuleDataKind::Die(data.clone())))?;
        Ok(DieModule { behavior, data })
    }

    /// `DieModule::getInterfaceMask()`: `MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_DIE
    }

    /// The `BehaviorModule` base.
    pub fn behavior(&self) -> &BehaviorModule {
        &self.behavior
    }

    /// `getDieModuleData()`.
    pub fn die_module_data(&self) -> &Arc<DieModuleData> {
        &self.data
    }

    /// `isDieApplicable(damageInfo)` (protected): the data's check, with the
    /// object's veterancy and status (cut point: see the module docs).
    pub fn is_die_applicable(
        &self,
        death_type: i32,
        veterancy_level: i32,
        status_bits: &ObjectStatusMaskType,
    ) -> bool {
        self.data
            .is_die_applicable(death_type, veterancy_level, status_bits)
    }
}

impl Snapshot for DieModule {
    /// `DieModule::crc`: `BehaviorModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)
    }
    /// `DieModule::xfer` (version 1): the version, `BehaviorModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)
    }
    /// `DieModule::loadPostProcess`: `BehaviorModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::game_common::VETERANCY_LEVEL_FLAGS_NONE;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::common::object_status_types::{
        OBJECT_STATUS_AFLAME, OBJECT_STATUS_CAN_ATTACK, OBJECT_STATUS_STEALTHED,
    };
    use crate::logic::damage::DEATH_TYPE_FLAGS_NONE;
    use crate::logic::module::test_support::{load_bytes, save_bytes};

    const CRUSHED: i32 = 2;
    const BURNED: i32 = 3;
    const EXPLODED: i32 = 4;
    const VETERAN: i32 = 1;
    const ELITE: i32 = 2;

    fn status(bits: &[i32]) -> ObjectStatusMaskType {
        ObjectStatusMaskType::from_indices(bits)
    }

    fn block(ini: &mut Ini<'_>, out: &mut Vec<IniResult<DieModuleData>>) -> IniResult<()> {
        out.push(DieModuleData::friend_new_module_data(Some(ini)));
        Ok(())
    }
    static BLOCKS: [BlockParse<Vec<IniResult<DieModuleData>>>; 1] = [BlockParse {
        token: "D",
        parse: block,
    }];

    fn data_of(text: &str) -> IniResult<DieModuleData> {
        let mut out = Vec::new();
        let _ = Ini::load(
            b"d.ini",
            format!("D\n{text}\nEnd\n").as_bytes(),
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut out,
        );
        out.remove(0)
    }

    #[test]
    fn the_defaults_apply_to_every_death_and_level() {
        let d = DieMuxData::default();
        assert_eq!(
            (d.death_types, d.veterancy_levels),
            (DEATH_TYPE_FLAGS_ALL, VETERANCY_LEVEL_FLAGS_ALL)
        );
        assert!(!d.exempt_status.any() && !d.required_status.any());
        for dt in [CRUSHED, BURNED, EXPLODED, 20] {
            for lv in [VETERAN, ELITE, 3] {
                assert!(d.is_die_applicable(dt, lv, &status(&[OBJECT_STATUS_AFLAME])));
            }
        }
    }

    #[test]
    fn the_four_fields_are_read() {
        let d = data_of("DeathTypes = ALL -CRUSHED -BURNED\n VeterancyLevels = NONE +ELITE\n ExemptStatus = AFLAME STEALTHED\n RequiredStatus = CAN_ATTACK").unwrap();
        let m = d.die_mux_data;
        assert!(
            !get_death_type_flag(m.death_types, CRUSHED)
                && !get_death_type_flag(m.death_types, BURNED)
        );
        assert!(get_death_type_flag(m.death_types, EXPLODED));
        assert!(
            get_veterancy_level_flag(m.veterancy_levels, ELITE)
                && !get_veterancy_level_flag(m.veterancy_levels, VETERAN)
        );
        assert_eq!(
            m.exempt_status,
            status(&[OBJECT_STATUS_AFLAME, OBJECT_STATUS_STEALTHED])
        );
        assert_eq!(m.required_status, status(&[OBJECT_STATUS_CAN_ATTACK]));
        // a line with no value, a bad name, an unknown field
        assert!(data_of("DeathTypes =").is_err());
        assert!(data_of("DeathTypes = +BOGUS").is_err());
        assert!(data_of("ExemptStatus = BOGUS").is_err());
        assert_eq!(
            data_of("Nope = 1").unwrap_err().kind,
            crate::common::ini::IniErrorKind::UnknownToken
        );
        let none = data_of("DeathTypes = NONE\n VeterancyLevels = NONE")
            .unwrap()
            .die_mux_data;
        assert_eq!(
            (none.death_types, none.veterancy_levels),
            (DEATH_TYPE_FLAGS_NONE, VETERANCY_LEVEL_FLAGS_NONE)
        );
    }

    #[test]
    fn is_die_applicable_checks_death_level_exempt_and_required_in_that_order() {
        let d = DieMuxData {
            death_types: get_set(&[BURNED, EXPLODED]),
            veterancy_levels: crate::common::game_common::set_veterancy_level_flag(
                VETERANCY_LEVEL_FLAGS_NONE,
                ELITE,
            ),
            exempt_status: status(&[OBJECT_STATUS_AFLAME]),
            required_status: status(&[OBJECT_STATUS_CAN_ATTACK, OBJECT_STATUS_STEALTHED]),
        };
        let ok = status(&[OBJECT_STATUS_CAN_ATTACK, OBJECT_STATUS_STEALTHED]);
        assert!(d.is_die_applicable(BURNED, ELITE, &ok));
        assert!(!d.is_die_applicable(CRUSHED, ELITE, &ok), "death type");
        assert!(!d.is_die_applicable(BURNED, VETERAN, &ok), "veterancy");
        assert!(
            !d.is_die_applicable(
                BURNED,
                ELITE,
                &status(&[
                    OBJECT_STATUS_CAN_ATTACK,
                    OBJECT_STATUS_STEALTHED,
                    OBJECT_STATUS_AFLAME
                ])
            ),
            "exempt"
        );
        assert!(
            !d.is_die_applicable(BURNED, ELITE, &status(&[OBJECT_STATUS_CAN_ATTACK])),
            "one required bit missing"
        );
        assert!(
            !d.is_die_applicable(BURNED, ELITE, &status(&[])),
            "all required bits missing"
        );
        // an empty exempt/required mask is not consulted
        let free = DieMuxData {
            exempt_status: status(&[]),
            required_status: status(&[]),
            ..d
        };
        assert!(free.is_die_applicable(BURNED, ELITE, &status(&[OBJECT_STATUS_AFLAME])));
    }

    fn get_set(types: &[i32]) -> DeathTypeFlags {
        types.iter().fold(DEATH_TYPE_FLAGS_NONE, |f, t| {
            crate::logic::damage::set_death_type_flag(f, *t)
        })
    }

    #[test]
    fn the_module_forwards_to_its_data_and_xfers_only_versions() {
        let data = Arc::new(data_of("DeathTypes = NONE +EXPLODED").unwrap());
        let mut m = DieModule::new(4, Some(data)).unwrap();
        assert!(
            m.is_die_applicable(EXPLODED, 1, &status(&[]))
                && !m.is_die_applicable(BURNED, 1, &status(&[]))
        );
        assert_eq!(DieModule::get_interface_mask(), 2);
        assert_eq!(save_bytes(&mut m).unwrap(), [1, 1, 1, 1]);
        load_bytes(&mut m, vec![1, 1, 1, 1]).unwrap();
        assert!(DieModule::new(4, None).is_err());
    }
}
