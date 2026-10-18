// SPDX-License-Identifier: GPL-3.0-only
//! Translations from `GameEngine/Source/Common` and `GameEngine/Include/Common`
//! (and, for now, the fast float helpers of `Libraries/Include/Lib/BaseType.h`,
//! which the whole engine includes, and `Libraries/Include/Lib/trig.h`).

pub mod ascii_string;
pub mod base_type;
pub mod bit_flags;
pub mod crc;
pub mod dict;
pub mod disabled_types;
pub mod discrete_circle;
pub mod errors;
pub mod game_common;
pub mod game_type;
pub mod ini;
pub mod kind_of;
pub mod model_state;
pub mod name_key_generator;
pub mod object_status_types;
pub mod quick_trig;
pub mod random_value;
pub mod sparse_match_finder;
pub mod special_power_mask_type;
pub mod special_power_type;
pub mod thing_sort;
pub mod trig;
mod trig_tables;
pub mod well_known_keys;
pub mod xfer;
pub mod xfer_crc;
pub mod xfer_load;
pub mod xfer_save;

// --- p1w1-small ---
pub mod latch_restore;
pub mod overridable;
pub mod stl_typedefs;
pub mod subsystem_interface;
// --- p1w1-unicode ---
pub mod unicode_string;
// --- p1w1-partition ---
pub mod partition_solver;
// --- end p1w1-partition ---
// --- p1w1-geometry ---
pub mod bezier_segment;
pub mod geometry;
pub mod handicap;
pub mod money;
// --- p1w1-armor ---
pub mod damage_fx;
// --- end p1w1-armor ---
// --- p1w2-terrain ---
pub mod multiplayer_settings;
pub mod terrain_types;
// --- end p1w2-terrain ---
// --- p1w2-globaldata ---
pub mod global_data;
// --- end p1w2-globaldata ---
// --- p1w2-science ---
pub mod science;
// --- end p1w2-science ---
