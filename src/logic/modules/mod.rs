// SPDX-License-Identifier: GPL-3.0-only
//! Layer-9 concrete object modules, one file per upstream module
//! (`GameLogic/Object/*`), each made on its layer-5 base (`logic::module`).
//! The calls a module makes into the rest of the game go through a host trait
//! (generic, no `dyn`), see [`create_host`].

pub mod create_host;

// --- p1l9-creates ---
pub mod grant_upgrade_create;
pub mod lock_weapon_create;
pub mod preorder_create;
pub mod special_power_create;
pub mod supply_center_create;
pub mod supply_warehouse_create;
pub mod veterancy_gain_create;
// --- end p1l9-creates ---
