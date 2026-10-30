// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (AIUpdateModuleData, 81-200)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AIUpdate.h (AIUpdateModuleData)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `AIUpdateModuleData`: an AI's INI block (its turrets, idle auto-acquisition, mood check rate, player commands, linked turrets),
//! and the locomotor sets an object's `Locomotor` lines give it (`parseLocomotorSet`). `p1w13-sonnet`, layer 10, batch A1.
//!
//! # Upstream facts kept
//!
//! - **The locomotor sets are shared.** A template's override and reskin copies share its module data (upstream copies the
//!   pointers), and `parseLocomotorSet` writes into that shared data (`friend_getNthData` casts the `const` away): a copy's
//!   `Locomotor` line changes the original's sets too. Here the sets are an `Arc<RwLock<..>>` that every clone of the data shares.
//! - `parseLocomotorSet` makes the set's entry before it checks it (`m_locomotorTemplates[set]`), so a set named and then refused,
//!   or failed part way, is present (empty or partial): `findLocomotorTemplateVector` finds it.
//! - A second `Turret` (or `AltTurret`) in one block throws `INI_INVALID_DATA` ("Only one turret to a customer, for now"), after the
//!   first is kept; `ControlledWeaponSlots` ORs into the slots it already has.
//!
//! Verified against upstream's `AIUpdate.cpp` and `TurretAI.cpp` members (cut verbatim) behind the real `ThingTemplate`, `INI`
//! and `LocomotorStore` by `tests/differential_thing_template.rs`: 0 mismatches over 1,173 AI module data in 19,438 dumps
//! (2026-10-27; plants 3180-3186, 3188 caught).

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::ModuleData;
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::logic::ai::ai_update_types::{LocomotorSetType, AUTO_ACQUIRE_ENEMIES_NAMES, LOCOMOTOR_SET_NAMES};
use crate::logic::ai::turret::TurretAiData;
use crate::logic::locomotor::LocomotorStore;
use crate::logic::modules::plant;
use crate::logic::module::update_module::UpdateModuleData;

/// `LocomotorTemplateMap`: each set's locomotor templates, by name key (the store's templates), in order.
pub type LocomotorTemplateMap = BTreeMap<LocomotorSetType, Vec<NameKeyType>>;

/// `class AIUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug)]
pub struct AiUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_locomotorTemplates`: shared by every copy of the data (see the module's notes).
    pub locomotor_templates: Arc<RwLock<LocomotorTemplateMap>>,
    /// `m_turretData[MAX_TURRETS]`.
    pub turret_data: [Option<Arc<TurretAiData>>; 2],
    /// "how frequently we should recheck for enemies due to moods, when idle"
    pub mood_attack_check_rate: u32,
    pub forbid_player_commands: bool,
    pub turrets_linked: bool,
    pub auto_acquire_enemies_when_idle: u32,
}

impl Default for AiUpdateModuleData {
    /// `AIUpdateModuleData()`.
    fn default() -> Self {
        AiUpdateModuleData {
            update: UpdateModuleData::new(),
            locomotor_templates: Arc::new(RwLock::new(BTreeMap::new())),
            turret_data: [None, None],
            mood_attack_check_rate: LOGICFRAMES_PER_SECOND as u32 * if plant(3184) { 3 } else { 2 },
            forbid_player_commands: false,
            turrets_linked: false,
            auto_acquire_enemies_when_idle: 0,
        }
    }
}

/// `parseTurret` into turret `k`: a nested `TurretAIData` block; a second one throws.
fn parse_turret(ini: &mut Ini<'_>, d: &mut AiUpdateModuleData, k: usize) -> IniResult<()> {
    if d.turret_data[k].is_some() && !plant(3180) {
        return Err(IniError::new(IniErrorKind::InvalidData)); // "Only one turret to a customer, for now"
    }
    let mut td = TurretAiData::default();
    ini.init_from_ini_multi_proc(&mut td, TurretAiData::build_field_parse)?;
    d.turret_data[if plant(3188) { 0 } else { k }] = Some(Arc::new(td));
    Ok(())
}

/// `AIUpdateModuleData::buildFieldParse`'s own table.
pub(crate) static OWN_FIELD_PARSE: [FieldParse<AiUpdateModuleData>; 7] = [
    FieldParse::new("Turret", |i, d, _| parse_turret(i, d, 0), UserData::None),
    FieldParse::new("AltTurret", |i, d, _| parse_turret(i, d, 1), UserData::None),
    FieldParse::new(
        "AutoAcquireEnemiesWhenIdle",
        |i, d, _| {
            let mut v = d.auto_acquire_enemies_when_idle;
            let r = i.parse_bit_string32(&mut v, &AUTO_ACQUIRE_ENEMIES_NAMES);
            d.auto_acquire_enemies_when_idle = v;
            r
        },
        UserData::None,
    ),
    FieldParse::new("MoodAttackCheckRate", |i, d, _| { d.mood_attack_check_rate = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ForbidPlayerCommands", |i, d, _| { d.forbid_player_commands = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("TurretsLinked", |i, d, _| { d.turrets_linked = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl AiUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (nothing), then its own.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, AiUpdateModuleData>) -> IniResult<()> {
        ModuleData::build_field_parse(p)?;
        p.add(&OWN_FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<AiUpdateModuleData> {
        let mut data = AiUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, AiUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }

    /// `isAiModuleData()`.
    pub fn is_ai_module_data(&self) -> bool {
        true
    }

    /// `findLocomotorTemplateVector(t)`: the set's templates; none when the set was never named.
    pub fn find_locomotor_template_vector(&self, t: LocomotorSetType) -> Option<Vec<NameKeyType>> {
        let map = self.locomotor_templates.read().expect("the locomotor sets");
        map.get(&t).cloned()
    }

    /// `parseLocomotorSet(ini, thingTemplate, ...)` once the template's AI data is found: a set's name, then its locomotors
    /// (`None` and empty names skipped), each a template of the store. A set named twice throws unless the load creates overrides.
    pub fn parse_locomotor_set(&self, ini: &mut Ini<'_>, names: &mut NameKeyGenerator, store: &LocomotorStore) -> IniResult<()> {
        let set = Ini::scan_index_list(&ini.get_next_token(None)?, &LOCOMOTOR_SET_NAMES)?;
        let mut map = self.locomotor_templates.write().expect("the locomotor sets");
        let entry = map.entry(set).or_default();
        if !entry.is_empty() && (ini.get_load_type() != IniLoadType::CreateOverrides || plant(3181)) {
            return Err(IniError::new(IniErrorKind::InvalidData)); // "re-specifying a LocomotorSet is no longer allowed"
        }
        entry.clear();
        let mut loco_name = Some(ini.get_next_token(None)?);
        while let Some(name) = loco_name {
            if !name.is_empty() && !(if plant(3182) { name == b"None" } else { name.eq_ignore_ascii_case(b"None") }) {
                let key = names.name_to_key(&name);
                if store.find_locomotor_template(key).is_none() {
                    if plant(3183) {
                        drop(map);
                        self.locomotor_templates.write().expect("the locomotor sets").remove(&set);
                    }
                    return Err(IniError::new(IniErrorKind::InvalidData)); // "Locomotor %s not found!"
                }
                entry.push(key);
            }
            loco_name = ini.get_next_token_or_null(None);
        }
        Ok(())
    }
}
