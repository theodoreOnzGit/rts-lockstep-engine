// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/EjectPilotDie.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/EjectPilotDie.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `EjectPilotDie`: a vehicle that dies ejects its pilot, with the object creation list for the air or the ground, and the voice and sound
//! of the ejection.
//!
//! `AirCreationList` and `GroundCreationList` are `INI::parseObjectCreationList`, kept as names and looked up in
//! `world.statics.object_creation_list_store` when they are used (an unknown name and `None` are `NULL`, as the pointer upstream stores). `InvulnerableTime` is read and
//! never used. Upstream `onDie`: the die mux; the damage dealer (`findObjectByID(sourceID)`, `NULL` is fine); the air list if
//! `isSignificantlyAboveTerrain()` else the ground list; `ejectPilot(ocl, object, dealer)`: nothing for a `NULL` list, else
//! `ObjectCreationList::create(ocl, object, dealer)` ([`CutCall::OclCreateWithSecondary`]) and two sounds, the template's per-unit
//! `VoiceEject` (at the object's position, for the controlling player's index: **dereferenced without a test**, so an object with no controlling
//! player is undefined, [`Undefined::NullControllingPlayer`]) and `SoundEject` (at the position): [`ObjectClientEvent::AudioAddEvent`].

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
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::update_body::UpdateBody;

/// `class EjectPilotDieModuleData : public DieModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EjectPilotDieModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
    /// `m_oclInAir`, as the name `AirCreationList` gave.
    pub ocl_in_air: Option<Vec<u8>>,
    /// `m_oclOnGround`.
    pub ocl_on_ground: Option<Vec<u8>>,
    /// `m_invulnerableTime` (frames; never used).
    pub invulnerable_time: u32,
}

impl EjectPilotDieModuleData {
    /// `EjectPilotDieModuleData()`.
    pub fn new() -> Self {
        EjectPilotDieModuleData {
            die: DieModuleData::new(),
            ocl_in_air: None,
            ocl_on_ground: None,
            invulnerable_time: 0,
        }
    }

    /// `buildFieldParse(p)`: the behaviour base, the four die mux fields (read from the layer-5 table into `die`), then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, EjectPilotDieModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<EjectPilotDieModuleData> {
        let mut data = EjectPilotDieModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, EjectPilotDieModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `EjectPilotDieModuleData::die`.
fn lift_die_field(
    k: usize,
    i: &mut Ini<'_>,
    d: &mut EjectPilotDieModuleData,
    u: &UserData,
) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `EjectPilotDieModuleData::m_dataFieldParse`: the die mux fields, then this class's.
static FIELD_PARSE: [FieldParse<EjectPilotDieModuleData>; 8] = [
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
        "AirCreationList",
        |ini, d, _| {
            d.ocl_in_air = Some(ini.get_next_token(None)?);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "GroundCreationList",
        |ini, d, _| {
            d.ocl_on_ground = Some(ini.get_next_token(None)?);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "InvulnerableTime",
        |ini, d, _| {
            d.invulnerable_time = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class EjectPilotDie : public DieModule`.
#[derive(Clone, Debug)]
pub struct EjectPilotDie {
    die: DieModule,
    /// `getEjectPilotDieModuleData()`.
    data: Arc<EjectPilotDieModuleData>,
}

impl PartialEq for EjectPilotDie {
    /// The same object, the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl EjectPilotDie {
    /// `EjectPilotDie(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<EjectPilotDieModuleData>>,
    ) -> IniResult<EjectPilotDie> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(EjectPilotDie { die, data })
    }

    /// The data, as `getEjectPilotDieModuleData()`.
    pub fn module_data(&self) -> &Arc<EjectPilotDieModuleData> {
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

    /// `static ejectPilot(ocl, dyingObject, damageDealer)`; `damage_dealer` is `INVALID_ID` for `NULL`. An error is upstream's undefined
    /// behaviour (the object has no controlling player).
    pub fn eject_pilot(
        world: &mut World,
        ocl: Option<&[u8]>,
        dying: ObjectId,
        damage_dealer: ObjectId,
    ) -> Result<(), IniError> {
        let Some(ocl) = ocl else { return Ok(()) };
        if world.objects.find(dying).is_none() {
            return Ok(());
        }
        // it's OK for damageDealer to be null
        world.cut(CutCall::OclCreateWithSecondary {
            primary: dying,
            secondary: damage_dealer,
            ocl: ocl.to_vec(),
        });
        let (voice, sound, position) = {
            let o = world.objects.find(dying).expect("checked above");
            (
                o.template()
                    .get_per_unit_sound(b"VoiceEject")
                    .event_name
                    .clone(),
                o.template()
                    .get_per_unit_sound(b"SoundEject")
                    .event_name
                    .clone(),
                *o.thing.position(),
            )
        };
        let player = world
            .get_controlling_player(dying)
            .ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        world.emit(ObjectClientEvent::AudioAddEvent {
            name: voice,
            object_id: INVALID_ID,
            position: Some(position),
            player_index: if crate::logic::modules::plant(723) {
                None
            } else {
                Some(player)
            },
        });
        if !crate::logic::modules::plant(722) {
            world.emit(ObjectClientEvent::AudioAddEvent {
                name: sound,
                object_id: INVALID_ID,
                position: Some(position),
                player_index: None,
            });
        }
        Ok(())
    }
}

impl DieBehavior for EjectPilotDie {
    fn die(&self) -> &DieModule {
        &self.die
    }
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.is_die_applicable(world, me, info) && !crate::logic::modules::plant(725) {
            return;
        }
        // `findObjectByID(sourceID)`: NULL for no such object
        let dealer = if world.objects.find(info.input.source_id).is_some()
            && !crate::logic::modules::plant(724)
        {
            info.input.source_id
        } else {
            INVALID_ID
        };
        let above = world.is_significantly_above_terrain(me.object);
        let chosen = if above != crate::logic::modules::plant(721) {
            self.data.ocl_in_air.clone()
        } else {
            self.data.ocl_on_ground.clone()
        };
        // `INI::parseObjectCreationList` stored `NULL` for a name that is no list (and for `None`)
        let statics = world.statics.clone();
        let ocl = chosen.filter(|n| {
            statics
                .object_creation_list_store
                .find_object_creation_list(&mut world.names, n)
                .is_some()
        });
        if let Err(e) = Self::eject_pilot(world, ocl.as_deref(), me.object, dealer) {
            world.cuts.module_errors.push((me, e));
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::EjectPilotDie(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::EjectPilotDie(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for EjectPilotDie {
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

/// The factory's `NewModuleDataProc` for `EjectPilotDie`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::EjectPilotDie(Arc::new(
        EjectPilotDieModuleData::friend_new_module_data(ini)?,
    )))
}
