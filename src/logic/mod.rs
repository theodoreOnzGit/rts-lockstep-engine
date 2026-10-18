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

// --- p1w1-geometry ---
pub mod client_event;
// --- p1w1-armor ---
pub mod armor;
// --- end p1w1-armor ---
// --- p1w2-terrain ---
pub mod crate_system;
// --- end p1w2-terrain ---
// --- p1w2-globaldata ---
pub mod weapon_bonus;
// --- end p1w2-globaldata ---
// --- p1w2-science ---
pub mod rank_info;
// --- end p1w2-science ---
// --- p1w3-thing ---
pub mod object_iter;
pub mod thing;
// --- end p1w3-thing ---
