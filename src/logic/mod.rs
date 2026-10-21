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
// --- p1w3-modules ---
pub mod module;
// --- end p1w3-modules ---
// --- p1w4-polygon ---
pub mod map_object;
pub mod polygon_trigger;
// --- end p1w4-polygon ---
// --- p1w4-thingtemplate ---
pub mod weapon_set;
// --- end p1w4-thingtemplate ---
// --- p1w4-sides ---
pub mod scripts;
// --- end p1w4-sides ---
// --- p1w4-sides ---
pub mod sides_list;
// --- end p1w4-sides ---
// --- p1w4-terrain ---
pub mod terrain_logic;
pub mod terrain_visual;
// --- end p1w4-terrain ---
// --- p1w5-world ---
pub mod world;
// --- end p1w5-world ---
// --- p1w6-locomotor ---
pub mod locomotor;
// --- end p1w6-locomotor ---

// --- p1w6-trackers ---
pub mod experience_tracker;
pub mod firing_tracker;
pub mod object_types;
// --- end p1w6-trackers ---

// --- p1w7-sonnet ---
pub mod weapon;
// --- end p1w7-sonnet ---

// --- p1w8-sonnet ---
pub mod action_manager;
pub mod weapon_fire;
pub mod weapon_host;
pub mod weapon_set_state;
// --- end p1w8-sonnet ---

// --- p1l9-example ---
pub mod modules;
// --- end p1l9-example ---

