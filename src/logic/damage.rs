// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Damage.h (lines 50-160: the
//   damage types and DamageTypeFlags; lines 165-252: the death types,
//   TheDeathNames and DeathTypeFlags)
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/Damage.cpp (lines
//   42-83: DamageTypeFlags::s_bitNameList, DAMAGE_TYPE_FLAGS_NONE/ALL)
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

//! Damage and death type flags, as far as the INI field parsers use them
//! (`parseDamageTypeFlags`, `parseDeathTypeFlags`). The `DamageInfo`
//! structures and their `Xfer` are not translated yet.

/// `DAMAGE_NUM_TYPES`.
pub const DAMAGE_NUM_TYPES: usize = 38;

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

/// `typedef BitFlags<DAMAGE_NUM_TYPES> DamageTypeFlags`: a `std::bitset` of
/// 38 bits, here the low 38 bits of a `u64` (bit `i` is damage type `i`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DamageTypeFlags(pub u64);

/// `std::bitset::set(pos, val)` was given `pos >= DAMAGE_NUM_TYPES`: it
/// throws `std::out_of_range` upstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitIndexOutOfRange;

const ALL_BITS: u64 = (1u64 << DAMAGE_NUM_TYPES) - 1;

impl DamageTypeFlags {
    /// `DAMAGE_TYPE_FLAGS_NONE` ("inits to all zeroes").
    pub const NONE: DamageTypeFlags = DamageTypeFlags(0);
    /// `DAMAGE_TYPE_FLAGS_ALL` (`SET_ALL_DAMAGE_TYPE_BITS`: clear, flip).
    pub const ALL: DamageTypeFlags = DamageTypeFlags(ALL_BITS);

    /// `BitFlags::getSingleBitFromName`: the index of the name in
    /// [`DAMAGE_TYPE_NAMES`] (compared with `stricmp`), or -1.
    pub fn get_single_bit_from_name(token: &[u8]) -> i32 {
        DAMAGE_TYPE_NAMES
            .iter()
            .position(|n| n.as_bytes().eq_ignore_ascii_case(token))
            .map_or(-1, |i| i as i32)
    }

    /// `std::bitset::test(pos)` (`pos` in range).
    pub fn test(&self, pos: usize) -> bool {
        pos < DAMAGE_NUM_TYPES && self.0 & (1u64 << pos) != 0
    }

    /// `std::bitset::set(pos, val)`: the `DamageType` argument converts to
    /// `size_t`, so -1 (an unknown name) becomes `SIZE_MAX` and throws.
    pub fn set(&mut self, pos: i32, val: bool) -> Result<(), BitIndexOutOfRange> {
        if pos < 0 || pos as usize >= DAMAGE_NUM_TYPES {
            return Err(BitIndexOutOfRange);
        }
        if val {
            self.0 |= 1u64 << pos;
        } else {
            self.0 &= !(1u64 << pos);
        }
        Ok(())
    }

    /// `std::bitset::flip()`.
    pub fn flip(&mut self) {
        self.0 = !self.0 & ALL_BITS;
    }
}

/// `setDamageTypeFlag(flags, dt)`.
pub fn set_damage_type_flag(mut flags: DamageTypeFlags, dt: i32) -> Result<DamageTypeFlags, BitIndexOutOfRange> {
    flags.set(dt, true)?;
    Ok(flags)
}

/// `clearDamageTypeFlag(flags, dt)`.
pub fn clear_damage_type_flag(mut flags: DamageTypeFlags, dt: i32) -> Result<DamageTypeFlags, BitIndexOutOfRange> {
    flags.set(dt, false)?;
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

/// `1UL << (dt - 1)`. **Upstream bug:** for `DEATH_NORMAL` (0) the shift
/// count is -1, undefined behaviour (bit 31 on the shipped 32-bit x86 build,
/// 0 on a 64-bit one). `None` there.
fn death_bit(dt: i32) -> Option<u32> {
    let shift = dt.checked_sub(1)?;
    if (0..32).contains(&shift) {
        Some(1u32 << shift)
    } else {
        None
    }
}

/// `getDeathTypeFlag`; `None` for `DEATH_NORMAL` (see [`set_death_type_flag`]).
pub fn get_death_type_flag(flags: DeathTypeFlags, dt: i32) -> Option<bool> {
    death_bit(dt).map(|b| flags & b != 0)
}

/// `setDeathTypeFlag`: `flags | (1UL << (dt - 1))`; `None` for
/// `DEATH_NORMAL`, where upstream shifts by -1.
pub fn set_death_type_flag(flags: DeathTypeFlags, dt: i32) -> Option<DeathTypeFlags> {
    death_bit(dt).map(|b| flags | b)
}

/// `clearDeathTypeFlag`: `flags & ~(1UL << (dt - 1))`; `None` for
/// `DEATH_NORMAL`.
pub fn clear_death_type_flag(flags: DeathTypeFlags, dt: i32) -> Option<DeathTypeFlags> {
    death_bit(dt).map(|b| flags & !b)
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
        assert_eq!(clear_damage_type_flag(DamageTypeFlags::ALL, 0).unwrap().0, ALL_BITS - 1);
    }

    #[test]
    fn death_flags() {
        assert_eq!(set_death_type_flag(0, 0), None);
        assert_eq!(set_death_type_flag(0, 20), Some(1 << 19));
        assert_eq!(get_death_type_flag(1, 1), Some(true));
    }
}
