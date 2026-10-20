// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the host trait of the layer-9 create modules. Upstream reaches the Player,
// PlayerList, ResourceGatheringManager, UpgradeCenter, ExperienceTracker and
// the WeaponSet of the object directly; those units are not all translated
// yet (Player is a shell), so each call the create modules make is a method
// of this trait. The differential fixture implements it on a recording stand-in;
// the World adapter is wired by the layer-9 coordinator.

//! [`CreateHost`]: the calls the create modules make into units outside
//! `CreateModule`, one method per upstream call, in upstream's order.
//!
//! Every method is named after the upstream call it stands for. Methods that
//! upstream answers with a null-checked pointer say so in their doc comment:
//! the null test happens **in the host**, so the module logic reads as the C++
//! does. The trait is generic (no trait objects), like the other host traits
//! of the workspace.

use crate::common::game_common::VeterancyLevel;
use crate::common::model_state::ModelConditionFlagType;
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::logic::weapon_set_state::WeaponLockType;
use crate::logic::world::ids::PlayerIndex;
use crate::common::game_type::ObjectId;

/// An upgrade template as the create modules need it: upstream's
/// `const UpgradeTemplate*` reduced to the two facts they read
/// (`getUpgradeType() == UPGRADE_TYPE_PLAYER`) and an identity the host can
/// find again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeRef {
    /// The host's identity of the template (its name key, for example).
    pub id: u32,
    /// `getUpgradeType() == UPGRADE_TYPE_PLAYER`.
    pub player_type: bool,
}

/// Why a create module's call cannot be carried out. Each is an upstream
/// undefined behaviour (a null dereference in a release build): the input is
/// sent to Rust only, which returns this error instead of crashing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateError {
    /// `getControllingPlayer()` is `NULL` where upstream dereferences it.
    NoControllingPlayer,
}

/// The calls of the create modules into the rest of the game.
pub trait CreateHost {
    /// `getObject()->getControllingPlayer()`: `None` for no player.
    fn controlling_player(&self, obj: ObjectId) -> Option<PlayerIndex>;

    /// `player->didPlayerPreorder()`.
    fn player_did_preorder(&self, player: PlayerIndex) -> bool;

    /// `player->hasScience(science)`, the science by its name. (Upstream holds
    /// a `ScienceType`; the name is what the INI carries until the science
    /// store is wired to the host.)
    fn player_has_science(&self, player: PlayerIndex, science: &[u8]) -> bool;

    /// `object->setModelConditionState(flag)`.
    fn set_model_condition_state(&mut self, obj: ObjectId, flag: ModelConditionFlagType);

    /// `object->clearModelConditionState(flag)`.
    fn clear_model_condition_state(&mut self, obj: ObjectId, flag: ModelConditionFlagType);

    /// `object->setWeaponLock(slot, lockType)`.
    fn set_weapon_lock(&mut self, obj: ObjectId, slot: i32, lock: WeaponLockType);

    /// The loop of `SpecialPowerCreate::onBuildComplete`: for every behaviour
    /// module of `obj` with a special power interface, `onSpecialPowerCreation()`,
    /// in module order.
    fn special_power_creation(&mut self, obj: ObjectId);

    /// `ThePlayerList == NULL`: `None`; otherwise `getPlayerCount()`.
    fn player_count(&self) -> Option<i32>;

    /// The body of the supply-center loop for player `index`:
    /// `getNthPlayer(index)`, skipped if `NULL`, then
    /// `getResourceGatheringManager()`, skipped if `NULL`, then
    /// `addSupplyCenter(obj)`.
    fn resource_add_supply_center(&mut self, index: PlayerIndex, obj: ObjectId);

    /// As [`CreateHost::resource_add_supply_center`], with `addSupplyWarehouse`.
    fn resource_add_supply_warehouse(&mut self, index: PlayerIndex, obj: ObjectId);

    /// `ExperienceTracker` of `obj` and `isTrainable()`: `None` for no tracker.
    fn experience_is_trainable(&self, obj: ObjectId) -> Option<bool>;

    /// `setMinVeterancyLevel(level)` of the tracker of `obj`.
    fn experience_set_min_veterancy_level(&mut self, obj: ObjectId, level: VeterancyLevel);

    /// `TheUpgradeCenter->findUpgrade(name)`: `None` if there is no such upgrade.
    fn find_upgrade(&self, name: &[u8]) -> Option<UpgradeRef>;

    /// `player->addUpgrade(upgrade, UPGRADE_STATUS_COMPLETE)`.
    fn player_add_upgrade_complete(&mut self, player: PlayerIndex, upgrade: UpgradeRef);

    /// `object->giveUpgrade(upgrade)`.
    fn object_give_upgrade(&mut self, obj: ObjectId, upgrade: UpgradeRef);

    /// `player->getAcademyStats()->recordUpgrade(upgrade, granted)`.
    fn academy_record_upgrade(&mut self, player: PlayerIndex, upgrade: UpgradeRef, granted: bool);

    /// `getObject()->getStatusBits()`.
    fn object_status_bits(&self, obj: ObjectId) -> ObjectStatusMaskType;
}

/// A recording [`CreateHost`] for the unit tests of the create modules: the
/// answers are fields, and every call is logged in order.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::common::game_common::VeterancyLevel;
    use crate::common::model_state::ModelConditionFlagType;

    #[derive(Default)]
    pub struct Recorder {
        pub controller: Option<PlayerIndex>,
        pub preorder: bool,
        pub sciences: Vec<Vec<u8>>,
        pub status: ObjectStatusMaskType,
        pub trainable: Option<bool>,
        pub players: Option<i32>,
        pub upgrades: Vec<(Vec<u8>, UpgradeRef)>,
        pub log: Vec<String>,
    }

    impl CreateHost for Recorder {
        fn controlling_player(&self, obj: ObjectId) -> Option<PlayerIndex> {
            let _ = obj;
            self.controller
        }
        fn player_did_preorder(&self, player: PlayerIndex) -> bool {
            let _ = player;
            self.preorder
        }
        fn player_has_science(&self, player: PlayerIndex, science: &[u8]) -> bool {
            let _ = player;
            self.sciences.iter().any(|s| s == science)
        }
        fn set_model_condition_state(&mut self, obj: ObjectId, flag: ModelConditionFlagType) {
            self.log.push(format!("set {obj} {flag}"));
        }
        fn clear_model_condition_state(&mut self, obj: ObjectId, flag: ModelConditionFlagType) {
            self.log.push(format!("clear {obj} {flag}"));
        }
        fn set_weapon_lock(&mut self, obj: ObjectId, slot: i32, lock: WeaponLockType) {
            self.log.push(format!("lock {obj} {slot} {lock}"));
        }
        fn special_power_creation(&mut self, obj: ObjectId) {
            self.log.push(format!("sp {obj}"));
        }
        fn player_count(&self) -> Option<i32> {
            self.players
        }
        fn resource_add_supply_center(&mut self, index: PlayerIndex, obj: ObjectId) {
            self.log.push(format!("center {index} {obj}"));
        }
        fn resource_add_supply_warehouse(&mut self, index: PlayerIndex, obj: ObjectId) {
            self.log.push(format!("warehouse {index} {obj}"));
        }
        fn experience_is_trainable(&self, obj: ObjectId) -> Option<bool> {
            let _ = obj;
            self.trainable
        }
        fn experience_set_min_veterancy_level(&mut self, obj: ObjectId, level: VeterancyLevel) {
            self.log.push(format!("minvet {obj} {}", level as i32));
        }
        fn find_upgrade(&self, name: &[u8]) -> Option<UpgradeRef> {
            self.upgrades.iter().find(|(n, _)| n == name).map(|(_, u)| *u)
        }
        fn player_add_upgrade_complete(&mut self, player: PlayerIndex, upgrade: UpgradeRef) {
            self.log.push(format!("player-upgrade {player} {}", upgrade.id));
        }
        fn object_give_upgrade(&mut self, obj: ObjectId, upgrade: UpgradeRef) {
            self.log.push(format!("object-upgrade {obj} {}", upgrade.id));
        }
        fn academy_record_upgrade(&mut self, player: PlayerIndex, upgrade: UpgradeRef, granted: bool) {
            self.log.push(format!("academy {player} {} {granted}", upgrade.id));
        }
        fn object_status_bits(&self, obj: ObjectId) -> ObjectStatusMaskType {
            let _ = obj;
            self.status
        }
    }
}
