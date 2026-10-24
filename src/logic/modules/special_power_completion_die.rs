// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpecialPowerCompletionDie.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/SpecialPowerCompletionDie.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpecialPowerCompletionDie`: tells the script engine that the special power its creator launched has completed (a paradropped or
//! airdropped payload that has died).
//!
//! `SpecialPowerTemplate` is `INI::parseSpecialPowerTemplate`; as in `UnpauseSpecialPowerUpgrade` the name is kept and looked up in
//! `world.statics.special_power_store` when it is used. `setCreator` (called by the object creation list on the die it finds with
//! `findSpecialPowerCompletionDie`, [`ModuleCall::SetCreator`]) takes the first creator it is given. Upstream `onDie`: the die mux, then
//! `notifyScriptEngine`: if a creator was set, `TheScriptEngine->notifyOfCompletedSpecialPower(getObject()->getControllingPlayer()->getPlayerIndex(),
//! m_specialPowerTemplate->getName(), m_creatorID)` ([`CutCall::ScriptNotifyOfCompletedSpecialPower`]), where **the controlling player and the
//! template are dereferenced without a test**: an object with no controlling player, or a name the store does not know, is undefined
//! upstream ([`Undefined::NullControllingPlayer`], [`Undefined::NullSpecialPowerTemplate`]); the call is then not made and the error is recorded
//! in `world.cuts.module_errors`.

use std::sync::Arc;

use crate::common::game_type::INVALID_ID;
use crate::common::ini::Undefined;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::modules::die_body::{die_module_call, DieBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::update_body::UpdateBody;

/// `class SpecialPowerCompletionDieModuleData : public DieModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpecialPowerCompletionDieModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
    /// `m_specialPowerTemplate`, as the token that names it (see the module docs); `None` for a pointer that is `NULL` before any lookup.
    pub special_power_template: Option<Vec<u8>>,
}

impl SpecialPowerCompletionDieModuleData {
    /// `SpecialPowerCompletionDieModuleData()`.
    pub fn new() -> Self {
        SpecialPowerCompletionDieModuleData {
            die: DieModuleData::new(),
            special_power_template: None,
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the four die mux fields (read from the layer-5 table into `die`), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, SpecialPowerCompletionDieModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<SpecialPowerCompletionDieModuleData> {
        let mut data = SpecialPowerCompletionDieModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                SpecialPowerCompletionDieModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `SpecialPowerCompletionDieModuleData::die`.
fn lift_die_field(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut SpecialPowerCompletionDieModuleData,
    u: &UserData,
) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `SpecialPowerCompletionDieModuleData::m_dataFieldParse`: the die mux fields, then this class's.
static FIELD_PARSE: [FieldParse<SpecialPowerCompletionDieModuleData>; 6] = [
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
        "SpecialPowerTemplate",
        |ini, d, _| {
            d.special_power_template = Some(ini.get_next_token(None)?);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class SpecialPowerCompletionDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct SpecialPowerCompletionDie {
    die: DieModule,
    /// `getSpecialPowerCompletionDieModuleData()`.
    data: Arc<SpecialPowerCompletionDieModuleData>,
    /// `m_creatorID`.
    creator_id: ObjectId,
    /// `m_creatorSet`.
    creator_set: bool,
}

impl PartialEq for SpecialPowerCompletionDie {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.creator_id == other.creator_id
            && self.creator_set == other.creator_set
    }
}

impl SpecialPowerCompletionDie {
    /// `SpecialPowerCompletionDie(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<SpecialPowerCompletionDieModuleData>>,
    ) -> IniResult<SpecialPowerCompletionDie> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(SpecialPowerCompletionDie {
            die,
            data,
            creator_id: INVALID_ID,
            creator_set: false,
        })
    }

    /// The data, as `getSpecialPowerCompletionDieModuleData()`.
    pub fn module_data(&self) -> &Arc<SpecialPowerCompletionDieModuleData> {
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

    /// `notifyScriptEngine()`.
    pub fn notify_script_engine(&mut self, world: &mut World, me: ModuleHandle) {
        if self.creator_id != INVALID_ID || crate::logic::modules::plant(712) {
            let statics = world.statics.clone();
            let name = self
                .data
                .special_power_template
                .as_deref()
                .and_then(|n| statics.special_power_store.find_special_power_template(n))
                .map(|t| t.name().to_vec());
            let Some(player) = world.get_controlling_player(me.object) else {
                world
                    .cuts
                    .module_errors
                    .push((me, IniError::undefined(Undefined::NullControllingPlayer)));
                return;
            };
            let Some(name) = name else {
                world
                    .cuts
                    .module_errors
                    .push((me, IniError::undefined(Undefined::NullSpecialPowerTemplate)));
                return;
            };
            world.cut(CutCall::ScriptNotifyOfCompletedSpecialPower {
                player: player + i32::from(crate::logic::modules::plant(713)),
                name,
                creator: self.creator_id,
            });
        }
    }

    /// `setCreator(creatorID)`: only the first call counts.
    pub fn set_creator(&mut self, creator: ObjectId) {
        if !self.creator_set || crate::logic::modules::plant(711) {
            self.creator_set = true;
            self.creator_id = creator;
        }
    }
}

impl DieBehavior for SpecialPowerCompletionDie {
    fn die(&self) -> &DieModule {
        &self.die
    }
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.is_die_applicable(world, me, info) && !crate::logic::modules::plant(715) {
            return;
        }
        self.notify_script_engine(world, me);
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpecialPowerCompletionDie(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpecialPowerCompletionDie(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(
        &mut self,
        _world: &mut World,
        _me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::SetCreator { creator } => {
                self.set_creator(creator);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for SpecialPowerCompletionDie {
    /// `crc`: `DieModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.die.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `DieModule::xfer`, then creator_id, creator_set.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.die.xfer(xfer)?;
        xfer.xfer_object_id(&mut self.creator_id)?;
        xfer.xfer_bool(&mut self.creator_set)?;
        Ok(())
    }
    /// `loadPostProcess`: `DieModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.die.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `SpecialPowerCompletionDie`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SpecialPowerCompletionDie(Arc::new(
        SpecialPowerCompletionDieModuleData::friend_new_module_data(ini)?,
    )))
}
