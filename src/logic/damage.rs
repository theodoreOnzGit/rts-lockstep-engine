// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Damage.h (lines 50-160: the
//   damage types and DamageTypeFlags; lines 165-252: the death types,
//   TheDeathNames and DeathTypeFlags)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Damage.h (lines 255-352:
//   DamageInfoInput, DamageInfoOutput, DamageInfo, HUGE_DAMAGE_AMOUNT)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/Damage.cpp (lines
//   42-83: DamageTypeFlags::s_bitNameList, DAMAGE_TYPE_FLAGS_NONE/ALL;
//   the xfer of DamageInfo, DamageInfoInput and DamageInfoOutput)
//   GeneralsMD/Code/GameEngine/Include/Common/BitFlags.h (the
//   getSingleBitFromName/set/flip members DamageTypeFlags uses)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Damage.cpp: Colin Day, September 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Damage and death types and their flag sets (`parseDamageTypeFlags`,
//! `parseDeathTypeFlags`), and `DamageInfo` (`in`/`out`) with its `Xfer`.
//!
//! # Cut point: `DamageInfoInput::m_sourceTemplate`
//!
//! Upstream holds a `const ThingTemplate*` and, on load, looks it up by name
//! in `TheThingFactory`. Here the field is the template's **name**
//! (`source_template`, empty for `NULL`); a load stores the name read and
//! does not resolve it (the thing system is a later layer, which resolves
//! it, and treats a name it does not know as `NULL`, as `findTemplate`
//! does).
//!
//! # Where upstream is undefined
//!
//! `DamageType` is a plain `int` here, as the enum's values are used as
//! array indices. [`is_subdual_damage`] and [`is_health_damaging_damage`]
//! are `switch`es and take any value. Functions that index a table by a
//! `DamageType` (`ArmorTemplate::adjustDamage`, the `DamageFX` getters)
//! return an error for a value outside `[0, DAMAGE_NUM_TYPES)`.
//!
//! # Where upstream is wrong, kept
//!
//! `DamageInfoInput::xfer` reads and writes `m_kill` under
//! `if (currentVersion >= 2)`, the constant 3, not the stream's `version`:
//! a version-1 stream is read as if it had the byte (and so is misread).
//! Reproduced.

use crate::common::base_type::Coord3D;
use crate::common::bit_flags::{BitFlagNames, BitFlags};
use crate::common::game_common::PlayerMaskType;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_NONE};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
pub use crate::common::bit_flags::BitIndexOutOfRange;

/// `DAMAGE_NUM_TYPES`.
pub const DAMAGE_NUM_TYPES: usize = 38;

/// `enum DamageType`: an `int` in the Rust (see the module docs).
pub type DamageType = i32;

/// `DAMAGE_EXPLOSION`.
pub const DAMAGE_EXPLOSION: DamageType = 0;
/// `DAMAGE_CRUSH`.
pub const DAMAGE_CRUSH: DamageType = 1;
/// `DAMAGE_ARMOR_PIERCING`.
pub const DAMAGE_ARMOR_PIERCING: DamageType = 2;
/// `DAMAGE_SMALL_ARMS`.
pub const DAMAGE_SMALL_ARMS: DamageType = 3;
/// `DAMAGE_GATTLING`.
pub const DAMAGE_GATTLING: DamageType = 4;
/// `DAMAGE_RADIATION`.
pub const DAMAGE_RADIATION: DamageType = 5;
/// `DAMAGE_FLAME`.
pub const DAMAGE_FLAME: DamageType = 6;
/// `DAMAGE_LASER`.
pub const DAMAGE_LASER: DamageType = 7;
/// `DAMAGE_SNIPER`.
pub const DAMAGE_SNIPER: DamageType = 8;
/// `DAMAGE_POISON`.
pub const DAMAGE_POISON: DamageType = 9;
/// `DAMAGE_HEALING`.
pub const DAMAGE_HEALING: DamageType = 10;
/// `DAMAGE_UNRESISTABLE`.
pub const DAMAGE_UNRESISTABLE: DamageType = 11;
/// `DAMAGE_WATER`.
pub const DAMAGE_WATER: DamageType = 12;
/// `DAMAGE_DEPLOY`.
pub const DAMAGE_DEPLOY: DamageType = 13;
/// `DAMAGE_SURRENDER`.
pub const DAMAGE_SURRENDER: DamageType = 14;
/// `DAMAGE_HACK`.
pub const DAMAGE_HACK: DamageType = 15;
/// `DAMAGE_KILLPILOT`.
pub const DAMAGE_KILLPILOT: DamageType = 16;
/// `DAMAGE_PENALTY`.
pub const DAMAGE_PENALTY: DamageType = 17;
/// `DAMAGE_FALLING`.
pub const DAMAGE_FALLING: DamageType = 18;
/// `DAMAGE_MELEE`.
pub const DAMAGE_MELEE: DamageType = 19;
/// `DAMAGE_DISARM`.
pub const DAMAGE_DISARM: DamageType = 20;
/// `DAMAGE_HAZARD_CLEANUP`.
pub const DAMAGE_HAZARD_CLEANUP: DamageType = 21;
/// `DAMAGE_PARTICLE_BEAM`.
pub const DAMAGE_PARTICLE_BEAM: DamageType = 22;
/// `DAMAGE_TOPPLING`.
pub const DAMAGE_TOPPLING: DamageType = 23;
/// `DAMAGE_INFANTRY_MISSILE`.
pub const DAMAGE_INFANTRY_MISSILE: DamageType = 24;
/// `DAMAGE_AURORA_BOMB`.
pub const DAMAGE_AURORA_BOMB: DamageType = 25;
/// `DAMAGE_LAND_MINE`.
pub const DAMAGE_LAND_MINE: DamageType = 26;
/// `DAMAGE_JET_MISSILES`.
pub const DAMAGE_JET_MISSILES: DamageType = 27;
/// `DAMAGE_STEALTHJET_MISSILES`.
pub const DAMAGE_STEALTHJET_MISSILES: DamageType = 28;
/// `DAMAGE_MOLOTOV_COCKTAIL`.
pub const DAMAGE_MOLOTOV_COCKTAIL: DamageType = 29;
/// `DAMAGE_COMANCHE_VULCAN`.
pub const DAMAGE_COMANCHE_VULCAN: DamageType = 30;
/// `DAMAGE_SUBDUAL_MISSILE`.
pub const DAMAGE_SUBDUAL_MISSILE: DamageType = 31;
/// `DAMAGE_SUBDUAL_VEHICLE`.
pub const DAMAGE_SUBDUAL_VEHICLE: DamageType = 32;
/// `DAMAGE_SUBDUAL_BUILDING`.
pub const DAMAGE_SUBDUAL_BUILDING: DamageType = 33;
/// `DAMAGE_SUBDUAL_UNRESISTABLE`.
pub const DAMAGE_SUBDUAL_UNRESISTABLE: DamageType = 34;
/// `DAMAGE_MICROWAVE`.
pub const DAMAGE_MICROWAVE: DamageType = 35;
/// `DAMAGE_KILL_GARRISONED`.
pub const DAMAGE_KILL_GARRISONED: DamageType = 36;
/// `DAMAGE_STATUS`.
pub const DAMAGE_STATUS: DamageType = 37;

/// `DamageTypeFlags::s_bitNameList[]` (Damage.cpp), without the `NULL`
/// terminator, in `enum DamageType` order. (The enum spells one of them
/// `DAMAGE_KILLPILOT`; the name is `KILL_PILOT`.)
pub const DAMAGE_TYPE_NAMES: [&str; DAMAGE_NUM_TYPES] = [
    "EXPLOSION",
    "CRUSH",
    "ARMOR_PIERCING",
    "SMALL_ARMS",
    "GATTLING",
    "RADIATION",
    "FLAME",
    "LASER",
    "SNIPER",
    "POISON",
    "HEALING",
    "UNRESISTABLE",
    "WATER",
    "DEPLOY",
    "SURRENDER",
    "HACK",
    "KILL_PILOT",
    "PENALTY",
    "FALLING",
    "MELEE",
    "DISARM",
    "HAZARD_CLEANUP",
    "PARTICLE_BEAM",
    "TOPPLING",
    "INFANTRY_MISSILE",
    "AURORA_BOMB",
    "LAND_MINE",
    "JET_MISSILES",
    "STEALTHJET_MISSILES",
    "MOLOTOV_COCKTAIL",
    "COMANCHE_VULCAN",
    "SUBDUAL_MISSILE",
    "SUBDUAL_VEHICLE",
    "SUBDUAL_BUILDING",
    "SUBDUAL_UNRESISTABLE",
    "MICROWAVE",
    "KILL_GARRISONED",
    "STATUS",
];

/// The name table of `DamageTypeFlags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DamageTypeNames;
impl BitFlagNames for DamageTypeNames {
    const NUM_BITS: usize = DAMAGE_NUM_TYPES;
    const NAMES: &'static [&'static str] = &DAMAGE_TYPE_NAMES;
}

/// `typedef BitFlags<DAMAGE_NUM_TYPES> DamageTypeFlags`. (Folded into the
/// generic [`BitFlags`] in Phase 1 layer 4; it was a hand-written `u64`.)
pub type DamageTypeFlags = BitFlags<DamageTypeNames>;

/// `DAMAGE_TYPE_FLAGS_NONE` ("inits to all zeroes").
pub const DAMAGE_TYPE_FLAGS_NONE: DamageTypeFlags = DamageTypeFlags::NONE;
/// `DAMAGE_TYPE_FLAGS_ALL` (`SET_ALL_DAMAGE_TYPE_BITS`: clear, flip).
pub const DAMAGE_TYPE_FLAGS_ALL: DamageTypeFlags = DamageTypeFlags::ALL;

/// `setDamageTypeFlag(flags, dt)`: `std::bitset::set(dt)` throws
/// `std::out_of_range` for `dt` outside `[0, DAMAGE_NUM_TYPES)` (an unknown
/// name gives -1, which converts to `SIZE_MAX`).
pub fn set_damage_type_flag(mut flags: DamageTypeFlags, dt: i32) -> Result<DamageTypeFlags, BitIndexOutOfRange> {
    flags.try_set(dt, 1)?;
    Ok(flags)
}

/// `clearDamageTypeFlag(flags, dt)`.
pub fn clear_damage_type_flag(mut flags: DamageTypeFlags, dt: i32) -> Result<DamageTypeFlags, BitIndexOutOfRange> {
    flags.try_set(dt, 0)?;
    Ok(flags)
}

/// `DEATH_NUM_TYPES`.
pub const DEATH_NUM_TYPES: usize = 21;

/// `TheDeathNames[]` (Damage.h, under `DEFINE_DEATH_NAMES`), without the
/// `NULL` terminator, in `enum DeathType` order.
pub const DEATH_NAMES: [&str; DEATH_NUM_TYPES] = [
    "NORMAL",
    "NONE",
    "CRUSHED",
    "BURNED",
    "EXPLODED",
    "POISONED",
    "TOPPLED",
    "FLOODED",
    "SUICIDED",
    "LASERED",
    "DETONATED",
    "SPLATTED",
    "POISONED_BETA",
    "EXTRA_2",
    "EXTRA_3",
    "EXTRA_4",
    "EXTRA_5",
    "EXTRA_6",
    "EXTRA_7",
    "EXTRA_8",
    "POISONED_GAMMA",
];

/// `typedef UnsignedInt DeathTypeFlags`.
pub type DeathTypeFlags = u32;
/// `DEATH_TYPE_FLAGS_ALL`.
pub const DEATH_TYPE_FLAGS_ALL: DeathTypeFlags = 0xffff_ffff;
/// `DEATH_TYPE_FLAGS_NONE`.
pub const DEATH_TYPE_FLAGS_NONE: DeathTypeFlags = 0;

/// `1UL << (dt - 1)` **as the shipped 32-bit x86 build computes it** (owner
/// decision, 2026-10-17). **Upstream bug:** for `DEATH_NORMAL` (0) the shift
/// count is -1, undefined behaviour in C++; the shipped game's `shl` masks the
/// count to 5 bits, so `NORMAL` is bit 31, consistently in get, set and
/// clear. This reproduces that with a count of `(dt - 1) & 31`. A 64-bit C++
/// build differs, so the differential tests cannot compare this case;
/// `death_flags` pins it instead.
fn death_bit(dt: i32) -> u32 {
    1u32 << (dt.wrapping_sub(1) as u32 & 31)
}

/// `getDeathTypeFlag`; `DEATH_NORMAL` is bit 31, as shipped (see `death_bit`).
pub fn get_death_type_flag(flags: DeathTypeFlags, dt: i32) -> bool {
    flags & death_bit(dt) != 0
}

/// `setDeathTypeFlag`: `flags | (1UL << (dt - 1))`; `DEATH_NORMAL` is bit
/// 31, as shipped.
pub fn set_death_type_flag(flags: DeathTypeFlags, dt: i32) -> DeathTypeFlags {
    flags | death_bit(dt)
}

/// `clearDeathTypeFlag`: `flags & ~(1UL << (dt - 1))`; `DEATH_NORMAL` is
/// bit 31, as shipped.
pub fn clear_death_type_flag(flags: DeathTypeFlags, dt: i32) -> DeathTypeFlags {
    flags & !death_bit(dt)
}

/// `IsSubdualDamage(type)`.
pub fn is_subdual_damage(t: DamageType) -> bool {
    matches!(t, DAMAGE_SUBDUAL_MISSILE | DAMAGE_SUBDUAL_VEHICLE | DAMAGE_SUBDUAL_BUILDING | DAMAGE_SUBDUAL_UNRESISTABLE)
}

/// `IsHealthDamagingDamage(type)`: does this type of damage go to
/// `internalChangeHealth`?
pub fn is_health_damaging_damage(t: DamageType) -> bool {
    !matches!(
        t,
        DAMAGE_STATUS
            | DAMAGE_SUBDUAL_MISSILE
            | DAMAGE_SUBDUAL_VEHICLE
            | DAMAGE_SUBDUAL_BUILDING
            | DAMAGE_SUBDUAL_UNRESISTABLE
            | DAMAGE_KILLPILOT
            | DAMAGE_KILL_GARRISONED
    )
}

/// `SET_ALL_DAMAGE_TYPE_BITS(m)`: clear, then flip.
pub fn set_all_damage_type_bits(m: &mut DamageTypeFlags) {
    m.clear();
    m.flip();
}

/// `HUGE_DAMAGE_AMOUNT`.
pub const HUGE_DAMAGE_AMOUNT: f32 = 999999.0;

/// `enum DeathType`, an `int` like [`DamageType`].
pub type DeathType = i32;
/// `DEATH_NORMAL`.
pub const DEATH_NORMAL: DeathType = 0;
/// `DEATH_NONE`: "a special case that can't normally cause death".
pub const DEATH_NONE: DeathType = 1;
// --- p1w13-sonnet ---
/// `DEATH_CRUSHED`.
pub const DEATH_CRUSHED: DeathType = 2;
/// `DEATH_BURNED`.
pub const DEATH_BURNED: DeathType = 3;
/// `DEATH_EXPLODED` (batch R49: the neutron missile's blasts).
pub const DEATH_EXPLODED: DeathType = 4;
/// `DEATH_DETONATED`: "the 'death' that occurs when a missile/warhead/etc detonates normally".
pub const DEATH_DETONATED: DeathType = 10;
/// `DEATH_EXTRA_4` (batch R47: the battle bus's empty-hulk kill).
pub const DEATH_EXTRA_4: DeathType = 15;
// --- end p1w13-sonnet ---

/// `class DamageInfoInput`: what the damage is (the inputs of a
/// [`DamageInfo`]). `source_template` is a name, see the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct DamageInfoInput {
    /// `m_sourceID`.
    pub source_id: ObjectId,
    /// `m_sourceTemplate`, by name; empty for `NULL`.
    pub source_template: Vec<u8>,
    /// `m_sourcePlayerMask`.
    pub source_player_mask: PlayerMaskType,
    /// `m_damageType`.
    pub damage_type: DamageType,
    /// `m_damageStatusType`.
    pub damage_status_type: ObjectStatusTypes,
    /// `m_damageFXOverride`.
    pub damage_fx_override: DamageType,
    /// `m_deathType`.
    pub death_type: DeathType,
    /// `m_amount`.
    pub amount: f32,
    /// `m_kill`.
    pub kill: bool,
    /// `m_shockWaveVector`.
    pub shock_wave_vector: Coord3D,
    /// `m_shockWaveAmount`.
    pub shock_wave_amount: f32,
    /// `m_shockWaveRadius`.
    pub shock_wave_radius: f32,
    /// `m_shockWaveTaperOff`.
    pub shock_wave_taper_off: f32,
}

impl Default for DamageInfoInput {
    /// `DamageInfoInput()`.
    fn default() -> Self {
        DamageInfoInput {
            source_id: INVALID_ID,
            source_template: Vec::new(),
            source_player_mask: 0,
            damage_type: DAMAGE_EXPLOSION,
            damage_status_type: OBJECT_STATUS_NONE,
            damage_fx_override: DAMAGE_UNRESISTABLE,
            death_type: DEATH_NORMAL,
            amount: 0.0,
            kill: false,
            shock_wave_vector: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
        }
    }
}

fn xfer_enum(xfer: &mut dyn Xfer, v: &mut i32) -> XferResult<()> {
    // xferUser(&v, sizeof(enum)): 4 raw bytes, little-endian.
    let mut b = v.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    *v = i32::from_le_bytes(b);
    Ok(())
}

impl Snapshot for DamageInfoInput {
    /// `crc` is empty upstream.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `DamageInfoInput::xfer` (version 3; 2 added the damage FX override,
    /// 3 the source template's name). See the module docs for `m_kill`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 3;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        xfer.xfer_object_id(&mut self.source_id)?;
        let mut mask = self.source_player_mask.to_le_bytes();
        xfer.xfer_user(&mut mask)?;
        self.source_player_mask = PlayerMaskType::from_le_bytes(mask);
        xfer_enum(xfer, &mut self.damage_type)?;
        if version >= 2 {
            xfer_enum(xfer, &mut self.damage_fx_override)?;
        }
        xfer_enum(xfer, &mut self.death_type)?;
        xfer.xfer_real(&mut self.amount)?;
        // upstream tests `currentVersion` here, which is always 3
        if current_version >= 2 {
            xfer.xfer_bool(&mut self.kill)?;
        }
        xfer_enum(xfer, &mut self.damage_status_type)?;
        xfer.xfer_coord3d(&mut self.shock_wave_vector)?;
        xfer.xfer_real(&mut self.shock_wave_amount)?;
        xfer.xfer_real(&mut self.shock_wave_radius)?;
        xfer.xfer_real(&mut self.shock_wave_taper_off)?;
        if version >= 3 {
            let mut thing_string = self.source_template.clone();
            xfer.xfer_ascii_string(&mut thing_string)?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.source_template = thing_string;
            }
        }
        Ok(())
    }

    fn load_post_process(&mut self) {}
}

/// `class DamageInfoOutput`: what the damage did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageInfoOutput {
    /// `m_actualDamageDealt`: the damage after multipliers and such.
    pub actual_damage_dealt: f32,
    /// `m_actualDamageClipped`: that, clipped to the health remaining.
    pub actual_damage_clipped: f32,
    /// `m_noEffect`: no damage was done at all.
    pub no_effect: bool,
}

impl Snapshot for DamageInfoOutput {
    /// `crc` is empty upstream.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `DamageInfoOutput::xfer` (version 1).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        xfer.xfer_real(&mut self.actual_damage_dealt)?;
        xfer.xfer_real(&mut self.actual_damage_clipped)?;
        xfer.xfer_bool(&mut self.no_effect)
    }

    fn load_post_process(&mut self) {}
}

/// `class DamageInfo`: a descriptor of damage being inflicted, inputs
/// (`in`) and results (`out`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageInfo {
    /// `in`.
    pub input: DamageInfoInput,
    /// `out`.
    pub out: DamageInfoOutput,
}

impl Snapshot for DamageInfo {
    /// `crc` is empty upstream.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `DamageInfo::xfer` (version 1): the input, then the output, each as
    /// a snapshot.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        xfer.xfer_snapshot(&mut self.input)?;
        xfer.xfer_snapshot(&mut self.out)
    }

    fn load_post_process(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_flags() {
        assert_eq!(DamageTypeFlags::get_single_bit_from_name(b"status"), 37);
        assert_eq!(DamageTypeFlags::get_single_bit_from_name(b"KILLPILOT"), -1);
        let mut f = DamageTypeFlags::NONE;
        f.flip();
        assert_eq!(f, DamageTypeFlags::ALL);
        assert_eq!(set_damage_type_flag(DamageTypeFlags::NONE, -1), Err(BitIndexOutOfRange));
        assert_eq!(clear_damage_type_flag(DamageTypeFlags::ALL, 0).unwrap().words()[0], (1u64 << DAMAGE_NUM_TYPES) - 2);
    }

    #[test]
    fn death_flags() {
        // DEATH_NORMAL (0) is bit 31, as the shipped 32-bit build computes it.
        assert_eq!(set_death_type_flag(0, 0), 1 << 31);
        assert_eq!(clear_death_type_flag(DEATH_TYPE_FLAGS_ALL, 0), 0x7fff_ffff);
        assert!(get_death_type_flag(1 << 31, 0));
        assert_eq!(set_death_type_flag(0, 20), 1 << 19);
        assert!(get_death_type_flag(1, 1));
    }
}
