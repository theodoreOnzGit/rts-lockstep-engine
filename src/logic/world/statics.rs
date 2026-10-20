// SPDX-License-Identifier: GPL-3.0-only
//
// Not translated from one upstream file: the world's replacement for the
// INI-store globals of EA's release (https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590), such as `TheUpgradeCenter`
// (GeneralsMD/Code/GameEngine/Include/Common/Upgrade.h).
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this file), with the additional terms in NOTICE.md.

//! The read-only data the game loads before it starts, as the world sees it.
//!
//! # What this replaces
//!
//! Upstream reaches its INI stores through globals: `TheUpgradeCenter`,
//! `TheThingFactory`, `TheSpecialPowerStore`, `TheScienceStore`, ... Modules
//! call them from anywhere (`CommandSetUpgrade::upgradeImplementation` calls
//! `TheUpgradeCenter->findUpgrade`). There are no globals here: the stores
//! that are **only read once the game runs** live in one [`GameStatics`], and
//! the world holds it as `world.statics`, an `Arc` (shared, never written
//! after load; see `docs/world-design.md`, "Statics").
//!
//! `TheNameKeyGenerator` is not read-only: `nameToKey` assigns a key to a name
//! it has not seen, and modules call it at run time (every `findUpgrade` by
//! name does). It is a field of the world of its own, `world.names`, and is
//! the **same generator the INI load used**, so keys agree.
//!
//! # Adding a store
//!
//! A layer-9 module that needs another store adds it as a field of
//! [`GameStatics`] (in its marked block) the first time one needs it, with
//! its `new()` in [`GameStatics::new`]. Only stores with no run-time writes
//! belong here; one that changes during a game (the `WeaponStore`'s delayed
//! damage list, for example) is a field of the world instead.
//!
//! Lead decision, 2026-10-20.

use crate::common::upgrade::UpgradeCenter;

/// The stores upstream reaches through globals and only reads once the game
/// runs. See the module docs.
#[derive(Clone, Debug, Default)]
pub struct GameStatics {
    /// `TheUpgradeCenter`.
    pub upgrade_center: UpgradeCenter,
    // New stores go here, each in its translator's marked block.
}

impl GameStatics {
    /// Empty stores, as before any INI is loaded.
    pub fn new() -> GameStatics {
        GameStatics::default()
    }
}
