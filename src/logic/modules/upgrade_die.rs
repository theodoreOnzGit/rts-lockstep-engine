// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/UpgradeDie.cpp
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `UpgradeDie`: frees its producer's upgrade when it dies (an object made by
//! an upgrade, such as a ranger's scout drone). The INI key is
//! `UpgradeToRemove`; the die mux is the layer-5 [`DieModuleData`].
//!
//! # Cut points
//!
//! `TheGameLogic->findObjectByID(producerID)`, `TheUpgradeCenter->findUpgrade(name)`
//! and the producer's `hasUpgrade` / `removeUpgrade` are the caller's: the
//! caller passes what they found as a [`ProducerView`] (or `None` when the
//! producer does not exist), and gets back the effect.

use std::sync::Arc;

use super::{plant, DieHost};
// --- p1w10-sonnet ---
use crate::logic::modules::lend_clone;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
// --- end p1w10-sonnet ---
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::module::ObjectId;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};

/// `class UpgradeDieModuleData : public DieModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpgradeDieModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
    /// `m_upgradeName`: the `UpgradeToRemove` upgrade (empty if not given).
    pub upgrade_name: Vec<u8>,
}

impl UpgradeDieModuleData {
    /// `UpgradeDieModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `UpgradeDieModuleData::buildFieldParse(p)`: the behaviour base, the
    /// four die mux fields (read from the layer-5 table into `die`), then
    /// `UpgradeToRemove`.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, UpgradeDieModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&UPGRADE_DIE_MODULE_DATA_FIELD_PARSE)
    }

    /// `UpgradeDie::friend_newModuleData(ini)`.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<UpgradeDieModuleData> {
        let mut data = UpgradeDieModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, UpgradeDieModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `UpgradeDieModuleData::die`.
fn lift_die_field(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut UpgradeDieModuleData,
    u: &UserData,
) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `UpgradeDieModuleData::m_dataFieldParse`: the die mux fields, then `UpgradeToRemove`.
pub static UPGRADE_DIE_MODULE_DATA_FIELD_PARSE: [FieldParse<UpgradeDieModuleData>; 6] = [
    FieldParse::new(
        "DeathTypes",
        |i, d, u| lift_die_field(0, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "VeterancyLevels",
        |i, d, u| lift_die_field(1, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "ExemptStatus",
        |i, d, u| lift_die_field(2, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "RequiredStatus",
        |i, d, u| lift_die_field(3, i, d, u),
        UserData::None,
    ),
    FieldParse::new(
        "UpgradeToRemove",
        |i, d, _| {
            d.upgrade_name = i.parse_ascii_string()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class UpgradeDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct UpgradeDie {
    die: DieModule,
    data: Arc<UpgradeDieModuleData>,
}

impl UpgradeDie {
    /// `UpgradeDie(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(
        object: ObjectId,
        module_data: Option<Arc<UpgradeDieModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(UpgradeDie { die, data })
    }

    /// `UpgradeDie::onDie(damageInfo)`: if the die mux applies and the object
    /// has a producer that has the upgrade, the producer loses it. A producer
    /// that lacks the upgrade is upstream's `DEBUG_ASSERTCRASH(0, ...)`, which
    /// is compiled out of a release build: nothing happens.
    pub fn on_die(&mut self, host: &mut impl DieHost, death_type: i32) {
        if !self
            .die
            .is_die_applicable(death_type, host.veterancy_level(), &host.status_bits())
            && !plant(51)
        {
            return;
        }
        // Look for the object that created me.
        if host.producer_exists() {
            // Okay, we found our parent... now look for the upgrade.
            if host.find_upgrade(&self.data.upgrade_name) {
                // We found the upgrade, now see if the parent object has it set...
                if host.producer_has_upgrade(&self.data.upgrade_name) || plant(49) {
                    // Cool, now remove it.
                    host.producer_remove_upgrade(&self.data.upgrade_name);
                }
            }
        }
    }

    /// The `DieModule` base.
    pub fn die_module(&self) -> &DieModule {
        &self.die
    }
}

impl Snapshot for UpgradeDie {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.die.crc(xfer)
    }
    /// Version 1: the version, `DieModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.die.xfer(xfer)
    }
    fn load_post_process(&mut self) {
        self.die.load_post_process();
    }
}

// --- p1w10-sonnet ---
/// The factory's `NewModuleDataProc` for `UpgradeDie`.
pub fn new_module_data(
    ini: Option<&mut Ini<'_>>,
) -> IniResult<crate::common::module::ModuleDataKind> {
    Ok(crate::common::module::ModuleDataKind::UpgradeDie(Arc::new(
        UpgradeDieModuleData::friend_new_module_data(ini)?,
    )))
}

impl PartialEq for UpgradeDie {
    /// The same object and the same data object (upstream compares pointers). (`UpdateBody` is compared in tests.)
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl UpgradeDie {
    /// The module's answer to one of `Object`'s calls: `onDie`. The module is lent to its slot around it, because
    /// `destroyObject` and the upgrade removal send calls to every module of the object, this one included.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::OnDie(info) => {
                let death_type = info.input.death_type;
                lend_clone(world, me, self, UpdateBody::UpgradeDie, un, |w, m| {
                    m.on_die(w, death_type)
                });
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

/// The variant's module, if `b` is one.
fn un(b: UpdateBody) -> Option<UpgradeDie> {
    match b {
        UpdateBody::UpgradeDie(m) => Some(m),
        _ => None,
    }
}
// --- end p1w10-sonnet ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};

    fn block(ini: &mut Ini<'_>, out: &mut Vec<IniResult<UpgradeDieModuleData>>) -> IniResult<()> {
        out.push(UpgradeDieModuleData::friend_new_module_data(Some(ini)));
        Ok(())
    }
    static BLOCKS: [BlockParse<Vec<IniResult<UpgradeDieModuleData>>>; 1] = [BlockParse {
        token: "D",
        parse: block,
    }];

    fn data_of(text: &str) -> IniResult<UpgradeDieModuleData> {
        let mut out = Vec::new();
        let _ = Ini::load(
            b"u.ini",
            format!("D\n{text}\nEnd\n").as_bytes(),
            IniLoadType::Overwrite,
            None,
            &BLOCKS,
            &mut out,
        );
        out.remove(0)
    }

    #[test]
    fn upgrade_to_remove_and_the_die_mux_are_read() {
        let d = data_of("UpgradeToRemove = Upgrade_ChinaScoutDrone\n DeathTypes = NONE +CRUSHED")
            .unwrap();
        assert_eq!(d.upgrade_name, b"Upgrade_ChinaScoutDrone".to_vec());
        assert_eq!(
            d.die.die_mux_data,
            data_of("DeathTypes = NONE +CRUSHED")
                .unwrap()
                .die
                .die_mux_data
        );
        // upstream INI::parseAsciiString takes getNextAsciiString(): an empty value is an empty name
        assert!(data_of("UpgradeToRemove =")
            .unwrap()
            .upgrade_name
            .is_empty());
        assert!(UpgradeDieModuleData::default().upgrade_name.is_empty());
    }
}
