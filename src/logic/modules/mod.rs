// SPDX-License-Identifier: GPL-3.0-only
//! Layer-9 object modules of the helper and die families (`Object/Helper/`,
//! `Object/Die/`). Each module is pure logic: it returns the object-side
//! effects it causes, in upstream's call order, as a list the caller applies.

// --- p1l9-helpers ---
pub mod object_helper;
pub mod object_smc_helper;
pub mod object_repulsor_helper;
pub mod object_weapon_status_helper;
pub mod status_damage_helper;
pub mod temp_weapon_bonus_helper;
pub mod destroy_die;
pub mod keep_object_die;
pub mod upgrade_die;
// --- end p1l9-helpers ---
