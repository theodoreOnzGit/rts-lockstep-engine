// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CreateObjectDie.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateObjectDie.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CreateObjectDie`: when its object dies (and the die mux applies), `CreationList` makes something, with the damage dealer as the secondary object
//! ([`World::ocl_create_returning`]). With `TransferPreviousHealth` the new object is hurt as the old one was: first its subdual damage
//! (`DAMAGE_SUBDUAL_UNRESISTABLE`, no source), then the health it had lost before the killing blow (`DAMAGE_UNRESISTABLE`, from the last damage's
//! source; "the last known health before we died"); and every AI that was attacking the old object attacks the new one.
//!
//! Upstream facts kept: the transfer needs both bodies; the subdual damage only when there is some, the health only when it is above 0; the attackers
//! are moved for every object with an AI, in the game logic's list order (newest first).

use std::sync::Arc;

use crate::common::game_type::INVALID_ID;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_SUBDUAL_UNRESISTABLE, DAMAGE_UNRESISTABLE};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::modules::die_body::{die_module_call, DieBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;

/// `class CreateObjectDieModuleData : public DieModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateObjectDieModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
    /// `m_ocl`, as the name `CreationList` gave.
    pub ocl: Option<Vec<u8>>,
    /// `m_transferPreviousHealth`.
    pub transfer_previous_health: bool,
}

impl CreateObjectDieModuleData {
    /// `CreateObjectDieModuleData()`.
    pub fn new() -> Self {
        CreateObjectDieModuleData {
            die: DieModuleData::new(),
            ocl: None,
            transfer_previous_health: false,
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the four die mux fields (read from the layer-5 table into `die`), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, CreateObjectDieModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<CreateObjectDieModuleData> {
        let mut data = CreateObjectDieModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CreateObjectDieModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `CreateObjectDieModuleData::die`.
fn lift_die_field(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut CreateObjectDieModuleData,
    u: &UserData,
) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `DieModuleData::buildFieldParse`'s fields, then `CreateObjectDieModuleData`'s.
static FIELD_PARSE: [FieldParse<CreateObjectDieModuleData>; 7] = [
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
        "CreationList",
        |ini, d, _| {
            d.ocl = Some(ini.get_next_token(None)?);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "TransferPreviousHealth",
        |ini, d, _| {
            d.transfer_previous_health = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class CreateObjectDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct CreateObjectDie {
    die: DieModule,
    /// `getCreateObjectDieModuleData()`.
    data: Arc<CreateObjectDieModuleData>,
}

impl PartialEq for CreateObjectDie {
    /// The same object, the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl CreateObjectDie {
    /// `CreateObjectDie(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<CreateObjectDieModuleData>>,
    ) -> IniResult<CreateObjectDie> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(CreateObjectDie { die, data })
    }

    /// The data, as `getCreateObjectDieModuleData()`.
    pub fn module_data(&self) -> &Arc<CreateObjectDieModuleData> {
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

    /// The health transfer of `TransferPreviousHealth`, from `old` to `new`.
    fn transfer_previous_health(world: &mut World, old: ObjectId, new: ObjectId) {
        let (Some(old_body), Some(new_body)) =
            (world.get_body_module(old), world.get_body_module(new))
        else {
            return;
        };
        // "First transfer subdual damage"
        let subdual = world
            .module_call(old, old_body, ModuleCall::GetCurrentSubdualDamageAmount)
            .as_real(0.0);
        if subdual > 0.0 && !crate::logic::modules::plant(1752) {
            let mut info = DamageInfo::default();
            info.input.amount = subdual;
            info.input.damage_type = DAMAGE_SUBDUAL_UNRESISTABLE;
            info.input.source_id = INVALID_ID;
            world.module_call(new, new_body, ModuleCall::AttemptDamage(info));
        }
        // "Now transfer the previous health from the old object to the new."
        let max = world
            .module_call(old, old_body, ModuleCall::GetMaxHealth)
            .as_real(0.0);
        let previous = world
            .module_call(old, old_body, ModuleCall::GetPreviousHealth)
            .as_real(0.0);
        let source = match world.module_call(old, old_body, ModuleCall::GetLastDamageInfo) {
            ModuleReply::Damage(d) => d.input.source_id,
            _ => INVALID_ID,
        };
        let mut info = DamageInfo::default();
        info.input.amount = if crate::logic::modules::plant(1753) {
            max
        } else {
            max - previous
        };
        info.input.damage_type = DAMAGE_UNRESISTABLE;
        info.input.source_id = if crate::logic::modules::plant(1754) {
            INVALID_ID
        } else {
            source
        };
        if info.input.amount > 0.0 {
            world.module_call(new, new_body, ModuleCall::AttemptDamage(info));
        }
    }
}

impl DieBehavior for CreateObjectDie {
    fn die(&self) -> &DieModule {
        &self.die
    }
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.is_die_applicable(world, me, info) && !crate::logic::modules::plant(1751) {
            return;
        }
        // `findObjectByID(sourceID)`: NULL for no such object
        let dealer = if world.objects.find(info.input.source_id).is_some() {
            info.input.source_id
        } else {
            INVALID_ID
        };
        // `INI::parseObjectCreationList` stored `NULL` for a name that is no list (and for `None`); `create(NULL, ...)` makes nothing
        let statics = world.statics.clone();
        let ocl = self.data.ocl.clone().filter(|n| {
            statics
                .object_creation_list_store
                .find_object_creation_list(&mut world.names, n)
                .is_some()
        });
        let Some(ocl) = ocl else { return };
        let new_object = world.ocl_create_returning(me.object, dealer, &ocl);
        if new_object == INVALID_ID
            || !self.data.transfer_previous_health
            || crate::logic::modules::plant(1755)
        {
            return;
        }
        let old = me.object;
        Self::transfer_previous_health(world, old, new_object);
        // "Transfer attackers."
        for obj in world.objects.ids_in_order() {
            let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) else {
                continue;
            };
            if !crate::logic::modules::plant(1756) {
                world.module_call(
                    obj,
                    ai,
                    ModuleCall::AiTransferAttack {
                        from: old,
                        to: new_object,
                    },
                );
            }
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CreateObjectDie(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CreateObjectDie(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for CreateObjectDie {
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

/// The factory's `NewModuleDataProc` for `CreateObjectDie`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CreateObjectDie(Arc::new(
        CreateObjectDieModuleData::friend_new_module_data(ini)?,
    )))
}
