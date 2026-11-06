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
    /// Integration batch I19: the control bar's command buttons and sets (`TheControlBar`'s data the logic reads).
    pub command_buttons: crate::logic::command_button::CommandButtonStore,
    // --- p1l9-upgrades ---
    /// `TheScienceStore`.
    pub science_store: crate::common::science::ScienceStore,
    // --- end p1l9-upgrades ---
    // --- p1w11-sonnet ---
    /// `TheSpecialPowerStore` (`UnpauseSpecialPowerUpgrade` finds its template in it).
    pub special_power_store: crate::common::special_power::SpecialPowerStore,
    /// `TheThingFactory` (`ReplaceObjectUpgrade` finds its replacement in it, and `World::thing_factory_new_object` makes the object).
    pub thing_factory: crate::common::thing_factory::ThingFactory,
    /// `TheObjectCreationListStore` (`EjectPilotDie` finds out whether its lists exist).
    pub object_creation_list_store: crate::logic::object_creation_list::ObjectCreationListStore,
    /// `TheWeaponStore` (`FireWeaponWhenDeadBehavior` finds its death weapon in it).
    pub weapon_store: crate::logic::weapon::WeaponStore,
    // --- end p1w11-sonnet ---
    // --- p1w13-sonnet ---
    /// `TheArmorStore` (`ActiveBody::validateArmorAndDamageFX` makes its armor from it).
    pub armor_store: crate::logic::armor::ArmorStore,
    /// `TheDamageFXStore` (`ActiveBody::doDamageFX` finds the damage FX of its armor set in it).
    pub damage_fx_store: crate::common::damage_fx::DamageFxStore,
    /// `TheCrateSystem` (`CreateCrateDie` finds its crate templates in it).
    pub crate_system: crate::logic::crate_system::CrateSystem,
    /// batch W1b: `TheGlobalData` (the weapon code reads its weapon bonus set and historic damage limit; empty: `NULL`).
    pub global_data: crate::common::global_data::GlobalDataStore,
    // --- end p1w13-sonnet ---
    // --- batch I3 ---
    /// `ThePlayerTemplateStore` (a player's template is its index here).
    pub player_template_store: crate::common::player_template::PlayerTemplateStore,
    /// `TheRankInfoStore`.
    pub rank_info_store: crate::logic::rank_info::RankInfoStore,
    // --- end batch I3 ---
    // --- batch I9b ---
    /// `TheLocomotorStore` (an AI's locomotor sets are made from it).
    pub locomotor_store: crate::logic::locomotor::LocomotorStore,
    /// `TheAI->getAiData()` (`TAiData`'s constructor values until `AIData.ini` is read): its scalars (`data`) and, since batch
    /// I9e, its side infos and build lists (`lists`), which the computer players read.
    pub ai_data: crate::logic::ai::ai_data::AiDataEntry,
    // --- end batch I9b ---
    // --- batch I50 ---
    /// `TheTerrainRoads` (`Roads.ini`'s road and bridge types; `BridgeBehavior` and `TerrainLogic` read the bridges).
    pub terrain_roads: crate::common::terrain_roads::TerrainRoadCollection,
    // --- end batch I50 ---
    // --- batch I56 ---
    /// `TheMultiplayerSettings` (`Multiplayer.ini`: the player colours, shroud, beacons, starting money).
    pub multiplayer_settings: crate::common::multiplayer_settings::MultiplayerSettings,
    /// `TheMapCache` (each map's player count, start waypoints, size and CRC). Made by `updateCache` at engine start
    /// ([`crate::logic::world::game_logic::World::update_map_cache`]), read during a game.
    pub map_cache: crate::common::map_cache::MapCache,
    /// Batch I57: `TheGameText` (the install's string file; a test's table, or none).
    pub game_text: crate::logic::world::game_data::TextTable,
    /// Batch I59 (#16): `TheTerrainTypes` (`Terrain.ini`: each terrain tile's class, and whether it restricts construction).
    pub terrain_types: crate::common::terrain_types::TerrainTypeCollection,
    // --- end batch I56 ---
    // --- GitHub #50 ---
    /// `TheAudio`'s descriptions (`AudioManager::init`'s files, read by [`crate::logic::world::game_data::load_game_data`]
    /// with no user preferences): the logic reads a logical event's `Sounds`, `Attack`, `Decay`, `Control` and type
    /// to make upstream's logic draws (`crate::logic::logical_audio`). The client reads its own copy of the same files.
    pub audio: crate::client::audio_info::AudioTables,
    // --- end GitHub #50 ---
    // --- GitHub #52 ---
    /// `TheAudio->getAudioLengthMS(&AudioEventRTS(name))` of every described event, by name, as the script engine's
    /// `isSpeechComplete`/`isAudioComplete` read it: made at load from the files' headers, the first file of each list
    /// (`crate::logic::audio_length`). An event not listed is 0. Empty for a load without the install's audio.
    pub audio_lengths_ms: std::collections::BTreeMap<Vec<u8>, f32>,
    // --- end GitHub #52 ---
    // New stores go here, each in its translator's marked block.
}

impl GameStatics {
    /// Empty stores, as before any INI is loaded.
    pub fn new() -> GameStatics {
        GameStatics::default()
    }
}
