// SPDX-License-Identifier: GPL-3.0-only
//! Translations from `GameEngine/Source/Common` and `GameEngine/Include/Common`
//! (and, for now, the fast float helpers of `Libraries/Include/Lib/BaseType.h`,
//! which the whole engine includes, and `Libraries/Include/Lib/trig.h`).

pub mod base_type;
pub mod bit_flags;
pub mod crc;
pub mod dict;
pub mod disabled_types;
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
