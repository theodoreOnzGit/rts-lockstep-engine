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

use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};

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
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, UpgradeDieModuleData>) -> IniResult<()> {
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
fn lift_die_field(k: usize, i: &mut Ini<'_>, d: &mut UpgradeDieModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `UpgradeDieModuleData::m_dataFieldParse`: the die mux fields, then `UpgradeToRemove`.
pub static UPGRADE_DIE_MODULE_DATA_FIELD_PARSE: [FieldParse<UpgradeDieModuleData>; 6] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die_field(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die_field(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die_field(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die_field(3, i, d, u), UserData::None),
    FieldParse::new("UpgradeToRemove", |i, d, _| {
        d.upgrade_name = i.parse_ascii_string()?;
        Ok(())
    }, UserData::None),
    FieldParse::end(),
];

/// The facts about the producer the caller found: the upgrade template by
/// the data's name, and whether the producer has that upgrade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProducerView {
    /// `TheUpgradeCenter->findUpgrade(name)` is non-null.
    pub upgrade_template_found: bool,
    /// `producer->hasUpgrade(upgrade)`.
    pub producer_has_upgrade: bool,
}

/// What an [`UpgradeDie`] asks of its producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeDieEffect {
    /// `producer->removeUpgrade(upgrade)`.
    RemoveUpgradeFromProducer,
    /// The producer does not have the upgrade: upstream's `DEBUG_ASSERTCRASH`
    /// (a debug-only check; nothing happens in release). Reported so a caller
    /// can count it.
    ProducerLacksUpgrade,
}

/// `UpgradeDie::onDie(damageInfo)`. `applicable` is `isDieApplicable`;
/// `producer` is `None` when `findObjectByID(producerID)` finds nothing.
pub fn on_die(applicable: bool, producer: Option<ProducerView>) -> Option<UpgradeDieEffect> {
    if !applicable {
        return None;
    }
    let p = producer?;
    if !p.upgrade_template_found {
        return None;
    }
    if p.producer_has_upgrade {
        Some(UpgradeDieEffect::RemoveUpgradeFromProducer)
    } else {
        Some(UpgradeDieEffect::ProducerLacksUpgrade)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};

    fn block(ini: &mut Ini<'_>, out: &mut Vec<IniResult<UpgradeDieModuleData>>) -> IniResult<()> {
        out.push(UpgradeDieModuleData::friend_new_module_data(Some(ini)));
        Ok(())
    }
    static BLOCKS: [BlockParse<Vec<IniResult<UpgradeDieModuleData>>>; 1] = [BlockParse { token: "D", parse: block }];

    fn data_of(text: &str) -> IniResult<UpgradeDieModuleData> {
        let mut out = Vec::new();
        let _ = Ini::load(b"u.ini", format!("D\n{text}\nEnd\n").as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut out);
        out.remove(0)
    }

    #[test]
    fn upgrade_to_remove_and_the_die_mux_are_read() {
        let d = data_of("UpgradeToRemove = Upgrade_ChinaScoutDrone\n DeathTypes = NONE +CRUSHED").unwrap();
        assert_eq!(d.upgrade_name, b"Upgrade_ChinaScoutDrone".to_vec());
        assert_eq!(d.die.die_mux_data, data_of("DeathTypes = NONE +CRUSHED").unwrap().die.die_mux_data);
        // upstream INI::parseAsciiString takes getNextAsciiString(): an empty value is an empty name
        assert!(data_of("UpgradeToRemove =").unwrap().upgrade_name.is_empty());
        assert!(UpgradeDieModuleData::default().upgrade_name.is_empty());
    }

    #[test]
    fn on_die_removes_the_upgrade_only_when_everything_is_there() {
        let has = ProducerView { upgrade_template_found: true, producer_has_upgrade: true };
        let lacks = ProducerView { upgrade_template_found: true, producer_has_upgrade: false };
        let no_template = ProducerView { upgrade_template_found: false, producer_has_upgrade: true };
        assert_eq!(on_die(true, Some(has)), Some(UpgradeDieEffect::RemoveUpgradeFromProducer));
        assert_eq!(on_die(true, Some(lacks)), Some(UpgradeDieEffect::ProducerLacksUpgrade));
        assert_eq!(on_die(true, Some(no_template)), None);
        assert_eq!(on_die(true, None), None, "no producer");
        assert_eq!(on_die(false, Some(has)), None, "not applicable");
    }
}
