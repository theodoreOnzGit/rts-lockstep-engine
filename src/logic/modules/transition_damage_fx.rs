// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/TransitionDamageFX.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Damage/TransitionDamageFX.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TransitionDamageFX`: effects for a change of body damage state. When the body's damage state changes, the particle systems this module made
//! for the old state are destroyed; when the new state is worse, its FX lists, object creation lists and particle systems play (up to twelve of
//! each, unless the last damage's type is one the data excludes), each at a fixed point of the object or at a bone of its art (a named bone,
//! or one picked at random among the bones of a prefix), and the new particle systems are kept for the next change.
//!
//! # Where the world comes in
//!
//! - The module is a damage module ([`ModuleCall::OnBodyDamageStateChange`]; `onDamage` and `onHealing` do nothing).
//! - The bones are the art's (`getPristineBonePositions`, the client's): `cuts.contain.pristine_bones`, through [`World::drawable_pristine_bones`]
//!   (a named bone, or the prefix's bones `<prefix>01`, `<prefix>02`, ... while the art has them); `convertBonePosToWorldPos` is
//!   [`World::contain_convert_bone_pos_to_world_pos`].
//! - `FXList::doFXPos` is [`ObjectClientEvent::FxListDoFxPos`]; `ObjectCreationList::create(ocl, obj, &pos, damageSource->getPosition(),
//!   INVALID_ANGLE)` is [`CutCall::OclCreateAt`]; the particle systems are [`World::create_particle_system`], `World::body_particle_attach_to_object`
//!   and [`World::destroy_particle_system_by_id`]. The body's last damage is [`ModuleCall::GetLastDamageInfo`].
//!
//! # Upstream facts kept
//!
//! - Only the damaged, really damaged and rubble states have entries; "worse" is a greater state (`IS_CONDITION_WORSE`).
//! - A location is a bone only with a drawable; a bone the art does not have, or a prefix with no bones, falls back on the entry's `Loc:` (zero
//!   unless an earlier line of the same field gave one: the entry is not reset). A random bone is a logic random number, drawn only when the
//!   effect plays.
//! - The OCL and particle system parsers are handed the entry itself as their store (`parseObjectCreationList(ini, instance, store, &info->ocl)`):
//!   the effect is the entry's first member, so the name lands where it belongs.
//! - The save is `xferUser` of the raw id array.
//!
//! # Where upstream is undefined
//!
//! An OCL that plays for a damage with no source object (`damageSource->getPosition()` through `NULL`, [`Undefined::NullObject`]); a worse state
//! with no body ([`Undefined::NullBodyModule`]); a state outside the four ([`Undefined::ArrayIndexOutOfRange`]). The case is recorded in
//! `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R31, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ANGLE;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData,
    SEPS_COLON,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DamageTypeFlags, DAMAGE_TYPE_FLAGS_ALL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::damage_module::{DamageModule, DamageModuleData};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;

/// `DAMAGE_MODULE_MAX_FX`.
pub const DAMAGE_MODULE_MAX_FX: usize = 12;
/// `BODYDAMAGETYPE_COUNT`.
pub const BODYDAMAGETYPE_COUNT: usize = 4;
/// `INVALID_PARTICLE_SYSTEM_ID`.
pub const INVALID_PARTICLE_SYSTEM_ID: u32 = 0;
/// `FX_DAMAGE_LOC_TYPE_BONE`.
pub const FX_DAMAGE_LOC_TYPE_BONE: i8 = 0;
/// `FX_DAMAGE_LOC_TYPE_COORD`.
pub const FX_DAMAGE_LOC_TYPE_COORD: i8 = 1;
/// The three kinds of effect, in the data's order: `m_fxList`, `m_OCL`, `m_particleSystem`.
pub const KIND_FX: usize = 0;
/// See [`KIND_FX`].
pub const KIND_OCL: usize = 1;
/// See [`KIND_FX`].
pub const KIND_PARTICLE: usize = 2;
/// `MAX_BONES` of `getLocalEffectPos`.
const MAX_BONES: i32 = 32;

/// `FXLocInfo` with its effect (`FXDamageFXListInfo::fx`, `FXDamageOCLInfo::ocl`, `FXDamageParticleSystemInfo::particleSysTemplate`, by name).
#[derive(Clone, Debug, PartialEq)]
pub struct FxEntry {
    /// The effect (`None`: `NULL`; an FX list or a particle system named `None`). An OCL is looked up when it plays.
    pub effect: Option<Vec<u8>>,
    /// `locType`: "location is bone or coord".
    pub loc_type: i8,
    /// `boneName`.
    pub bone_name: Vec<u8>,
    /// `randomBone`: "if true, boneName is a prefix and we will do the effect at one of the bones with that prefix".
    pub random_bone: bool,
    /// `loc`: "location is a specified point in space".
    pub loc: Coord3D,
}

impl Default for FxEntry {
    /// `TransitionDamageFXModuleData()`'s: no effect, at the origin.
    fn default() -> Self {
        FxEntry {
            effect: None,
            loc_type: FX_DAMAGE_LOC_TYPE_COORD,
            bone_name: Vec::new(),
            random_bone: false,
            loc: Coord3D::default(),
        }
    }
}

/// `class TransitionDamageFXModuleData : public DamageModuleData`.
#[derive(Clone, Debug)]
pub struct TransitionDamageFxModuleData {
    /// The `DamageModuleData` base.
    pub damage: DamageModuleData,
    /// `m_damageFXTypes`, `m_damageOCLTypes`, `m_damageParticleTypes`: "flags used to play or not play the effects".
    pub damage_types: [DamageTypeFlags; 3],
    /// `m_fxList`, `m_OCL`, `m_particleSystem`: `[kind][body damage state][index]`.
    pub lists: [[[FxEntry; DAMAGE_MODULE_MAX_FX]; BODYDAMAGETYPE_COUNT]; 3],
}

impl Default for TransitionDamageFxModuleData {
    /// `TransitionDamageFXModuleData()`: every damage type plays every effect.
    fn default() -> Self {
        TransitionDamageFxModuleData {
            damage: DamageModuleData::new(),
            damage_types: [DAMAGE_TYPE_FLAGS_ALL; 3],
            lists: Default::default(),
        }
    }
}

/// `parseFXLocInfo(ini, instance, locInfo)`: "Parse fx location info ... that is a named bone or a coord3d position".
fn parse_fx_loc_info(ini: &mut Ini<'_>, info: &mut FxEntry) -> IniResult<()> {
    let token = ini.get_next_token(Some(SEPS_COLON))?;
    if token.eq_ignore_ascii_case(b"bone") {
        // save bone name and location type
        info.bone_name = ini.get_next_token(None)?;
        info.loc_type = FX_DAMAGE_LOC_TYPE_BONE;
        // "bones are followed by RandomBone:<Yes|No>"
        let token = ini.get_next_token(Some(SEPS_COLON))?;
        if !token.eq_ignore_ascii_case(b"randombone") {
            // DEBUG_CRASH(( "parseFXLocInfo: Bone name not followed by RandomBone specifier ..." ));
            return Err(IniError::new(IniErrorKind::InvalidData));
        }
        info.random_bone = ini.parse_bool()?;
    } else if token.eq_ignore_ascii_case(b"loc") {
        // save location and location type
        let t = ini.get_next_sub_token(b"X")?;
        info.loc.x = Ini::scan_real(&t)?;
        let t = ini.get_next_sub_token(b"Y")?;
        info.loc.y = Ini::scan_real(&t)?;
        let t = ini.get_next_sub_token(b"Z")?;
        info.loc.z = Ini::scan_real(&t)?;
        info.loc_type = FX_DAMAGE_LOC_TYPE_COORD;
    } else {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    Ok(())
}

/// `parseFXList`, `parseObjectCreationList` and `parseParticleSystem` for entry `kind * 64 + state * 16 + index` (the field's user data):
/// `<Bone:<name> RandomBone:<Yes|No> | Loc: X:x Y:y Z:z> FXList:<name>` (`OCL:`, `PSys:`).
fn parse_entry(
    ini: &mut Ini<'_>,
    d: &mut TransitionDamageFxModuleData,
    u: &UserData,
) -> IniResult<()> {
    let UserData::Mask(m) = u else {
        return Err(IniError::new(IniErrorKind::InvalidData));
    };
    let (kind, state, index) = (
        (*m / 64) as usize,
        (*m / 16 % 4) as usize,
        (*m % 16) as usize,
    );
    let info = &mut d.lists[kind][state][index];
    parse_fx_loc_info(ini, info)?;
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
static FIELD_PARSE: [FieldParse<TransitionDamageFxModuleData>; 112] = [
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
    FieldParse::new("DamagedFXList1", parse_entry, UserData::Mask(16)),
    FieldParse::new("DamagedFXList2", parse_entry, UserData::Mask(17)),
    FieldParse::new("DamagedFXList3", parse_entry, UserData::Mask(18)),
    FieldParse::new("DamagedFXList4", parse_entry, UserData::Mask(19)),
    FieldParse::new("DamagedFXList5", parse_entry, UserData::Mask(20)),
    FieldParse::new("DamagedFXList6", parse_entry, UserData::Mask(21)),
    FieldParse::new("DamagedFXList7", parse_entry, UserData::Mask(22)),
    FieldParse::new("DamagedFXList8", parse_entry, UserData::Mask(23)),
    FieldParse::new("DamagedFXList9", parse_entry, UserData::Mask(24)),
    FieldParse::new("DamagedFXList10", parse_entry, UserData::Mask(25)),
    FieldParse::new("DamagedFXList11", parse_entry, UserData::Mask(26)),
    FieldParse::new("DamagedFXList12", parse_entry, UserData::Mask(27)),
    FieldParse::new("ReallyDamagedFXList1", parse_entry, UserData::Mask(32)),
    FieldParse::new("ReallyDamagedFXList2", parse_entry, UserData::Mask(33)),
    FieldParse::new("ReallyDamagedFXList3", parse_entry, UserData::Mask(34)),
    FieldParse::new("ReallyDamagedFXList4", parse_entry, UserData::Mask(35)),
    FieldParse::new("ReallyDamagedFXList5", parse_entry, UserData::Mask(36)),
    FieldParse::new("ReallyDamagedFXList6", parse_entry, UserData::Mask(37)),
    FieldParse::new("ReallyDamagedFXList7", parse_entry, UserData::Mask(38)),
    FieldParse::new("ReallyDamagedFXList8", parse_entry, UserData::Mask(39)),
    FieldParse::new("ReallyDamagedFXList9", parse_entry, UserData::Mask(40)),
    FieldParse::new("ReallyDamagedFXList10", parse_entry, UserData::Mask(41)),
    FieldParse::new("ReallyDamagedFXList11", parse_entry, UserData::Mask(42)),
    FieldParse::new("ReallyDamagedFXList12", parse_entry, UserData::Mask(43)),
    FieldParse::new("RubbleFXList1", parse_entry, UserData::Mask(48)),
    FieldParse::new("RubbleFXList2", parse_entry, UserData::Mask(49)),
    FieldParse::new("RubbleFXList3", parse_entry, UserData::Mask(50)),
    FieldParse::new("RubbleFXList4", parse_entry, UserData::Mask(51)),
    FieldParse::new("RubbleFXList5", parse_entry, UserData::Mask(52)),
    FieldParse::new("RubbleFXList6", parse_entry, UserData::Mask(53)),
    FieldParse::new("RubbleFXList7", parse_entry, UserData::Mask(54)),
    FieldParse::new("RubbleFXList8", parse_entry, UserData::Mask(55)),
    FieldParse::new("RubbleFXList9", parse_entry, UserData::Mask(56)),
    FieldParse::new("RubbleFXList10", parse_entry, UserData::Mask(57)),
    FieldParse::new("RubbleFXList11", parse_entry, UserData::Mask(58)),
    FieldParse::new("RubbleFXList12", parse_entry, UserData::Mask(59)),
    FieldParse::new("DamagedOCL1", parse_entry, UserData::Mask(80)),
    FieldParse::new("DamagedOCL2", parse_entry, UserData::Mask(81)),
    FieldParse::new("DamagedOCL3", parse_entry, UserData::Mask(82)),
    FieldParse::new("DamagedOCL4", parse_entry, UserData::Mask(83)),
    FieldParse::new("DamagedOCL5", parse_entry, UserData::Mask(84)),
    FieldParse::new("DamagedOCL6", parse_entry, UserData::Mask(85)),
    FieldParse::new("DamagedOCL7", parse_entry, UserData::Mask(86)),
    FieldParse::new("DamagedOCL8", parse_entry, UserData::Mask(87)),
    FieldParse::new("DamagedOCL9", parse_entry, UserData::Mask(88)),
    FieldParse::new("DamagedOCL10", parse_entry, UserData::Mask(89)),
    FieldParse::new("DamagedOCL11", parse_entry, UserData::Mask(90)),
    FieldParse::new("DamagedOCL12", parse_entry, UserData::Mask(91)),
    FieldParse::new("ReallyDamagedOCL1", parse_entry, UserData::Mask(96)),
    FieldParse::new("ReallyDamagedOCL2", parse_entry, UserData::Mask(97)),
    FieldParse::new("ReallyDamagedOCL3", parse_entry, UserData::Mask(98)),
    FieldParse::new("ReallyDamagedOCL4", parse_entry, UserData::Mask(99)),
    FieldParse::new("ReallyDamagedOCL5", parse_entry, UserData::Mask(100)),
    FieldParse::new("ReallyDamagedOCL6", parse_entry, UserData::Mask(101)),
    FieldParse::new("ReallyDamagedOCL7", parse_entry, UserData::Mask(102)),
    FieldParse::new("ReallyDamagedOCL8", parse_entry, UserData::Mask(103)),
    FieldParse::new("ReallyDamagedOCL9", parse_entry, UserData::Mask(104)),
    FieldParse::new("ReallyDamagedOCL10", parse_entry, UserData::Mask(105)),
    FieldParse::new("ReallyDamagedOCL11", parse_entry, UserData::Mask(106)),
    FieldParse::new("ReallyDamagedOCL12", parse_entry, UserData::Mask(107)),
    FieldParse::new("RubbleOCL1", parse_entry, UserData::Mask(112)),
    FieldParse::new("RubbleOCL2", parse_entry, UserData::Mask(113)),
    FieldParse::new("RubbleOCL3", parse_entry, UserData::Mask(114)),
    FieldParse::new("RubbleOCL4", parse_entry, UserData::Mask(115)),
    FieldParse::new("RubbleOCL5", parse_entry, UserData::Mask(116)),
    FieldParse::new("RubbleOCL6", parse_entry, UserData::Mask(117)),
    FieldParse::new("RubbleOCL7", parse_entry, UserData::Mask(118)),
    FieldParse::new("RubbleOCL8", parse_entry, UserData::Mask(119)),
    FieldParse::new("RubbleOCL9", parse_entry, UserData::Mask(120)),
    FieldParse::new("RubbleOCL10", parse_entry, UserData::Mask(121)),
    FieldParse::new("RubbleOCL11", parse_entry, UserData::Mask(122)),
    FieldParse::new("RubbleOCL12", parse_entry, UserData::Mask(123)),
    FieldParse::new("DamagedParticleSystem1", parse_entry, UserData::Mask(144)),
    FieldParse::new("DamagedParticleSystem2", parse_entry, UserData::Mask(145)),
    FieldParse::new("DamagedParticleSystem3", parse_entry, UserData::Mask(146)),
    FieldParse::new("DamagedParticleSystem4", parse_entry, UserData::Mask(147)),
    FieldParse::new("DamagedParticleSystem5", parse_entry, UserData::Mask(148)),
    FieldParse::new("DamagedParticleSystem6", parse_entry, UserData::Mask(149)),
    FieldParse::new("DamagedParticleSystem7", parse_entry, UserData::Mask(150)),
    FieldParse::new("DamagedParticleSystem8", parse_entry, UserData::Mask(151)),
    FieldParse::new("DamagedParticleSystem9", parse_entry, UserData::Mask(152)),
    FieldParse::new("DamagedParticleSystem10", parse_entry, UserData::Mask(153)),
    FieldParse::new("DamagedParticleSystem11", parse_entry, UserData::Mask(154)),
    FieldParse::new("DamagedParticleSystem12", parse_entry, UserData::Mask(155)),
    FieldParse::new(
        "ReallyDamagedParticleSystem1",
        parse_entry,
        UserData::Mask(160),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem2",
        parse_entry,
        UserData::Mask(161),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem3",
        parse_entry,
        UserData::Mask(162),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem4",
        parse_entry,
        UserData::Mask(163),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem5",
        parse_entry,
        UserData::Mask(164),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem6",
        parse_entry,
        UserData::Mask(165),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem7",
        parse_entry,
        UserData::Mask(166),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem8",
        parse_entry,
        UserData::Mask(167),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem9",
        parse_entry,
        UserData::Mask(168),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem10",
        parse_entry,
        UserData::Mask(169),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem11",
        parse_entry,
        UserData::Mask(170),
    ),
    FieldParse::new(
        "ReallyDamagedParticleSystem12",
        parse_entry,
        UserData::Mask(171),
    ),
    FieldParse::new("RubbleParticleSystem1", parse_entry, UserData::Mask(176)),
    FieldParse::new("RubbleParticleSystem2", parse_entry, UserData::Mask(177)),
    FieldParse::new("RubbleParticleSystem3", parse_entry, UserData::Mask(178)),
    FieldParse::new("RubbleParticleSystem4", parse_entry, UserData::Mask(179)),
    FieldParse::new("RubbleParticleSystem5", parse_entry, UserData::Mask(180)),
    FieldParse::new("RubbleParticleSystem6", parse_entry, UserData::Mask(181)),
    FieldParse::new("RubbleParticleSystem7", parse_entry, UserData::Mask(182)),
    FieldParse::new("RubbleParticleSystem8", parse_entry, UserData::Mask(183)),
    FieldParse::new("RubbleParticleSystem9", parse_entry, UserData::Mask(184)),
    FieldParse::new("RubbleParticleSystem10", parse_entry, UserData::Mask(185)),
    FieldParse::new("RubbleParticleSystem11", parse_entry, UserData::Mask(186)),
    FieldParse::new("RubbleParticleSystem12", parse_entry, UserData::Mask(187)),
    FieldParse::end(),
];

impl TransitionDamageFxModuleData {
    /// `buildFieldParse(p)`: the damage module base, then this class's fields.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, TransitionDamageFxModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<TransitionDamageFxModuleData> {
        let mut data = TransitionDamageFxModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                TransitionDamageFxModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class TransitionDamageFX : public DamageModule`.
#[derive(Clone, Debug)]
pub struct TransitionDamageFx {
    /// The `DamageModule` base.
    damage: DamageModule,
    /// `getTransitionDamageFXModuleData()`.
    data: Arc<TransitionDamageFxModuleData>,
    /// `m_particleSystemID`: the systems made for each state, `INVALID_PARTICLE_SYSTEM_ID` for none.
    particle_system_id: [[u32; DAMAGE_MODULE_MAX_FX]; BODYDAMAGETYPE_COUNT],
}

impl PartialEq for TransitionDamageFx {
    /// The same object, the same data object and the same ids.
    fn eq(&self, other: &Self) -> bool {
        self.damage.behavior().object() == other.damage.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.particle_system_id == other.particle_system_id
    }
}

/// A state as an index of the arrays.
fn state_index(s: i32) -> Result<usize, Undefined> {
    usize::try_from(s)
        .ok()
        .filter(|&s| s < BODYDAMAGETYPE_COUNT)
        .ok_or(Undefined::ArrayIndexOutOfRange)
}

impl TransitionDamageFx {
    /// `TransitionDamageFX(thing, moduleData)`; `INI_INVALID_DATA` for null data.
    pub fn new(
        object: ObjectId,
        module_data: Option<Arc<TransitionDamageFxModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let damage = DamageModule::new(object, Some(Arc::new(data.damage.clone())))?;
        Ok(TransitionDamageFx {
            damage,
            data,
            particle_system_id: [[INVALID_PARTICLE_SYSTEM_ID; DAMAGE_MODULE_MAX_FX];
                BODYDAMAGETYPE_COUNT],
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

    /// The module data (`getTransitionDamageFXModuleData()`).
    pub fn module_data(&self) -> &Arc<TransitionDamageFxModuleData> {
        &self.data
    }

    /// The ids, state by state (the test compares them).
    pub fn state_words(&self) -> Vec<u32> {
        self.particle_system_id.iter().flatten().copied().collect()
    }

    /// `getLocalEffectPos(locInfo, draw)`: "Given an FXLoc info struct, return the effect position that we are supposed to use. The position is
    /// local to to the object".
    fn get_local_effect_pos(world: &mut World, id: ObjectId, info: &FxEntry) -> Coord3D {
        if info.loc_type == FX_DAMAGE_LOC_TYPE_BONE && world.has_drawable(id) {
            if !info.random_bone {
                // get the bone position; "sanity, if bone not found revert back to location defined in struct (which is 0,0,0)"
                return world
                    .drawable_pristine_bones(id, &info.bone_name, 0, 1)
                    .first()
                    .copied()
                    .unwrap_or(if crate::logic::modules::plant(2014) {
                        Coord3D::default()
                    } else {
                        info.loc
                    });
            }
            let positions = world.drawable_pristine_bones(id, &info.bone_name, 1, MAX_BONES);
            if positions.is_empty() {
                return info.loc;
            }
            // pick one of the bone positions
            let pick = if crate::logic::modules::plant(2013) {
                0
            } else {
                world
                    .random
                    .game_logic_random_value(0, positions.len() as i32 - 1)
            };
            return positions[pick as usize];
        }
        info.loc
    }

    /// `onBodyDamageStateChange(damageInfo, oldState, newState)`: "Switching damage states".
    fn on_body_damage_state_change(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        info: &DamageInfo,
        old_state: i32,
        new_state: i32,
    ) -> Result<(), Undefined> {
        let id = me.object;
        let d = self.data.clone();
        // get the source of the damage if present
        let damage_source = world
            .objects
            .find(info.input.source_id)
            .map(|o| o.get_position());
        // remove any particle systems that might be emitting from our old state
        let old = state_index(old_state)?;
        for i in 0..DAMAGE_MODULE_MAX_FX {
            if self.particle_system_id[old][i] != INVALID_PARTICLE_SYSTEM_ID
                && !crate::logic::modules::plant(2012)
            {
                world.destroy_particle_system_by_id(self.particle_system_id[old][i]);
                self.particle_system_id[old][i] = INVALID_PARTICLE_SYSTEM_ID;
            }
        }
        // "when we are transitioning to a "worse" state we will play a set of effects for that new state to make the transition"
        if new_state <= old_state && !(crate::logic::modules::plant(2011) && new_state == old_state)
        {
            return Ok(());
        }
        let new = state_index(new_state)?;
        // if we are restricted by the damage type executing effect, bail out of here
        let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
        let last_type = match world.module_call(id, body, ModuleCall::GetLastDamageInfo) {
            ModuleReply::Damage(last) => Some(last.input.damage_type),
            _ => None,
        };
        let plays = |kind: usize| {
            crate::logic::modules::plant(2015)
                || last_type.is_none_or(|t| d.damage_types[kind].test(t))
        };
        for i in 0..DAMAGE_MODULE_MAX_FX {
            // play fx list for our new state
            let e = &d.lists[KIND_FX][new][i];
            if let Some(fx) = &e.effect {
                if plays(KIND_FX) {
                    let pos = Self::get_local_effect_pos(world, id, e);
                    let position = if crate::logic::modules::plant(2017) {
                        pos
                    } else {
                        world.contain_convert_bone_pos_to_world_pos(id, &pos)
                    };
                    world.emit(ObjectClientEvent::FxListDoFxPos {
                        fx: fx.clone(),
                        position,
                    });
                }
            }
            // do any object creation list for our new state
            let e = &d.lists[KIND_OCL][new][i];
            if let Some(ocl) = &e.effect {
                let found = world
                    .statics
                    .clone()
                    .object_creation_list_store
                    .find_object_creation_list(&mut world.names, ocl)
                    .is_some();
                if found && plays(KIND_OCL) {
                    let pos = Self::get_local_effect_pos(world, id, e);
                    let primary_pos = world.contain_convert_bone_pos_to_world_pos(id, &pos);
                    let secondary_pos = damage_source.ok_or(Undefined::NullObject)?;
                    let secondary_pos = if crate::logic::modules::plant(2016) {
                        primary_pos
                    } else {
                        secondary_pos
                    };
                    world.cut(CutCall::OclCreateAt {
                        primary: id,
                        ocl: ocl.clone(),
                        primary_pos,
                        secondary_pos,
                        angle: INVALID_ANGLE,
                    });
                }
            }
            // get the template of the system to create
            let e = &d.lists[KIND_PARTICLE][new][i];
            if let Some(template) = &e.effect {
                if plays(KIND_PARTICLE) {
                    // "the bone position is local to the object and that's what we want for the particle system"
                    let pos = Self::get_local_effect_pos(world, id, e);
                    let sys = world.create_particle_system(template.clone(), pos);
                    world.body_particle_attach_to_object(sys, id);
                    // save the id of this particle system so we can remove it later if it still exists
                    if !crate::logic::modules::plant(2019) {
                        self.particle_system_id[new][i] = sys;
                    }
                }
            }
        }
        Ok(())
    }

    /// The module's answer to one of `Object`'s calls: `onDamage` and `onHealing` (nothing), `onBodyDamageStateChange`.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        match call {
            ModuleCall::OnDamage(_) | ModuleCall::OnHealing(_) => ModuleReply::Done,
            ModuleCall::OnBodyDamageStateChange {
                info,
                old_state,
                new_state,
            } => {
                if let Err(u) =
                    self.on_body_damage_state_change(world, me, &info, old_state, new_state)
                {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for TransitionDamageFx {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.damage.crc(xfer)
    }
    /// Version 1: the version, `DamageModule::xfer`, the ids (`xferUser` of the array).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.damage.xfer(xfer)?;
        let mut b: Vec<u8> = self
            .particle_system_id
            .iter()
            .flatten()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        let r = if crate::logic::modules::plant(2018) {
            Ok(())
        } else {
            xfer.xfer_user(&mut b)
        };
        for (k, c) in b.chunks(4).enumerate() {
            self.particle_system_id[k / DAMAGE_MODULE_MAX_FX][k % DAMAGE_MODULE_MAX_FX] =
                u32::from_le_bytes(c.try_into().expect("four bytes"));
        }
        r
    }
    fn load_post_process(&mut self) {
        self.damage.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `TransitionDamageFX`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::TransitionDamageFx(Arc::new(
        TransitionDamageFxModuleData::friend_new_module_data(ini)?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = TransitionDamageFxModuleData::default();
        assert!(d
            .lists
            .iter()
            .flatten()
            .flatten()
            .all(|e| e.loc_type == FX_DAMAGE_LOC_TYPE_COORD
                && e.effect.is_none()
                && !e.random_bone));
        let m = TransitionDamageFx::new(3, Some(Arc::new(d))).unwrap();
        assert!(m
            .state_words()
            .iter()
            .all(|&w| w == INVALID_PARTICLE_SYSTEM_ID));
        assert_eq!(
            TransitionDamageFx::new(3, None).unwrap_err().kind,
            IniErrorKind::InvalidData
        );
    }
}
