// SPDX-License-Identifier: GPL-3.0-only
//! Translations from `GameEngine/Source/GameLogic` and
//! `GameEngine/Include/GameLogic`: so far the damage and death type flags of
//! `Damage.h` and the small enum headers (`Powers.h`, `WeaponSetType.h`,
//! `WeaponStatus.h`, `WeaponBonusConditionFlags.h`,
//! `ObjectScriptStatusBits.h`). Layer plan: `docs/phase1-porting-graph.md`.

pub mod armor_set;
pub mod damage;
pub mod object_script_status_bits;
pub mod powers;
pub mod weapon_set_flags;
pub mod weapon_types;
