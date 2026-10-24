// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BoneFXUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BoneFXUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BoneFXDamage.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/BoneFXDamage.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BoneFXUpdate` and its partner `BoneFXDamage`: effects at the bones of an object's art, by body damage state. For each of the four states the
//! data names up to eight FX lists, eight object creation lists and eight particle systems, each at a bone, once or again after a random delay.
//! The update plays the current state's effects when their frames come (an FX list and an OCL at the bone's world position, a particle system
//! at the bone, attached to the object), unless the last damage's type is one the data excludes; `BoneFXDamage` tells it when the body damage
//! state changes (the running particle systems are destroyed and the times start again), and `StructureCollapseUpdate` stops it.
//!
//! # Where the world comes in
//!
//! - The bones are the art's (`getPristineBonePositions`, the client's): `cuts.contain.pristine_bones` through
//!   [`World::contain_drawable_pristine_bone`]; `convertBonePosToWorldPos` is [`World::contain_convert_bone_pos_to_world_pos`].
//! - `FXList::doFXPos` is [`ObjectClientEvent::FxListDoFxPos`]; `ObjectCreationList::create(ocl, obj, &pos, NULL, INVALID_ANGLE)` is
//!   [`CutCall::OclCreateAtPrimary`]; the particle systems are [`World::create_particle_system`], `World::body_particle_attach_to_object` and
//!   `World::body_particle_destroy`; whether the drawable is hidden, which stops a new system, is the client's
//!   ([`ObjectClientEvent::ParticleSystemStopIfDrawableHidden`]).
//! - The body's last damage is [`ModuleCall::GetLastDamageInfo`]; `changeBodyDamageState` is [`ModuleCall::BoneFxChangeBodyDamageState`] and
//!   `stopAllBoneFX` [`ModuleCall::BoneFxStopAll`].
//!
//! # Upstream facts kept
//!
//! - The FX and OCL delays are drawn from the logic's random numbers, the particle systems' from the client's; the frames are `Int`s.
//! - The bones of a state are looked up once, at its first effect (a bone the art does not have leaves the position as it was: zero, or a
//!   saved one).
//! - An effect's bone and damage type test run even when its list or template is `NULL`; a particle system's position is the bone's, not the
//!   world's.
//! - The save is upstream's `xferUser` of the arrays (raw `Int`s, `Coord3D`s, the state enum and the `Bool`s); a load into a module that has
//!   particle systems throws `SC_INVALID_DATA` after the count ([`XferStatus::ListNotEmpty`]).
//! - The module data's random variables have no constructor; only those of the parsed entries are ever read.
//!
//! # Deviation
//!
//! `~BoneFXUpdate` (`killRunningParticleSystems`) is not run: there is no hook for a module's destruction (as for `~FlammableUpdate`).
//!
//! # Where upstream is undefined
//!
//! An effect's bones looked up with no drawable ([`Undefined::NullDrawable`]); its damage test with no body ([`Undefined::NullBodyModule`]); a
//! body damage state outside the four ([`Undefined::ArrayIndexOutOfRange`]). The case is recorded in `world.cuts.module_errors` and the call
//! returns.
//!
//! `p1w13-sonnet`, batch R30, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::{real_to_int, Coord3D};
use crate::common::game_type::INVALID_ANGLE;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
    SEPS_COLON,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::random_value::{
    DistributionType, GameClientRandomVariable, GameLogicRandomVariable,
};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageTypeFlags, DAMAGE_TYPE_FLAGS_ALL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::damage_module::{DamageModule, DamageModuleData};
use crate::logic::module::update_module::{
    SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `BONE_FX_MAX_BONES`: "we can have this many effects of each type per body state".
pub const BONE_FX_MAX_BONES: usize = 8;
/// `BODYDAMAGETYPE_COUNT`.
pub const BODYDAMAGETYPE_COUNT: usize = 4;
/// The three kinds of effect, in the data's order: `m_fxList`, `m_OCL`, `m_particleSystem`.
pub const KIND_FX: usize = 0;
/// See [`KIND_FX`].
pub const KIND_OCL: usize = 1;
/// See [`KIND_FX`].
pub const KIND_PARTICLE: usize = 2;

/// `BaseBoneListInfo` with its effect: `BoneFXListInfo::fx`, `BoneOCLInfo::ocl` or `BoneParticleSystemInfo::particleSysTemplate`, by name.
#[derive(Clone, Debug, PartialEq)]
pub struct BoneListInfo {
    /// `locInfo.boneName` (empty: no effect).
    pub bone_name: Vec<u8>,
    /// `gameClientDelay` (the particle systems').
    pub game_client_delay: GameClientRandomVariable,
    /// `gameLogicDelay` (the FX lists' and the OCLs').
    pub game_logic_delay: GameLogicRandomVariable,
    /// `onlyOnce`.
    pub only_once: bool,
    /// The effect's name (`None`: `NULL`; an FX list or a particle system named `None`). An OCL is looked up when it is made.
    pub effect: Option<Vec<u8>>,
}

impl Default for BoneListInfo {
    /// `BoneFXUpdateModuleData()`'s: no bone, once, no effect (the random variables have no constructor: these values are never read).
    fn default() -> Self {
        BoneListInfo {
            bone_name: Vec::new(),
            game_client_delay: GameClientRandomVariable::new(0.0, 0.0, DistributionType::Constant),
            game_logic_delay: GameLogicRandomVariable::new(0.0, 0.0, DistributionType::Constant),
            only_once: true,
            effect: None,
        }
    }
}

/// `class BoneFXUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug)]
pub struct BoneFxUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_damageFXTypes`, `m_damageOCLTypes`, `m_damageParticleTypes`: "flags used to play or not play the effects".
    pub damage_types: [DamageTypeFlags; 3],
    /// `m_fxList`, `m_OCL`, `m_particleSystem`: `[kind][body damage state][index]`.
    pub lists: [[[BoneListInfo; BONE_FX_MAX_BONES]; BODYDAMAGETYPE_COUNT]; 3],
}

impl Default for BoneFxUpdateModuleData {
    /// `BoneFXUpdateModuleData()`: every damage type plays every effect.
    fn default() -> Self {
        BoneFxUpdateModuleData {
            update: UpdateModuleData::new(),
            damage_types: [DAMAGE_TYPE_FLAGS_ALL; 3],
            lists: Default::default(),
        }
    }
}

/// `parseFXList`, `parseObjectCreationList` and `parseParticleSystem`, for entry `kind * 32 + state * 8 + index` (the field's user data):
/// `Bone:<name> OnlyOnce:<Yes|No> <min delay> <max delay> FXList:<name>` (`OCL:`, `PSys:`).
fn parse_bone_entry(
    ini: &mut Ini<'_>,
    d: &mut BoneFxUpdateModuleData,
    u: &UserData,
) -> IniResult<()> {
    let UserData::Mask(m) = u else {
        return Err(IniError::new(IniErrorKind::InvalidData));
    };
    let (kind, state, index) = ((*m / 32) as usize, (*m / 8 % 4) as usize, (*m % 8) as usize);
    let info = &mut d.lists[kind][state][index];
    // parseFXLocInfo: "parse the location bone or location"
    let token = ini.get_next_token(Some(SEPS_COLON))?;
    if !token.eq_ignore_ascii_case(b"bone") {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    info.bone_name = ini.get_next_token(None)?;
    // make sure we have an "OnlyOnce:" token
    let token = ini.get_next_token(Some(SEPS_COLON))?;
    if !token.eq_ignore_ascii_case(b"onlyonce") {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    info.only_once = ini.parse_bool()?;
    let min = ini.parse_duration_real()?;
    let max = ini.parse_duration_real()?;
    if kind == KIND_PARTICLE {
        info.game_client_delay
            .set_range(min, max, DistributionType::Uniform);
    } else {
        info.game_logic_delay
            .set_range(min, max, DistributionType::Uniform);
    }
    // make sure we have an "FXList:" ("OCL:", "PSys:") token
    let token = ini.get_next_token(Some(SEPS_COLON))?;
    let label: &[u8] = match kind {
        KIND_FX => b"fxlist",
        KIND_OCL => b"ocl",
        _ => b"psys",
    };
    if !token.eq_ignore_ascii_case(label) {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    // INI::parseFXList, parseObjectCreationList, parseParticleSystemTemplate: the next token, looked up ("could be null!")
    let name = ini.get_next_token(None)?;
    info.effect = if kind != KIND_OCL && name.eq_ignore_ascii_case(b"None") {
        None
    } else {
        Some(name)
    };
    Ok(())
}

/// `dataFieldParse`.
static FIELD_PARSE: [FieldParse<BoneFxUpdateModuleData>; 100] = [
    FieldParse::new(
        "DamageFXTypes",
        |i, d, _| {
            d.damage_types[KIND_FX] = i.parse_damage_type_flags()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "DamageOCLTypes",
        |i, d, _| {
            d.damage_types[KIND_OCL] = i.parse_damage_type_flags()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "DamageParticleTypes",
        |i, d, _| {
            d.damage_types[KIND_PARTICLE] = i.parse_damage_type_flags()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("PristineFXList1", parse_bone_entry, UserData::Mask(0)),
    FieldParse::new("PristineFXList2", parse_bone_entry, UserData::Mask(1)),
    FieldParse::new("PristineFXList3", parse_bone_entry, UserData::Mask(2)),
    FieldParse::new("PristineFXList4", parse_bone_entry, UserData::Mask(3)),
    FieldParse::new("PristineFXList5", parse_bone_entry, UserData::Mask(4)),
    FieldParse::new("PristineFXList6", parse_bone_entry, UserData::Mask(5)),
    FieldParse::new("PristineFXList7", parse_bone_entry, UserData::Mask(6)),
    FieldParse::new("PristineFXList8", parse_bone_entry, UserData::Mask(7)),
    FieldParse::new("DamagedFXList1", parse_bone_entry, UserData::Mask(8)),
    FieldParse::new("DamagedFXList2", parse_bone_entry, UserData::Mask(9)),
    FieldParse::new("DamagedFXList3", parse_bone_entry, UserData::Mask(10)),
    FieldParse::new("DamagedFXList4", parse_bone_entry, UserData::Mask(11)),
    FieldParse::new("DamagedFXList5", parse_bone_entry, UserData::Mask(12)),
    FieldParse::new("DamagedFXList6", parse_bone_entry, UserData::Mask(13)),
    FieldParse::new("DamagedFXList7", parse_bone_entry, UserData::Mask(14)),
    FieldParse::new("DamagedFXList8", parse_bone_entry, UserData::Mask(15)),
    FieldParse::new("ReallyDamagedFXList1", parse_bone_entry, UserData::Mask(16)),
    FieldParse::new("ReallyDamagedFXList2", parse_bone_entry, UserData::Mask(17)),
    FieldParse::new("ReallyDamagedFXList3", parse_bone_entry, UserData::Mask(18)),
    FieldParse::new("ReallyDamagedFXList4", parse_bone_entry, UserData::Mask(19)),
    FieldParse::new("ReallyDamagedFXList5", parse_bone_entry, UserData::Mask(20)),
    FieldParse::new("ReallyDamagedFXList6", parse_bone_entry, UserData::Mask(21)),
    FieldParse::new("ReallyDamagedFXList7", parse_bone_entry, UserData::Mask(22)),
    FieldParse::new("ReallyDamagedFXList8", parse_bone_entry, UserData::Mask(23)),
    FieldParse::new("RubbleFXList1", parse_bone_entry, UserData::Mask(24)),
    FieldParse::new("RubbleFXList2", parse_bone_entry, UserData::Mask(25)),
    FieldParse::new("RubbleFXList3", parse_bone_entry, UserData::Mask(26)),
    FieldParse::new("RubbleFXList4", parse_bone_entry, UserData::Mask(27)),
    FieldParse::new("RubbleFXList5", parse_bone_entry, UserData::Mask(28)),
    FieldParse::new("RubbleFXList6", parse_bone_entry, UserData::Mask(29)),
    FieldParse::new("RubbleFXList7", parse_bone_entry, UserData::Mask(30)),
    FieldParse::new("RubbleFXList8", parse_bone_entry, UserData::Mask(31)),
    FieldParse::new("PristineOCL1", parse_bone_entry, UserData::Mask(32)),
    FieldParse::new("PristineOCL2", parse_bone_entry, UserData::Mask(33)),
    FieldParse::new("PristineOCL3", parse_bone_entry, UserData::Mask(34)),
    FieldParse::new("PristineOCL4", parse_bone_entry, UserData::Mask(35)),
    FieldParse::new("PristineOCL5", parse_bone_entry, UserData::Mask(36)),
    FieldParse::new("PristineOCL6", parse_bone_entry, UserData::Mask(37)),
    FieldParse::new("PristineOCL7", parse_bone_entry, UserData::Mask(38)),
    FieldParse::new("PristineOCL8", parse_bone_entry, UserData::Mask(39)),
    FieldParse::new("DamagedOCL1", parse_bone_entry, UserData::Mask(40)),
    FieldParse::new("DamagedOCL2", parse_bone_entry, UserData::Mask(41)),
    FieldParse::new("DamagedOCL3", parse_bone_entry, UserData::Mask(42)),
    FieldParse::new("DamagedOCL4", parse_bone_entry, UserData::Mask(43)),
    FieldParse::new("DamagedOCL5", parse_bone_entry, UserData::Mask(44)),
    FieldParse::new("DamagedOCL6", parse_bone_entry, UserData::Mask(45)),
    FieldParse::new("DamagedOCL7", parse_bone_entry, UserData::Mask(46)),
    FieldParse::new("DamagedOCL8", parse_bone_entry, UserData::Mask(47)),
    FieldParse::new("ReallyDamagedOCL1", parse_bone_entry, UserData::Mask(48)),
    FieldParse::new("ReallyDamagedOCL2", parse_bone_entry, UserData::Mask(49)),
    FieldParse::new("ReallyDamagedOCL3", parse_bone_entry, UserData::Mask(50)),
    FieldParse::new("ReallyDamagedOCL4", parse_bone_entry, UserData::Mask(51)),
    FieldParse::new("ReallyDamagedOCL5", parse_bone_entry, UserData::Mask(52)),
    FieldParse::new("ReallyDamagedOCL6", parse_bone_entry, UserData::Mask(53)),
    FieldParse::new("ReallyDamagedOCL7", parse_bone_entry, UserData::Mask(54)),
    FieldParse::new("ReallyDamagedOCL8", parse_bone_entry, UserData::Mask(55)),
    FieldParse::new("RubbleOCL1", parse_bone_entry, UserData::Mask(56)),
    FieldParse::new("RubbleOCL2", parse_bone_entry, UserData::Mask(57)),
    FieldParse::new("RubbleOCL3", parse_bone_entry, UserData::Mask(58)),
    FieldParse::new("RubbleOCL4", parse_bone_entry, UserData::Mask(59)),
    FieldParse::new("RubbleOCL5", parse_bone_entry, UserData::Mask(60)),
    FieldParse::new("RubbleOCL6", parse_bone_entry, UserData::Mask(61)),
    FieldParse::new("RubbleOCL7", parse_bone_entry, UserData::Mask(62)),
    FieldParse::new("RubbleOCL8", parse_bone_entry, UserData::Mask(63)),
    FieldParse::new(
        "PristineParticleSystem1",
        parse_bone_entry,
        UserData::Mask(64),
    ),
    FieldParse::new(
        "PristineParticleSystem2",
        parse_bone_entry,
        UserData::Mask(65),
    ),
    FieldParse::new(
        "PristineParticleSystem3",
        parse_bone_entry,
        UserData::Mask(66),
    ),
    FieldParse::new(
        "PristineParticleSystem4",
        parse_bone_entry,
        UserData::Mask(67),
    ),
    FieldParse::new(
        "PristineParticleSystem5",
        parse_bone_entry,
        UserData::Mask(68),
    ),
    FieldParse::new(
        "PristineParticleSystem6",
        parse_bone_entry,
        UserData::Mask(69),
    ),
    FieldParse::new(
        "PristineParticleSystem7",
        parse_bone_entry,
        UserData::Mask(70),
    ),
    FieldParse::new(
        "PristineParticleSystem8",
        parse_bone_entry,
        UserData::Mask(71),
    ),
    FieldParse::new(
        "DamagedParticleSystem1",
        parse_bone_entry,
        UserData::Mask(72),
    ),
    FieldParse::new(
        "DamagedParticleSystem2",
        parse_bone_entry,
        UserData::Mask(73),
    ),
    FieldParse::new(
        "DamagedParticleSystem3",
        parse_bone_entry,
        UserData::Mask(74),
    ),
    FieldParse::new(
        "DamagedParticleSystem4",
        parse_bone_entry,
        UserData::Mask(75),
    ),
    FieldParse::new(
        "DamagedParticleSystem5",
        parse_bone_entry,
        UserData::Mask(76),
    ),
    FieldParse::new(
        "DamagedParticleSystem6",
        parse_bone_entry,
        UserData::Mask(77),
    ),
    FieldParse::new(
        "DamagedParticleSystem7",
        parse_bone_entry,
        UserData::Mask(78),
    ),
    FieldParse::new(
        "DamagedParticleSystem8",
        parse_bone_entry,
        UserData::Mask(79),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem1",
        parse_bone_entry,
        UserData::Mask(80),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem2",
        parse_bone_entry,
        UserData::Mask(81),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem3",
        parse_bone_entry,
        UserData::Mask(82),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem4",
        parse_bone_entry,
        UserData::Mask(83),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem5",
        parse_bone_entry,
        UserData::Mask(84),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem6",
        parse_bone_entry,
        UserData::Mask(85),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem7",
        parse_bone_entry,
        UserData::Mask(86),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem8",
        parse_bone_entry,
        UserData::Mask(87),
    ),
    FieldParse::new(
        "RubbleParticleSystem1",
        parse_bone_entry,
        UserData::Mask(88),
    ),
    FieldParse::new(
        "RubbleParticleSystem2",
        parse_bone_entry,
        UserData::Mask(89),
    ),
    FieldParse::new(
        "RubbleParticleSystem3",
        parse_bone_entry,
        UserData::Mask(90),
    ),
    FieldParse::new(
        "RubbleParticleSystem4",
        parse_bone_entry,
        UserData::Mask(91),
    ),
    FieldParse::new(
        "RubbleParticleSystem5",
        parse_bone_entry,
        UserData::Mask(92),
    ),
    FieldParse::new(
        "RubbleParticleSystem6",
        parse_bone_entry,
        UserData::Mask(93),
    ),
    FieldParse::new(
        "RubbleParticleSystem7",
        parse_bone_entry,
        UserData::Mask(94),
    ),
    FieldParse::new(
        "RubbleParticleSystem8",
        parse_bone_entry,
        UserData::Mask(95),
    ),
    FieldParse::end(),
];

impl BoneFxUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, BoneFxUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<BoneFxUpdateModuleData> {
        let mut data = BoneFxUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, BoneFxUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class BoneFXUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct BoneFxUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getBoneFXUpdateModuleData()`.
    data: Arc<BoneFxUpdateModuleData>,
    /// `m_particleSystemIDs`.
    particle_system_ids: Vec<u32>,
    /// `m_nextFXFrame`, `m_nextOCLFrame`, `m_nextParticleSystemFrame`: `[kind][state][index]`, -1 for none.
    next_frame: [[[i32; BONE_FX_MAX_BONES]; BODYDAMAGETYPE_COUNT]; 3],
    /// `m_FXBonePositions`, `m_OCLBonePositions`, `m_PSBonePositions`.
    bone_positions: [[[Coord3D; BONE_FX_MAX_BONES]; BODYDAMAGETYPE_COUNT]; 3],
    /// `m_curBodyState`.
    cur_body_state: i32,
    /// `m_bonesResolved`.
    bones_resolved: [bool; BODYDAMAGETYPE_COUNT],
    /// `m_active`.
    active: bool,
}

impl PartialEq for BoneFxUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.particle_system_ids == other.particle_system_ids
            && self.next_frame == other.next_frame
            && self.state_words() == other.state_words()
    }
}

/// The bytes of `xferUser` of an array of `Int`s.
fn int_bytes(v: &[[[i32; BONE_FX_MAX_BONES]; BODYDAMAGETYPE_COUNT]; 3], kind: usize) -> Vec<u8> {
    v[kind]
        .iter()
        .flatten()
        .flat_map(|x| x.to_le_bytes())
        .collect()
}

/// The bytes of `xferUser` of an array of `Coord3D`s.
fn coord_bytes(
    v: &[[[Coord3D; BONE_FX_MAX_BONES]; BODYDAMAGETYPE_COUNT]; 3],
    kind: usize,
) -> Vec<u8> {
    v[kind]
        .iter()
        .flatten()
        .flat_map(|c| [c.x, c.y, c.z])
        .flat_map(|x| x.to_le_bytes())
        .collect()
}

impl BoneFxUpdate {
    /// `getInterfaceMask()`: the update module's.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<BoneFxUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(BoneFxUpdate {
            update,
            data,
            particle_system_ids: Vec::new(),
            next_frame: [[[-1; BONE_FX_MAX_BONES]; BODYDAMAGETYPE_COUNT]; 3],
            bone_positions: Default::default(),
            cur_body_state: 0,
            bones_resolved: [false; BODYDAMAGETYPE_COUNT],
            active: false,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getBoneFXUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<BoneFxUpdateModuleData> {
        &self.data
    }

    /// The state the test compares, as words: the next frames, the bone positions, the state, the resolved flags, `m_active`, then the
    /// particle system count and ids.
    pub fn state_words(&self) -> Vec<u32> {
        let mut w: Vec<u32> = self
            .next_frame
            .iter()
            .flatten()
            .flatten()
            .map(|&x| x as u32)
            .collect();
        w.extend(
            self.bone_positions
                .iter()
                .flatten()
                .flatten()
                .flat_map(|c| [c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]),
        );
        w.push(self.cur_body_state as u32);
        w.extend(self.bones_resolved.iter().map(|&b| u32::from(b)));
        w.push(u32::from(self.active));
        w.push(self.particle_system_ids.len() as u32);
        w.extend(self.particle_system_ids.iter().copied());
        w
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object (the constructor makes no wake call).
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// Records that upstream is undefined here.
    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }

    /// `m_curBodyState` as an index of the arrays.
    fn state_index(&self) -> Result<usize, Undefined> {
        usize::try_from(self.cur_body_state)
            .ok()
            .filter(|&s| s < BODYDAMAGETYPE_COUNT)
            .ok_or(Undefined::ArrayIndexOutOfRange)
    }

    /// `onObjectCreated()`: "BoneFXUpdate requires BoneFXDamage" (`INI_INVALID_DATA` without one).
    fn on_object_created(world: &mut World, me: ModuleHandle) {
        if world.find_module(me.object, b"BoneFXDamage").is_none() {
            // DEBUG_CRASH(("BoneFXUpdate requires BoneFXDamage"));
            world
                .cuts
                .module_errors
                .push((me, IniError::new(IniErrorKind::InvalidData)));
        }
    }

    /// `initTimes()`: the current state's effects that have a bone get their first frame, the others none.
    fn init_times(&mut self, world: &mut World) -> Result<(), Undefined> {
        let s = self.state_index()?;
        let now = world.frame() as i32;
        let d = self.data.clone();
        for i in 0..BONE_FX_MAX_BONES {
            for kind in [KIND_FX, KIND_OCL, KIND_PARTICLE] {
                let info = &d.lists[kind][s][i];
                self.next_frame[kind][s][i] = if info.bone_name.is_empty() {
                    -1
                } else if kind == KIND_PARTICLE {
                    now.wrapping_add(real_to_int(
                        info.game_client_delay.get_value(&mut world.random),
                    ))
                } else {
                    now.wrapping_add(real_to_int(
                        info.game_logic_delay.get_value(&mut world.random),
                    ))
                };
            }
        }
        Ok(())
    }

    /// `computeNextClientFXTime(info, nextFrame)` and `computeNextLogicFXTime`: none for an effect played once, else now plus a delay.
    fn compute_next_time(world: &mut World, kind: usize, info: &BoneListInfo, next: &mut i32) {
        if info.only_once && !crate::logic::modules::plant(2002) {
            *next = -1;
            return;
        }
        let delay = if (kind == KIND_PARTICLE) != crate::logic::modules::plant(2003) {
            info.game_client_delay.get_value(&mut world.random)
        } else {
            info.game_logic_delay.get_value(&mut world.random)
        };
        *next = (world.frame() as i32).wrapping_add(real_to_int(delay));
    }

    /// `changeBodyDamageState(oldState, newState)`.
    fn change_body_damage_state(
        &mut self,
        world: &mut World,
        new_state: i32,
    ) -> Result<(), Undefined> {
        self.cur_body_state = new_state;
        if !crate::logic::modules::plant(2005) {
            self.kill_running_particle_systems(world);
        }
        self.init_times(world)
    }

    /// `killRunningParticleSystems()`: each system the manager still has is destroyed.
    fn kill_running_particle_systems(&mut self, world: &mut World) {
        for &id in &self.particle_system_ids {
            world.body_particle_destroy(id);
        }
        self.particle_system_ids.clear();
    }

    /// `stopAllBoneFX()`: no effect of any state is due any more, and the particle systems go.
    fn stop_all_bone_fx(&mut self, world: &mut World) {
        self.next_frame = [[[-1; BONE_FX_MAX_BONES]; BODYDAMAGETYPE_COUNT]; 3];
        self.kill_running_particle_systems(world);
    }

    /// `resolveBoneLocations()`: "This function is going to suck lots of time, should only be called once."
    fn resolve_bone_locations(&mut self, world: &mut World, id: ObjectId) -> Result<(), Undefined> {
        let s = self.state_index()?;
        let d = self.data.clone();
        let has_drawable = world.has_drawable(id);
        for i in 0..BONE_FX_MAX_BONES {
            for kind in [KIND_FX, KIND_OCL, KIND_PARTICLE] {
                let info = &d.lists[kind][s][i];
                if info.bone_name.is_empty() {
                    continue;
                }
                // DEBUG_ASSERTCRASH(drawable != NULL, ("There is no drawable?")), then `drawable->getPristineBonePositions(...)`
                if !has_drawable {
                    return Err(Undefined::NullDrawable);
                }
                // (a bone the art does not have writes nothing)
                if let Some(p) = world.contain_drawable_pristine_bone(id, &info.bone_name) {
                    self.bone_positions[kind][s][i] = p;
                }
            }
        }
        self.bones_resolved[s] = !crate::logic::modules::plant(2006);
        Ok(())
    }

    /// The head of `doFXListAtBone`, `doOCLAtBone` and `doParticleSystemAtBone`: the bones, then whether the last damage's type lets the effect
    /// play ("if we are restricted by the damage type executing effect, bail out of here").
    fn may_play(
        &mut self,
        world: &mut World,
        id: ObjectId,
        kind: usize,
    ) -> Result<bool, Undefined> {
        let s = self.state_index()?;
        if !self.bones_resolved[s] {
            self.resolve_bone_locations(world, id)?;
        }
        let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
        if let ModuleReply::Damage(last) =
            world.module_call(id, body, ModuleCall::GetLastDamageInfo)
        {
            if !self.data.damage_types[kind].test(last.input.damage_type)
                && !crate::logic::modules::plant(2001)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `doFXListAtBone`, `doOCLAtBone` or `doParticleSystemAtBone` of entry `i` of the current state.
    fn do_effect_at_bone(
        &mut self,
        world: &mut World,
        id: ObjectId,
        kind: usize,
        i: usize,
    ) -> Result<(), Undefined> {
        if !self.may_play(world, id, kind)? {
            return Ok(());
        }
        let s = self.state_index()?;
        let bone = self.bone_positions[kind][s][i];
        let effect = self.data.lists[kind][s][i].effect.clone();
        match kind {
            KIND_FX => {
                // "Convert the bone's position relative to the origin of the building to the current bone position in the world."
                let position = if crate::logic::modules::plant(2004) {
                    bone
                } else {
                    world.contain_convert_bone_pos_to_world_pos(id, &bone)
                };
                if let Some(fx) = effect {
                    world.emit(ObjectClientEvent::FxListDoFxPos { fx, position });
                }
            }
            KIND_OCL => {
                let position = world.contain_convert_bone_pos_to_world_pos(id, &bone);
                if let Some(ocl) = effect {
                    let found = world
                        .statics
                        .clone()
                        .object_creation_list_store
                        .find_object_creation_list(&mut world.names, &ocl)
                        .is_some();
                    if found {
                        world.cut(CutCall::OclCreateAtPrimary {
                            primary: id,
                            ocl,
                            primary_pos: position,
                            angle: INVALID_ANGLE,
                        });
                    }
                }
            }
            _ => {
                // `createParticleSystem(NULL)` makes none
                if let Some(template) = effect {
                    let sys = world.create_particle_system(template, bone);
                    self.particle_system_ids.push(sys);
                    world.body_particle_attach_to_object(sys, id);
                    if world.has_drawable(id) && !crate::logic::modules::plant(2009) {
                        world.emit(ObjectClientEvent::ParticleSystemStopIfDrawableHidden {
                            id: sys,
                            object_id: id,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let now = world.frame() as i32;
        if !self.active {
            self.init_times(world)?;
            self.active = true;
        }
        let d = self.data.clone();
        for i in 0..BONE_FX_MAX_BONES {
            // "Check to see if its time to fire off any cool stuff."
            for kind in [KIND_FX, KIND_OCL, KIND_PARTICLE] {
                let s = self.state_index()?;
                let next = self.next_frame[kind][s][i];
                if next != -1
                    && (next <= now || (crate::logic::modules::plant(2007) && next == now + 1))
                {
                    self.do_effect_at_bone(world, me.object, kind, i)?;
                    let mut n = self.next_frame[kind][s][i];
                    Self::compute_next_time(world, kind, &d.lists[kind][s][i], &mut n);
                    self.next_frame[kind][s][i] = n;
                }
            }
        }
        Ok(())
    }
}

impl UpdateBehavior for BoneFxUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// "@todo srj use SLEEPY_UPDATE here".
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(u) = self.update_inner(world, me) {
            Self::undefined(world, me, u);
        }
        UPDATE_SLEEP_NONE
    }
    /// The constructor makes no wake call.
    fn construct(&mut self, _world: &mut World, _me: ModuleHandle) {}
    /// The fields after the base, in order (see the module docs).
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        // particle system vector count and data
        let mut count = self.particle_system_ids.len() as u16;
        xfer.xfer_unsigned_short(&mut count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for id in &self.particle_system_ids {
                let mut b = id.to_le_bytes();
                xfer.xfer_user(&mut b)?;
            }
        } else {
            // the list should be emtpy right now
            if !self.particle_system_ids.is_empty() {
                // DEBUG_CRASH(( "BoneFXUpdate::xfer - m_particleSystemIDs should be empty but is not\n" )); throw SC_INVALID_DATA
                return Err(XferStatus::ListNotEmpty);
            }
            for _ in 0..count {
                let mut b = [0u8; 4];
                xfer.xfer_user(&mut b)?;
                if !crate::logic::modules::plant(2008) {
                    self.particle_system_ids.push(u32::from_le_bytes(b));
                }
            }
        }
        for kind in [KIND_FX, KIND_OCL, KIND_PARTICLE] {
            let mut b = int_bytes(&self.next_frame, kind);
            xfer.xfer_user(&mut b)?;
            for (k, c) in b.chunks(4).enumerate() {
                self.next_frame[kind][k / BONE_FX_MAX_BONES][k % BONE_FX_MAX_BONES] =
                    i32::from_le_bytes(c.try_into().expect("four bytes"));
            }
        }
        for kind in [KIND_FX, KIND_OCL, KIND_PARTICLE] {
            let mut b = coord_bytes(&self.bone_positions, kind);
            xfer.xfer_user(&mut b)?;
            for (k, c) in b.chunks(12).enumerate() {
                let f = |j: usize| {
                    f32::from_le_bytes(c[4 * j..4 * j + 4].try_into().expect("four bytes"))
                };
                self.bone_positions[kind][k / BONE_FX_MAX_BONES][k % BONE_FX_MAX_BONES] = Coord3D {
                    x: f(0),
                    y: f(1),
                    z: f(2),
                };
            }
        }
        let mut b = self.cur_body_state.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.cur_body_state = i32::from_le_bytes(b);
        let mut b = self.bones_resolved.map(u8::from);
        xfer.xfer_user(&mut b)?;
        self.bones_resolved = b.map(|x| x != 0);
        xfer.xfer_bool(&mut self.active)
    }
    /// The bone FX calls and `onObjectCreated`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::BoneFxChangeBodyDamageState { new_state, .. } => {
                if let Err(u) = self.change_body_damage_state(world, new_state) {
                    Self::undefined(world, me, u);
                }
                ModuleReply::Done
            }
            ModuleCall::BoneFxStopAll => {
                self.stop_all_bone_fx(world);
                ModuleReply::Done
            }
            ModuleCall::OnObjectCreated => {
                Self::on_object_created(world, me);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::BoneFxUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::BoneFxUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for BoneFxUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `BoneFXUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::BoneFxUpdate(Arc::new(
        BoneFxUpdateModuleData::friend_new_module_data(ini)?,
    )))
}

/// `class BoneFXDamage : public DamageModule`: tells the object's `BoneFXUpdate` when the body damage state changes.
#[derive(Clone, Debug)]
pub struct BoneFxDamage {
    /// The `DamageModule` base.
    damage: DamageModule,
}

impl PartialEq for BoneFxDamage {
    /// The same object.
    fn eq(&self, other: &Self) -> bool {
        self.damage.behavior().object() == other.damage.behavior().object()
    }
}

impl BoneFxDamage {
    /// `BoneFXDamage(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<DamageModuleData>>) -> IniResult<Self> {
        Ok(BoneFxDamage {
            damage: DamageModule::new(object, module_data)?,
        })
    }

    /// `getInterfaceMask()`: the damage module's.
    pub const fn get_interface_mask() -> i32 {
        DamageModule::get_interface_mask()
    }

    /// The `DamageModule` base.
    pub fn damage_module(&self) -> &DamageModule {
        &self.damage
    }

    /// The module's answer to one of `Object`'s calls: `onObjectCreated` ("BoneFXDamage requires BoneFXUpdate": `INI_INVALID_DATA`), `onDamage`
    /// and `onHealing` (nothing), `onBodyDamageStateChange` ("Switching damage states": the object's `BoneFXUpdate` changes state).
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                if world.find_module(me.object, b"BoneFXUpdate").is_none() {
                    world
                        .cuts
                        .module_errors
                        .push((me, IniError::new(IniErrorKind::InvalidData)));
                }
                ModuleReply::Done
            }
            ModuleCall::OnDamage(_) | ModuleCall::OnHealing(_) => ModuleReply::Done,
            ModuleCall::OnBodyDamageStateChange {
                old_state,
                new_state,
                ..
            } => {
                if let Some(bfxu) = world.find_module(me.object, b"BoneFXUpdate") {
                    world.module_call(
                        me.object,
                        bfxu,
                        ModuleCall::BoneFxChangeBodyDamageState {
                            old_state,
                            new_state,
                        },
                    );
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for BoneFxDamage {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.damage.crc(xfer)
    }
    /// Version 1: the version, `DamageModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.damage.xfer(xfer)
    }
    fn load_post_process(&mut self) {
        self.damage.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = BoneFxUpdateModuleData::default();
        assert!(d
            .lists
            .iter()
            .flatten()
            .flatten()
            .all(|i| i.only_once && i.bone_name.is_empty() && i.effect.is_none()));
        let m = BoneFxUpdate::unconstructed(3, Some(Arc::new(d))).unwrap();
        assert!(m.next_frame.iter().flatten().flatten().all(|&f| f == -1));
        assert!(!m.active && m.cur_body_state == 0 && m.particle_system_ids.is_empty());
        assert_eq!(
            BoneFxUpdate::unconstructed(3, None).unwrap_err().kind,
            IniErrorKind::InvalidData
        );
    }
}
