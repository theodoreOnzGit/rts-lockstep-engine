// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DamDie.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/DamDie.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DamDie`: when the dam dies, every water-wave object on the map is enabled.
//!
//! No fields of its own (the die mux only). Upstream `onDie`: `if (!isDieApplicable(damageInfo)) return;` then, for every object in the game
//! logic's list (newest first), in that order, `if (obj->isKindOf(KINDOF_WAVEGUIDE)) obj->clearDisabled(DISABLED_DEFAULT)`.

use std::sync::Arc;

use crate::common::disabled_types::DISABLED_DEFAULT;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::kind_of::KINDOF_WAVEGUIDE;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::modules::die_body::{die_module_call, DieBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;

/// `class DamDieModuleData : public DieModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamDieModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
}

impl DamDieModuleData {
    /// `DamDieModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `buildFieldParse(p)`: the behaviour base, the four die mux fields (read from the layer-5 table into `die`).
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, DamDieModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<DamDieModuleData> {
        let mut data = DamDieModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, DamDieModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `DamDieModuleData::die`.
fn lift_die_field(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut DamDieModuleData,
    u: &UserData,
) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `DamDieModuleData::m_dataFieldParse`: the die mux fields.
static FIELD_PARSE: [FieldParse<DamDieModuleData>; 5] = [
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
    FieldParse::end(),
];

/// `class DamDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct DamDie {
    die: DieModule,
    /// `getDamDieModuleData()`.
    data: Arc<DamDieModuleData>,
}

impl PartialEq for DamDie {
    /// The same object, the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl DamDie {
    /// `DamDie(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<DamDieModuleData>>) -> IniResult<DamDie> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(DamDie { die, data })
    }

    /// The data, as `getDamDieModuleData()`.
    pub fn module_data(&self) -> &Arc<DamDieModuleData> {
        &self.data
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        die_module_call(self, world, me, call)
    }
}

impl DieBehavior for DamDie {
    fn die(&self) -> &DieModule {
        &self.die
    }
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.is_die_applicable(world, me, info) && !crate::logic::modules::plant(703) {
            return;
        }
        // enable all the water wave objects on the map
        for id in world.objects.ids_in_order() {
            // only care aboue water waves
            let wave = world
                .objects
                .find(id)
                .is_some_and(|o| o.is_kind_of(KINDOF_WAVEGUIDE));
            if !wave && !crate::logic::modules::plant(701) {
                continue;
            }
            // clear any disabled status of the water wave
            if !crate::logic::modules::plant(702) {
                if let Some(t) = &mut world.cuts.trace {
                    t.push(format!("dam cleardisabled {id} {DISABLED_DEFAULT}"));
                }
                world.clear_disabled(id, DISABLED_DEFAULT);
            }
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::DamDie(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::DamDie(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for DamDie {
    /// `crc`: `DieModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.die.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `DieModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.die.xfer(xfer)?;
        Ok(())
    }
    /// `loadPostProcess`: `DieModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.die.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `DamDie`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::DamDie(Arc::new(
        DamDieModuleData::friend_new_module_data(ini)?,
    )))
}
