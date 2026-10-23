// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/PlayerList.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerList.cpp
//   (the constructor, getNthPlayer, findPlayerWithNameKey, init, setLocalPlayer)
//   GeneralsMD/Code/GameEngine/Include/Common/Player.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp
//   (the data of `Player`, `KindOfPercentProductionChange`, `SpecialPowerReadyTimerType`, `PlayerRelationMap`)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BattlePlanUpdate.h (`BattlePlanBonuses`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The player arena (`PlayerList`'s array of `MAX_PLAYER_COUNT` players, its count and the local player) and
//! [`Player`], the data of `class Player`. The operations are the free functions of `player_ops.rs` (slice 1) and
//! `player_objects.rs` (slice 2) over the `PlayerHost` / `PlayerObjectHost` traits, the way `team_ops.rs` does `Team`;
//! `PlayerList`'s own functions and `Player`'s save and load follow. Wave 7, `p1w10-sonnet`.
//!
//! # What is kept from upstream
//!
//! - **Players are an array, indexed by player index.** `PlayerList` makes all `MAX_PLAYER_COUNT` (16) players in its
//!   constructor, whether or not the game uses them, and `getNthPlayer(i)` is `NULL` outside `0..16`
//!   ([`PlayerArena::get_nth`] returns `None`). Index 0 is the neutral player.
//! - **`m_playerCount` bounds name lookups, not indexing**: `findPlayerWithNameKey` walks `0..m_playerCount`; `update`,
//!   `newMap`, `updateTeamStates` walk all 16 ([`PlayerArena::iter`]).
//! - **The local player is a pointer that may be unset only before `init`**; `setLocalPlayer(NULL)` selects the neutral
//!   player. `becomingLocalPlayer(true/false)` (a notification to the UI) is the [`LocalPlayerChange`] that
//!   [`PlayerArena::set_local_player`] returns for the caller to send; the arena itself sends nothing.
//! - **`Player::Player(index)` ends in `init( NULL )`**, which reaches the name keys, the rank store, the game text and
//!   the score keeper: [`Player::new`] is the constructor's field assignments only, and `player_ops::player_init` is
//!   the rest (the arena's constructor cannot call it; whoever builds the world does, for all sixteen).
//! - **`m_mpStartIndex` is never assigned before `initFromDict`**: it is 0 here.

use std::collections::BTreeMap;

use super::academy_stats::AcademyStats;
use super::energy::Energy;
use super::mission_stats::MissionStats;
use super::score_keeper::ScoreKeeper;
use super::ids::{ObjectId, PlayerIndex, TeamId, TeamPrototypeId};
use super::resource_gathering_manager::ResourceGatheringManager;
use super::team::{relationship_from_i32, TeamRelationMap};
use super::tunnel_tracker::TunnelTracker;
use crate::common::game_common::{PlayerType, Relationship};
// --- p1w10-sonnet ---
use crate::common::handicap::Handicap;
use crate::common::kind_of::KindOfMaskType;
use crate::common::money::Money;
use crate::common::name_key_generator::NameKeyType;
use crate::common::player_template::{ProductionChangeMap, ProductionVeterancyMap};
use crate::common::science::ScienceType;
use crate::common::unicode_string::UnicodeString;
use crate::common::upgrade::{UpgradeMaskType, UpgradeStatusType};
use crate::logic::sides_list::BuildListInfo;
// --- end p1w10-sonnet ---
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus};

/// `MAX_PLAYER_COUNT` (`GameCommon.h`).
pub use crate::common::game_common::MAX_PLAYER_COUNT;

/// `class PlayerRelationMap`: the players a player or a team has a special relationship with (`Player.h`,
/// `Player.cpp`; wave 7, `p1w9-sonnet`). `m_map` is a `hash_map` upstream, ordered here (only `xfer` walks it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerRelationMap {
    /// `m_map`.
    pub map: BTreeMap<PlayerIndex, Relationship>,
}

impl Snapshot for PlayerRelationMap {
    /// `crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer`, version 1: the count (`UnsignedShort`), then each player index (`xferInt`) and relationship
    /// (`xferUser`, 4 bytes); a load assigns `m_map[playerIndex] = r`. A relationship outside `0..=2` is
    /// `XFER_INVALID_PARAMETERS` here (upstream stores any int in the `enum`).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current_version = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut count = self.map.len() as u16;
        xfer.xfer_unsigned_short(&mut count)?;

        if xfer.get_xfer_mode() == XferMode::Save {
            for (index, r) in self.map.iter() {
                let mut player_index = *index;
                xfer.xfer_int(&mut player_index)?;
                let mut rb = (*r as i32).to_le_bytes();
                xfer.xfer_user(&mut rb)?;
            }
        } else {
            for _ in 0..count {
                let mut player_index = 0;
                xfer.xfer_int(&mut player_index)?;
                let mut rb = [0u8; 4];
                xfer.xfer_user(&mut rb)?;
                let r = relationship_from_i32(i32::from_le_bytes(rb)).ok_or(XferStatus::InvalidParameters)?;
                self.map.insert(player_index, r);
            }
        }
        Ok(())
    }

    /// `loadPostProcess`: nothing.
    fn load_post_process(&mut self) {}
}

// --- p1w10-sonnet ---
/// `KindOfPercentProductionChange` (`Player.h`): one production-cost change by kind, with a reference count.
#[derive(Clone, Debug, PartialEq)]
pub struct KindOfPercentProductionChange {
    /// `m_kindOf`.
    pub kind_of: KindOfMaskType,
    /// `m_percent`.
    pub percent: f32,
    /// `m_ref`.
    pub ref_count: u32,
}

/// `SpecialPowerReadyTimerType` (`Player.h`): the shared ready frame of one special power.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialPowerReadyTimer {
    /// `m_templateID`.
    pub template_id: u32,
    /// `m_readyFrame`.
    pub ready_frame: u32,
}

/// `BattlePlanBonuses` (`BattlePlanUpdate.h`).
#[derive(Clone, Debug, PartialEq)]
pub struct BattlePlanBonuses {
    /// `m_armorScalar`.
    pub armor_scalar: f32,
    /// `m_bombardment`.
    pub bombardment: i32,
    /// `m_searchAndDestroy`.
    pub search_and_destroy: i32,
    /// `m_holdTheLine`.
    pub hold_the_line: i32,
    /// `m_sightRangeScalar`.
    pub sight_range_scalar: f32,
    /// `m_validKindOf`.
    pub valid_kind_of: KindOfMaskType,
    /// `m_invalidKindOf`.
    pub invalid_kind_of: KindOfMaskType,
}

/// `Squad` (`Squad.h`), not translated yet: a hotkey squad or the current selection is only known to exist. The
/// squad unit replaces this type when it lands (the object ids it holds are its own).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Squad {
    /// `m_objectIDs`.
    pub object_ids: Vec<ObjectId>,
    /// `m_objectsCached`: the `Object*` of the last `getAllObjects` / `getLiveObjects`, kept (as ids) because it is
    /// left filled and `Squad::xfer` on a load refuses a squad whose cache is not empty.
    pub objects_cached: Vec<ObjectId>,
}

/// `m_ai` (`AIPlayer*`): the AI is not translated; the player only knows whether it has one and of which class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiPlayerRef {
    /// `AISkirmishPlayer` (true) or `AIPlayer` (false).
    pub skirmish: bool,
}

/// `m_playerTemplate` (`const PlayerTemplate*`): the host's handle for a template.
pub type PlayerTemplateRef = u32;

/// `NUM_HOTKEY_SQUADS`.
pub const NUM_HOTKEY_SQUADS: usize = 10;

/// `ScienceVec` of `Player`: the sciences, in the order they were acquired.
pub type ScienceVec = Vec<ScienceType>;

/// `class Player` (`Player.h`): the persistent and transient state of one player. `Player::Player(index)` is
/// [`Player::new`]; the `init(NULL)` it ends with is `player_ops::player_init` (it reaches the host).
/// Wave 7, `p1w10-sonnet`.
#[derive(Clone, Debug)]
pub struct Player {
    /// `m_playerIndex`.
    pub player_index: PlayerIndex,
    /// `m_playerNameKey` (`NameKeyType`); 0 = `NAMEKEY_INVALID` until named.
    pub name_key: i32,
    /// `m_playerTemplate`.
    pub template: Option<PlayerTemplateRef>,
    /// `m_playerDisplayName`.
    pub display_name: UnicodeString,
    /// `m_handicap`.
    pub handicap: Handicap,
    /// `m_playerName`.
    pub player_name: Vec<u8>,
    /// `m_side`.
    pub side: Vec<u8>,
    /// `m_baseSide`.
    pub base_side: Vec<u8>,
    /// `m_playerType`.
    pub player_type: PlayerType,
    /// `m_money`.
    pub money: Money,
    /// `m_upgradeList`: newest first (`addUpgrade` puts the new one at the head).
    pub upgrades: Vec<PlayerUpgrade>,
    /// `m_radarCount`.
    pub radar_count: i32,
    /// `m_disableProofRadarCount`.
    pub disable_proof_radar_count: i32,
    /// `m_radarDisabled`.
    pub radar_disabled: bool,
    /// `m_bombardBattlePlans`.
    pub bombard_battle_plans: i32,
    /// `m_holdTheLineBattlePlans`.
    pub hold_the_line_battle_plans: i32,
    /// `m_searchAndDestroyBattlePlans`.
    pub search_and_destroy_battle_plans: i32,
    /// `m_battlePlanBonuses`.
    pub battle_plan_bonuses: Option<BattlePlanBonuses>,
    /// `m_upgradesInProgress`.
    pub upgrades_in_progress: UpgradeMaskType,
    /// `m_upgradesCompleted`.
    pub upgrades_completed: UpgradeMaskType,
    // --- p1w6-energy ---
    /// `m_energy` (`Energy`): the power bookkeeping. `init(owner)` is `Player::init`'s.
    pub energy: Energy,
    /// `m_scoreKeeper` (`ScoreKeeper`): `Player::init` calls `reset(m_playerIndex)`.
    pub score_keeper: ScoreKeeper,
    /// `m_stats` (`MissionStats`).
    pub mission_stats: MissionStats,
    /// `m_academyStats` (`AcademyStats`): `Player::init` calls `init(this)`.
    pub academy_stats: AcademyStats,
    // --- end p1w6-energy ---
    /// `m_pBuildList` and the entries after it.
    pub build_list: Vec<BuildListInfo>,
    /// `m_color`.
    pub color: u32,
    /// `m_nightColor`.
    pub night_color: u32,
    /// `m_productionCostChanges`.
    pub production_cost_changes: ProductionChangeMap,
    /// `m_productionTimeChanges`.
    pub production_time_changes: ProductionChangeMap,
    /// `m_productionVeterancyLevels`.
    pub production_veterancy_levels: ProductionVeterancyMap,
    /// `m_ai`.
    pub ai: Option<AiPlayerRef>,
    /// `m_mpStartIndex`: upstream leaves it uninitialised until `initFromDict`; 0 here.
    pub mp_start_index: i32,
    /// `m_resourceGatheringManager`.
    pub resource_gathering_manager: Option<ResourceGatheringManager>,
    /// `m_tunnelSystem`.
    pub tunnel_system: Option<TunnelTracker>,
    /// `m_defaultTeam`.
    pub default_team: Option<TeamId>,
    /// `m_sciences`.
    pub sciences: ScienceVec,
    /// `m_sciencesDisabled`.
    pub sciences_disabled: ScienceVec,
    /// `m_sciencesHidden`.
    pub sciences_hidden: ScienceVec,
    /// `m_rankLevel`.
    pub rank_level: i32,
    /// `m_skillPoints`.
    pub skill_points: i32,
    /// `m_sciencePurchasePoints`.
    pub science_purchase_points: i32,
    /// `m_levelUp`.
    pub level_up: i32,
    /// `m_levelDown`.
    pub level_down: i32,
    /// `m_generalName`.
    pub general_name: UnicodeString,
    /// `m_playerTeamPrototypes`.
    pub team_prototypes: Vec<TeamPrototypeId>,
    /// `m_playerRelations`.
    pub player_relations: PlayerRelationMap,
    /// `m_teamRelations`.
    pub team_relations: TeamRelationMap,
    /// `m_canBuildUnits`.
    pub can_build_units: bool,
    /// `m_canBuildBase`.
    pub can_build_base: bool,
    /// `m_observer`.
    pub observer: bool,
    /// `m_isPreorder`.
    pub is_preorder: bool,
    /// `m_skillPointsModifier`.
    pub skill_points_modifier: f32,
    /// `m_listInScoreScreen`.
    pub list_in_score_screen: bool,
    /// `m_unitsShouldHunt`.
    pub units_should_hunt: bool,
    /// `m_attackedBy[MAX_PLAYER_COUNT]`.
    pub attacked_by: [bool; MAX_PLAYER_COUNT as usize],
    /// `m_attackedFrame`.
    pub attacked_frame: u32,
    /// `m_cashBountyPercent`.
    pub cash_bounty_percent: f32,
    /// `m_kindOfPercentProductionChangeList`.
    pub kind_of_production_changes: Vec<KindOfPercentProductionChange>,
    /// `m_specialPowerReadyTimerList`.
    pub special_power_timers: Vec<SpecialPowerReadyTimer>,
    /// `m_squads[NUM_HOTKEY_SQUADS]`: empty before `init` (all `NULL`), then ten.
    pub squads: Vec<Squad>,
    /// `m_currentSelection`.
    pub current_selection: Option<Squad>,
    /// `m_isPlayerDead`.
    pub is_player_dead: bool,
    /// `m_logicalRetaliationModeEnabled`.
    pub logical_retaliation_mode_enabled: bool,
}

/// One `Upgrade` of the player's list (`Upgrade.h`): the template and its status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerUpgrade {
    /// `m_template` (the upgrade template's name key).
    pub template: NameKeyType,
    /// `m_status`.
    pub status: UpgradeStatusType,
}

impl Player {
    /// `Player::Player(playerIndex)`, up to the `init( NULL )` it ends with (which is `player_ops::player_init`):
    /// every field the constructor assigns, and a member's default for the rest.
    pub fn new(player_index: PlayerIndex) -> Player {
        Player {
            player_index,
            name_key: 0,
            template: None,
            display_name: UnicodeString::default(),
            handicap: Handicap::new(),
            player_name: Vec::new(),
            side: Vec::new(),
            base_side: Vec::new(),
            player_type: PlayerType::Human,
            money: Money::new(),
            upgrades: Vec::new(),
            radar_count: 0,
            disable_proof_radar_count: 0,
            radar_disabled: false,
            bombard_battle_plans: 0,
            hold_the_line_battle_plans: 0,
            search_and_destroy_battle_plans: 0,
            battle_plan_bonuses: None,
            upgrades_in_progress: UpgradeMaskType::default(),
            upgrades_completed: UpgradeMaskType::default(),
            // --- p1w6-energy ---
            energy: Energy::new(),
            score_keeper: ScoreKeeper::new(),
            mission_stats: MissionStats::new(),
            academy_stats: AcademyStats::default(),
            // --- end p1w6-energy ---
            build_list: Vec::new(),
            color: 0,
            night_color: 0,
            production_cost_changes: ProductionChangeMap::new(),
            production_time_changes: ProductionChangeMap::new(),
            production_veterancy_levels: ProductionVeterancyMap::new(),
            ai: None,
            mp_start_index: 0,
            resource_gathering_manager: None,
            tunnel_system: None,
            default_team: None,
            sciences: Vec::new(),
            sciences_disabled: Vec::new(),
            sciences_hidden: Vec::new(),
            rank_level: 0,
            skill_points: 0,
            science_purchase_points: 0,
            level_up: 0,
            level_down: 0,
            general_name: UnicodeString::default(),
            team_prototypes: Vec::new(),
            player_relations: PlayerRelationMap::default(),
            team_relations: TeamRelationMap::default(),
            can_build_units: true,
            can_build_base: true,
            observer: false,
            is_preorder: false,
            skill_points_modifier: 1.0,
            list_in_score_screen: false,
            units_should_hunt: false,
            attacked_by: [false; MAX_PLAYER_COUNT as usize],
            attacked_frame: 0,
            cash_bounty_percent: 0.0,
            kind_of_production_changes: Vec::new(),
            special_power_timers: Vec::new(),
            squads: Vec::new(),
            current_selection: None,
            is_player_dead: false,
            logical_retaliation_mode_enabled: false,
        }
    }
}
// --- end p1w10-sonnet ---

/// What `setLocalPlayer` tells the caller to notify (`becomingLocalPlayer`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalPlayerChange {
    /// `m_local->becomingLocalPlayer(false)`: the old local player, if any.
    pub lost: Option<PlayerIndex>,
    /// `player->becomingLocalPlayer(true)`.
    pub gained: PlayerIndex,
}

/// `PlayerList`.
#[derive(Clone, Debug)]
pub struct PlayerArena {
    /// `m_players[MAX_PLAYER_COUNT]`.
    players: Vec<Player>,
    /// `m_playerCount`.
    player_count: i32,
    /// `m_local`, by index.
    local: Option<PlayerIndex>,
}

impl Default for PlayerArena {
    fn default() -> Self {
        PlayerArena::new()
    }
}

impl PlayerArena {
    /// `PlayerList::PlayerList()`: 16 players, count 0, no local player.
    pub fn new() -> PlayerArena {
        PlayerArena { players: (0..MAX_PLAYER_COUNT).map(Player::new).collect(), player_count: 0, local: None }
    }

    /// `getNthPlayer(i)`: `None` outside `0..MAX_PLAYER_COUNT`.
    pub fn get_nth(&self, i: PlayerIndex) -> Option<&Player> {
        if !(0..MAX_PLAYER_COUNT).contains(&i) {
            return None;
        }
        self.players.get(i as usize)
    }

    /// `getNthPlayer(i)` for a caller that changes the player.
    pub fn get_nth_mut(&mut self, i: PlayerIndex) -> Option<&mut Player> {
        if !(0..MAX_PLAYER_COUNT).contains(&i) {
            return None;
        }
        self.players.get_mut(i as usize)
    }

    /// `getNeutralPlayer()`: player 0.
    pub fn neutral(&self) -> &Player {
        &self.players[0]
    }

    /// `getPlayerCount()`.
    pub fn player_count(&self) -> i32 {
        self.player_count
    }

    /// `m_playerCount = n` (`newGame` sets it as it adds the sides' players).
    pub fn set_player_count(&mut self, n: i32) {
        self.player_count = n.clamp(0, MAX_PLAYER_COUNT);
    }

    /// All `MAX_PLAYER_COUNT` players, in index order.
    pub fn iter(&self) -> impl Iterator<Item = &Player> {
        self.players.iter()
    }

    /// `findPlayerWithNameKey(key)`: the first of `0..m_playerCount` with the key.
    pub fn find_with_name_key(&self, key: i32) -> Option<&Player> {
        self.players.iter().take(self.player_count as usize).find(|p| p.name_key == key)
    }

    /// `getLocalPlayer()`: `None` before `init`.
    pub fn local(&self) -> Option<&Player> {
        self.local.and_then(|i| self.get_nth(i))
    }

    /// The local player's index.
    pub fn local_index(&self) -> Option<PlayerIndex> {
        self.local
    }

    /// `PlayerList::init()`: count 1, and the neutral player becomes local.
    /// Returns the notification `setLocalPlayer(m_players[0])` implies.
    pub fn init(&mut self) -> Option<LocalPlayerChange> {
        self.player_count = 1;
        self.set_local_player(Some(0))
    }

    /// `setLocalPlayer(player)`: `None` (upstream's `NULL`) selects neutral. A
    /// change returns what to notify; setting the current local player again
    /// returns `None`. An index outside the array also selects neutral (a
    /// pointer outside `m_players` is undefined upstream).
    /// p1w13-sonnet, batch R13: the differential tests' local player, `None` included (`ThePlayerList->getLocalPlayer()` of a stand-in list may be
    /// `NULL`), with none of the notifications.
    pub fn friend_set_local_index(&mut self, player: Option<PlayerIndex>) {
        self.local = player;
    }

    pub fn set_local_player(&mut self, player: Option<PlayerIndex>) -> Option<LocalPlayerChange> {
        let idx = match player {
            Some(i) if (0..MAX_PLAYER_COUNT).contains(&i) => i,
            _ => 0,
        };
        if self.local == Some(idx) {
            return None;
        }
        let lost = self.local;
        self.local = Some(idx);
        Some(LocalPlayerChange { lost, gained: idx })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixteen_players_exist_from_the_start_and_indexing_is_bounded() {
        let a = PlayerArena::new();
        assert_eq!(a.iter().count(), 16);
        assert_eq!(a.get_nth(15).map(|p| p.player_index), Some(15));
        assert!(a.get_nth(16).is_none() && a.get_nth(-1).is_none());
        assert_eq!(a.player_count(), 0);
        assert!(a.local().is_none());
        assert_eq!(a.neutral().player_index, 0);
    }

    #[test]
    fn init_makes_neutral_local_and_null_selects_neutral() {
        let mut a = PlayerArena::new();
        assert_eq!(a.init(), Some(LocalPlayerChange { lost: None, gained: 0 }));
        assert_eq!(a.player_count(), 1);
        assert_eq!(a.set_local_player(Some(0)), None, "no change, no notification");
        assert_eq!(a.set_local_player(Some(3)), Some(LocalPlayerChange { lost: Some(0), gained: 3 }));
        assert_eq!(a.local().map(|p| p.player_index), Some(3));
        assert_eq!(a.set_local_player(None), Some(LocalPlayerChange { lost: Some(3), gained: 0 }));
        assert_eq!(a.set_local_player(Some(99)), None, "outside the array: neutral, already local");
    }

    #[test]
    fn name_key_lookup_stops_at_the_player_count() {
        let mut a = PlayerArena::new();
        a.get_nth_mut(2).unwrap().name_key = 77;
        a.set_player_count(2);
        assert!(a.find_with_name_key(77).is_none(), "player 2 is beyond m_playerCount");
        a.set_player_count(3);
        assert_eq!(a.find_with_name_key(77).map(|p| p.player_index), Some(2));
        a.set_player_count(99);
        assert_eq!(a.player_count(), 16);
    }
}
