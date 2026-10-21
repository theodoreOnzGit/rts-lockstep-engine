// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Player.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp
//   (slice 1: the constructor's `init`, the relationships, the build list, the team lists and the loops over a player's
//   objects, the AI forwarders, the radar, the special power timers, the production changes, the sciences and ranks)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The operations of `Player` (slice 1), as free functions over a [`PlayerHost`], the way `team_ops.rs` does `Team`.
//! Wave 7, `p1w10-sonnet`. The data is [`Player`](super::player::Player); the arena is
//! [`PlayerArena`](super::player::PlayerArena).
//!
//! # The shape
//!
//! A function takes the host and the player's index, reads and writes the player through the host's arena, and
//! reaches everything outside the player through the host's methods: the team arena (a player's teams are the
//! prototypes it owns; the host is a [`TeamHost`], so the team operations are called directly), the objects of its
//! teams, the AI, the score keeper and the academy statistics (other units, verified by their own tests), the client
//! (events), and the stores (ranks, sciences, player templates). `Money`, `Handicap`, `Energy` and `MissionStats` are
//! held by the player and called directly.
//!
//! # What is kept from upstream
//!
//! - **`init(pt)` copies the template's `Money` whole, player index included**, then sets the index; but when the
//!   amount is zero it assigns the default starting cash (`TheGameInfo`'s if there is one, else the global data's)
//!   whole, so the player's money then carries *that* money's player index, not the player's own. Kept: the sound
//!   a later deposit or withdrawal plays is for that index.
//! - **`addScience` calls `markUIDirty` once per prototype**, inside the loop over the prototypes, even for a prototype
//!   with no teams.
//! - **`addSkillPoints` loops `while (m_skillPoints >= m_levelUp)`**, relying on `setRankLevel` to move `m_levelUp`;
//!   a rank table that does not increase (or a rank limit below the player's rank) can make it spin. The Rust
//!   stops after [`SPIN_LIMIT`] rounds with [`PlayerError::Hang`].
//! - **`m_mpStartIndex` is read before it is ever written** unless `initFromDict` ran; 0 here.
//! - The `_DEBUG` / `_INTERNAL` / `_ALLOW_DEBUG_CHEATS_IN_RELEASE` blocks do not exist, as in the shipped game.
//!
//! # Where upstream is undefined
//!
//! An index that names no player, a team or prototype that is not there, a rank level outside the table that
//! `addSkillPoints` dereferences: [`PlayerError`], and the differential never gives them to the C++.

use crate::common::base_type::{real_to_int_ceil, Coord3D};
use crate::common::game_common::{GameDifficulty, PlayerType, Relationship, VeterancyLevel, LOGICFRAMES_PER_SECOND};
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{KindOfMaskType, KINDOF_POWERED, KINDOF_STRUCTURE};
use crate::common::money::Money;
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::name_key_generator::NameKeyType;
use crate::common::player_template::PlayerTemplate;
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::unicode_string::UnicodeString;
use crate::logic::client_event::ClientEvent;
use crate::logic::sides_list::BuildListInfo;

use super::ids::{PlayerIndex, TeamId, TeamPrototypeId};
use super::player::*;
use super::team::{TeamError, WaypointId};
use super::team_ops::*;

/// The number of rounds after which `addSkillPoints`' loop is reported as endless.
pub const SPIN_LIMIT: u32 = 10_000;

/// What the player code throws or cannot continue after (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerError {
    /// An index that names no player (upstream dereferences `NULL`).
    NoPlayer,
    /// A rank level the table does not have, where upstream dereferences the answer.
    NoRank,
    /// A loop upstream would never leave.
    Hang,
    /// What the team code reported.
    Team(TeamError),
    /// `throw SC_INVALID_DATA` (`Player::xfer`, `Squad::xfer`: a load that does not fit the world).
    InvalidData,
    /// What the xfer itself returned.
    Xfer(crate::common::xfer::XferStatus),
    /// An id or key names nothing where upstream dereferences a pointer (undefined behaviour there).
    Null,
    /// The name-key counter ran past `0x7fffffff` (undefined upstream).
    NameKeysExhausted,
    /// A float converted to an `Int` out of its range (undefined in C++).
    OutOfRange,
}

impl From<crate::common::xfer::XferStatus> for PlayerError {
    fn from(s: crate::common::xfer::XferStatus) -> PlayerError {
        PlayerError::Xfer(s)
    }
}

impl From<TeamError> for PlayerError {
    fn from(e: TeamError) -> PlayerError {
        PlayerError::Team(e)
    }
}

/// Results of the player operations.
pub type PResult<T> = Result<T, PlayerError>;

/// `NEUTRAL_PLAYER_COLOR`.
pub const NEUTRAL_PLAYER_COLOR: u32 = 0xffff_ffff;

/// `ScienceAvailabilityType` (`Player.h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ScienceAvailabilityType {
    /// `SCIENCE_AVAILABILITY_INVALID`.
    Invalid = -1,
    /// `SCIENCE_AVAILABLE`.
    Available = 0,
    /// `SCIENCE_DISABLED`.
    Disabled = 1,
    /// `SCIENCE_HIDDEN`.
    Hidden = 2,
}

/// `ScienceAvailabilityNames`.
pub const SCIENCE_AVAILABILITY_NAMES: [&str; 3] = ["Available", "Disabled", "Hidden"];

/// The global data and game state `Player` reads, as one value (`TheGlobalData`, `TheGameInfo`, `TheGameLogic`,
/// `TheInGameUI`, `TheAI`, `TheScriptEngine`).
#[derive(Clone, Copy, Debug)]
pub struct PlayerGlobals {
    /// `TheGlobalData->m_clientRetaliationModeEnabled`.
    pub client_retaliation_mode_enabled: bool,
    /// `TheGlobalData->m_baseValuePerSupplyBox`.
    pub base_value_per_supply_box: u32,
    /// `TheGlobalData->m_soloPlayerHealthBonusForDifficulty[PLAYERTYPE_COUNT][DIFFICULTY_COUNT]`.
    pub solo_player_health_bonus: [[f32; 3]; 2],
    /// `TheGlobalData->m_defaultStartingCash`.
    pub default_starting_cash: Money,
    /// `TheGlobalData->m_timeOfDay == TIME_OF_DAY_NIGHT`.
    pub time_of_day_is_night: bool,
    /// `TheGameInfo` (`NULL` = `None`): `getStartingCash()`.
    pub game_info_starting_cash: Option<Money>,
    /// `TheAI->getAiData()->m_forceSkirmishAI`.
    pub force_skirmish_ai: bool,
    /// `TheGameLogic->isInSinglePlayerGame()`.
    pub in_single_player_game: bool,
    /// `TheGameLogic->isInShellGame()`.
    pub in_shell_game: bool,
    /// `TheGameLogic->isInGameLogicUpdate()`.
    pub in_game_logic_update: bool,
    /// `TheGameLogic->getRankLevelLimit()`.
    pub rank_level_limit: i32,
    /// `TheInGameUI->isClientQuiet()`.
    pub client_quiet: bool,
    /// `TheScriptEngine->getGlobalDifficulty()`.
    pub global_difficulty: GameDifficulty,
}

/// What `Player` reads of a `PlayerTemplate` (`PlayerTemplate.h`): the layer-12 adapter fills it with
/// [`PlayerTemplateView::from_template`].
#[derive(Clone, Debug)]
pub struct PlayerTemplateView {
    /// `getSide()`.
    pub side: Vec<u8>,
    /// `getBaseSide()`.
    pub base_side: Vec<u8>,
    /// `getPreferredColor()->getAsInt()`.
    pub preferred_color_as_int: i32,
    /// `*getMoney()`.
    pub money: Money,
    /// `*getHandicap()`.
    pub handicap: crate::common::handicap::Handicap,
    /// `getProductionCostChanges()`.
    pub production_cost_changes: crate::common::player_template::ProductionChangeMap,
    /// `getProductionTimeChanges()`.
    pub production_time_changes: crate::common::player_template::ProductionChangeMap,
    /// `getProductionVeterancyLevels()`.
    pub production_veterancy_levels: crate::common::player_template::ProductionVeterancyMap,
    /// `isObserver()`.
    pub observer: bool,
    /// `isPlayableSide()`.
    pub playable_side: bool,
    /// `getIntrinsicSciences()`.
    pub intrinsic_sciences: Vec<ScienceType>,
    /// `getIntrinsicSciencePurchasePoints()`.
    pub intrinsic_science_purchase_points: i32,
    /// `getBeaconTemplate()`.
    pub beacon_template: Vec<u8>,
}

impl PlayerTemplateView {
    /// The values of a `PlayerTemplate`.
    pub fn from_template(t: &PlayerTemplate) -> PlayerTemplateView {
        PlayerTemplateView {
            side: t.side().to_vec(),
            base_side: t.base_side().to_vec(),
            preferred_color_as_int: t.preferred_color().get_as_int(),
            money: *t.money(),
            handicap: t.handicap().clone(),
            production_cost_changes: t.production_cost_changes().clone(),
            production_time_changes: t.production_time_changes().clone(),
            production_veterancy_levels: t.production_veterancy_levels().clone(),
            observer: t.is_observer(),
            playable_side: t.is_playable_side(),
            intrinsic_sciences: t.intrinsic_sciences().clone(),
            intrinsic_science_purchase_points: t.intrinsic_science_purchase_points(),
            beacon_template: t.beacon_template().to_vec(),
        }
    }
}

/// `RankInfo` (`RankInfo.h`).
#[derive(Clone, Debug)]
pub struct RankInfoView {
    /// `m_skillPointsNeeded`.
    pub skill_points_needed: i32,
    /// `m_sciencePurchasePointsGranted`.
    pub science_purchase_points_granted: i32,
    /// `m_sciencesGranted`.
    pub sciences_granted: Vec<ScienceType>,
}

/// A `SpecialPowerTemplate*` as `Player` reads it.
#[derive(Clone, Copy, Debug)]
pub struct SpecialPowerView {
    /// `getID()`.
    pub id: u32,
    /// `getReloadTime()`.
    pub reload_time: u32,
    /// `isSharedNSync()`.
    pub shared_n_sync: bool,
}

/// A `SpecialPowerModuleInterface*` of an object: an opaque handle the host gives out.
pub type SpecialPowerModuleRef = u32;
/// A `SpecialPowerTemplate*`.
pub type SpecialPowerTemplateRef = u32;

/// `EvaMessage`s `Player` plays (`TheEva->setShouldPlay`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaMessage {
    /// `EVA_GeneralLevelUp`.
    GeneralLevelUp,
    /// `EVA_SuperweaponDetected_Own_ParticleCannon`.
    SuperweaponDetectedOwnParticleCannon,
    /// `EVA_SuperweaponDetected_Ally_ParticleCannon`.
    SuperweaponDetectedAllyParticleCannon,
    /// `EVA_SuperweaponDetected_Enemy_ParticleCannon`.
    SuperweaponDetectedEnemyParticleCannon,
    /// `EVA_SuperweaponDetected_Own_Nuke`.
    SuperweaponDetectedOwnNuke,
    /// `EVA_SuperweaponDetected_Ally_Nuke`.
    SuperweaponDetectedAllyNuke,
    /// `EVA_SuperweaponDetected_Enemy_Nuke`.
    SuperweaponDetectedEnemyNuke,
    /// `EVA_SuperweaponDetected_Own_ScudStorm`.
    SuperweaponDetectedOwnScudStorm,
    /// `EVA_SuperweaponDetected_Ally_ScudStorm`.
    SuperweaponDetectedAllyScudStorm,
    /// `EVA_SuperweaponDetected_Enemy_ScudStorm`.
    SuperweaponDetectedEnemyScudStorm,
}

/// What `Player` tells the client (`TheAudio`, `TheControlBar`, `TheEva`): the one client interface
/// (`ClientEvent::Player`).
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerClientEvent {
    /// `TheAudio->addAudioEvent` of `MiscAudio`'s `m_radarOnlineSound` (true) or `m_radarOfflineSound` (false), with
    /// `setPlayerIndex(player_index)`.
    RadarSound {
        /// Online (true) or offline (false).
        online: bool,
        /// `setPlayerIndex`.
        player_index: i32,
    },
    /// `TheControlBar->markUIDirty()`.
    ControlBarDirty,
    /// `TheControlBar->onPlayerSciencePurchasePointsChanged(player)`.
    SciencePurchasePointsChanged {
        /// The player.
        player_index: i32,
    },
    /// `TheControlBar->onPlayerRankChanged(player)`.
    RankChanged {
        /// The player.
        player_index: i32,
    },
    /// `TheEva->setShouldPlay(message)`.
    Eva(EvaMessage),
    /// `TheControlBar->setControlBarSchemeByPlayer(player)`.
    ControlBarSchemeByPlayer {
        /// The player.
        player_index: i32,
    },
    /// `TheControlBar->setControlBarSchemeByPlayerTemplate(template)`.
    ControlBarSchemeByPlayerTemplate {
        /// The template found (`None` for `NULL`).
        template: Option<PlayerTemplateRef>,
    },
    /// `TheInGameUI->...`/`Color` of the local player's team: `ThePlayerList->getLocalPlayer()->getPlayerColor()` as
    /// `TheGameClient->setTeamColor(r, g, b)`.
    SetTeamColor {
        /// Red, 0 to 255.
        red: i32,
        /// Green.
        green: i32,
        /// Blue.
        blue: i32,
    },
    /// `TheRadar->removeObject(object)`.
    RadarRemoveObject {
        /// The object.
        object: ObjectId,
    },
    /// `TheRadar->addObject(object)`.
    RadarAddObject {
        /// The object.
        object: ObjectId,
    },
    /// `drawable->changeIndicatorColor`/`object->getDrawable()->setIndicatorColor(color)`.
    IndicatorColor {
        /// The object.
        object: ObjectId,
        /// The colour.
        color: u32,
    },
    /// `TheInGameUI->addFloatingText(text, &pos, color)` of a bounty.
    FloatingText {
        /// The amount shown.
        amount: i32,
        /// Where.
        pos: Coord3D,
        /// The colour.
        color: u32,
    },
}

/// Everything the player code reaches outside the player arena and the team arena. See the module docs.
pub trait PlayerHost: TeamHost {
    /// The player arena (`ThePlayerList`).
    fn players(&self) -> &PlayerArena;
    /// The player arena, mutable.
    fn players_mut(&mut self) -> &mut PlayerArena;

    /// The global data and game state (see [`PlayerGlobals`]).
    fn globals(&self) -> PlayerGlobals;
    /// `*pt` for a template handle, as the values `Player` reads of it.
    fn player_template(&self, t: PlayerTemplateRef) -> PlayerTemplateView;
    /// `TheClientNotifier`: one event for the client.
    fn client_event(&mut self, e: ClientEvent);

    /// `TheRankInfoStore->getRankLevelCount()`.
    fn rank_level_count(&self) -> i32;
    /// `TheRankInfoStore->getRankInfo(level)` (`NULL` = `None`).
    fn rank_info(&self, level: i32) -> Option<RankInfoView>;
    /// `TheScienceStore->isScienceGrantable(science)`.
    fn science_is_grantable(&self, science: ScienceType) -> bool;
    /// `TheScienceStore->getSciencePurchaseCost(science)`.
    fn science_purchase_cost(&self, science: ScienceType) -> i32;
    /// `TheScienceStore->playerHasPrereqsForScience(player, science)`, which asks the player `hasScience` of each
    /// prerequisite.
    fn science_has_prereqs(&self, p: PlayerIndex, science: ScienceType) -> bool;
    /// `TheGameText->fetch("SCIENCE:GeneralName")`.
    fn game_text_general_name(&mut self) -> UnicodeString;
    /// `TheScriptEngine->notifyOfObjectCreationOrDestruction()`.
    fn script_notify_of_object_creation_or_destruction(&mut self);
    /// `TheScriptEngine->notifyOfAcquiredScience(playerIndex, science)`.
    fn script_notify_of_acquired_science(&mut self, player_index: PlayerIndex, science: ScienceType);
    /// `TheVictoryConditions->hasSinglePlayerBeenDefeated(player)`.
    fn victory_has_single_player_been_defeated(&self, p: PlayerIndex) -> bool;
    /// `TheMessageStream->appendMessage(MSG_ENABLE_RETALIATION_MODE)` then `appendIntegerArgument(index)` and
    /// `appendBooleanArgument(enabled)`.
    fn message_enable_retaliation_mode(&mut self, player_index: PlayerIndex, enabled: bool);

    /// The player's academy statistics: `init(player)`.
    fn academy_init(&mut self, p: PlayerIndex);
    /// `getAcademyStats()->update()`.
    fn academy_update(&mut self, p: PlayerIndex);
    /// `getAcademyStats()->recordIncome()` (what `Money::deposit` asks of the player at its own index).
    fn academy_record_income(&mut self, p: PlayerIndex);
    /// `getAcademyStats()->recordGeneralsPointsSpent(points)`.
    fn academy_record_generals_points_spent(&mut self, p: PlayerIndex, points: i32);
    /// The score keeper: `reset(playerIdx)`.
    fn score_reset(&mut self, p: PlayerIndex, player_idx: PlayerIndex);
    /// `m_scoreKeeper.addObjectBuilt(o)`.
    fn score_add_object_built(&mut self, p: PlayerIndex, o: Option<ObjectId>);
    /// `m_scoreKeeper.removeObjectBuilt(o)`.
    fn score_remove_object_built(&mut self, p: PlayerIndex, o: Option<ObjectId>);

    /// `newInstance(AIPlayer)(this)` (or `AISkirmishPlayer`).
    fn ai_create(&mut self, p: PlayerIndex, skirmish: bool);
    /// `m_ai->deleteInstance()`.
    fn ai_delete(&mut self, p: PlayerIndex);
    /// `m_ai->update()`.
    fn ai_update(&mut self, p: PlayerIndex);
    /// `m_ai->newMap()`.
    fn ai_new_map(&mut self, p: PlayerIndex);
    /// `m_ai->getAIDifficulty()`.
    fn ai_difficulty(&self, p: PlayerIndex) -> GameDifficulty;
    /// `m_ai->isSkirmishAI()`.
    fn ai_is_skirmish(&self, p: PlayerIndex) -> bool;
    /// `m_ai->computeSuperweaponTarget(power, pos, playerNdx, weaponRadius)`; the AI may write `pos`.
    fn ai_compute_superweapon_target(&mut self, p: PlayerIndex, power: SpecialPowerTemplateRef, pos: &mut Coord3D, player_ndx: i32, weapon_radius: f32) -> bool;
    /// `m_ai->getAiEnemy()`.
    fn ai_enemy(&self, p: PlayerIndex) -> Option<PlayerIndex>;
    /// `m_ai->checkBridges(unit, way)`.
    fn ai_check_bridges(&mut self, p: PlayerIndex, unit: Option<ObjectId>, way: Option<WaypointId>) -> bool;
    /// `m_ai->getBaseCenter(pos)`; the AI may write `pos`.
    fn ai_base_center(&mut self, p: PlayerIndex, pos: &mut Coord3D) -> bool;
    /// `m_ai->repairStructure(id)`.
    fn ai_repair_structure(&mut self, p: PlayerIndex, id: ObjectId);
    /// `m_ai->onUnitProduced(factory, unit)`.
    fn ai_on_unit_produced(&mut self, p: PlayerIndex, factory: Option<ObjectId>, unit: Option<ObjectId>);
    /// `m_ai->onStructureProduced(builder, structure)`.
    fn ai_on_structure_produced(&mut self, p: PlayerIndex, builder: Option<ObjectId>, structure: Option<ObjectId>);
    /// `m_ai->isSupplySourceSafe(minSupplies)`.
    fn ai_is_supply_source_safe(&mut self, p: PlayerIndex, min_supplies: i32) -> bool;
    /// `m_ai->isSupplySourceAttacked()`.
    fn ai_is_supply_source_attacked(&mut self, p: PlayerIndex) -> bool;
    /// `m_ai->setTeamDelaySeconds(delay)`.
    fn ai_set_team_delay_seconds(&mut self, p: PlayerIndex, delay: i32);
    /// `m_ai->guardSupplyCenter(team, minSupplies)`.
    fn ai_guard_supply_center(&mut self, p: PlayerIndex, team: Option<TeamId>, min_supplies: i32);
    /// `m_ai->aiPreTeamDestroy(team)`.
    fn ai_pre_team_destroy(&mut self, p: PlayerIndex, team: Option<TeamId>);
    /// `m_ai->selectSkillset(skillSet)`.
    fn ai_select_skillset(&mut self, p: PlayerIndex, skill_set: i32);
    /// `m_ai->buildSpecificAITeam(proto, priority)`.
    fn ai_build_specific_team(&mut self, p: PlayerIndex, proto: Option<TeamPrototypeId>, priority: bool);
    /// `m_ai->buildAIBaseDefense(flank)`.
    fn ai_build_base_defense(&mut self, p: PlayerIndex, flank: bool);
    /// `m_ai->buildAIBaseDefenseStructure(name, flank)`.
    fn ai_build_base_defense_structure(&mut self, p: PlayerIndex, name: &[u8], flank: bool);
    /// `m_ai->buildSpecificAIBuilding(name)`.
    fn ai_build_specific_building(&mut self, p: PlayerIndex, name: &[u8]);
    /// `m_ai->buildBySupplies(minimumCash, name)`.
    fn ai_build_by_supplies(&mut self, p: PlayerIndex, minimum_cash: i32, name: &[u8]);
    /// `m_ai->buildSpecificBuildingNearestTeam(name, team)`.
    fn ai_build_specific_building_nearest_team(&mut self, p: PlayerIndex, name: &[u8], team: Option<TeamId>);
    /// `m_ai->buildUpgrade(name)`.
    fn ai_build_upgrade(&mut self, p: PlayerIndex, name: &[u8]);
    /// `m_ai->recruitSpecificAITeam(proto, radius)`.
    fn ai_recruit_specific_team(&mut self, p: PlayerIndex, proto: Option<TeamPrototypeId>, radius: f32);
    /// `m_ai->calcClosestConstructionZoneLocation(template, location)`; the AI may write `location`.
    fn ai_calc_closest_construction_zone_location(&mut self, p: PlayerIndex, template: TemplateRef, location: &mut Coord3D) -> bool;

    /// `obj->getVeterancyLevel()`.
    fn obj_veterancy_level(&self, o: ObjectId) -> i32;
    /// `t->getSkillPointValue(level)`.
    fn tmpl_skill_point_value(&self, t: TemplateRef, level: i32) -> i32;
    /// `obj->getOrientation()`.
    fn obj_orientation(&self, o: ObjectId) -> f32;
    /// `obj->setScriptStatus(bit, set)`.
    fn obj_set_script_status_disabled(&mut self, o: ObjectId, set: bool);
    /// `obj->setDisabled(DISABLED_UNDERPOWERED)`.
    fn obj_set_disabled_underpowered(&mut self, o: ObjectId);
    /// `obj->clearDisabled(DISABLED_UNDERPOWERED)`.
    fn obj_clear_disabled_underpowered(&mut self, o: ObjectId);
    /// The object's behaviour modules that have a special power interface, in module order (`getBehaviorModules()`
    /// then `getSpecialPower()`, the others skipped).
    fn obj_special_power_modules(&self, o: ObjectId) -> Vec<SpecialPowerModuleRef>;
    /// `sp->getRequiredScience()`.
    fn sp_required_science(&self, m: SpecialPowerModuleRef) -> ScienceType;
    /// `sp->onSpecialPowerCreation()`.
    fn sp_on_special_power_creation(&mut self, m: SpecialPowerModuleRef);
    /// `sp->setReadyFrame(frame)`.
    fn sp_set_ready_frame(&mut self, m: SpecialPowerModuleRef, frame: u32);
    /// A `SpecialPowerTemplate`'s `getID()`, `getReloadTime()` and `isSharedNSync()`.
    fn special_power_template(&self, t: SpecialPowerTemplateRef) -> SpecialPowerView;
    /// `ThePartitionManager->getDistanceSquared(a, b, FROM_CENTER_2D)`.
    fn partition_distance_squared_2d(&self, a: ObjectId, b: ObjectId) -> f32;
}

// ---- small helpers ------------------------------------------------------------------------------------------------

fn pl<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<&Player> {
    h.players().get_nth(p).ok_or(PlayerError::NoPlayer)
}

fn plm<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<&mut Player> {
    h.players_mut().get_nth_mut(p).ok_or(PlayerError::NoPlayer)
}

/// The prototypes a player owns, as `m_playerTeamPrototypes` holds them now.
fn protos<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<Vec<TeamPrototypeId>> {
    let list = pl(h, p)?.team_prototypes.clone();
    // a prototype deleted while another player still lists it (a prototype whose owner changed) is a dangling pointer upstream
    if list.iter().any(|id| h.teams().prototype(*id).is_none()) {
        return Err(PlayerError::Team(crate::logic::world::team::TeamError::Null));
    }
    Ok(list)
}

/// `for (DLINK_ITERATOR<Team> iter = (*it)->iterate_TeamInstanceList(); ...) for (DLINK_ITERATOR<Object> iterObj =
/// team->iterate_TeamMemberList(); ...)` over every prototype of the player, calling `body` for each member. The list
/// links are read after the body ran, as the iterator does, so a body that moves the current object to another team
/// continues in that team's list.
fn for_each_member<H: PlayerHost>(h: &mut H, p: PlayerIndex, mut body: impl FnMut(&mut H, ObjectId)) -> PResult<()> {
    for proto in protos(h, p)? {
        let mut cur_t = h.teams().first_instance(proto);
        while let Some(t) = cur_t {
            let mut cur_o = h.teams().first_member(t);
            while let Some(o) = cur_o {
                body(h, o);
                cur_o = h.teams().next_member(o);
            }
            cur_t = h.teams().next_instance(t);
        }
    }
    Ok(())
}

/// `NAMEKEY(name)`.
fn key_of<H: PlayerHost>(h: &mut H, name: &[u8]) -> NameKeyType {
    h.name_to_key(name)
}

// ---- construction -------------------------------------------------------------------------------------------------

/// `Player::init(pt)`: back to the state of a new player of this template (`None` = the neutral player).
pub fn player_init<H: PlayerHost>(h: &mut H, p: PlayerIndex, pt: Option<PlayerTemplateRef>) -> PResult<()> {
    let g = h.globals();
    {
        let me = plm(h, p)?;
        me.skill_points_modifier = 1.0;
        me.attacked_frame = 0;
        me.is_preorder = false;
        me.is_player_dead = false;
        me.radar_count = 0;
        me.disable_proof_radar_count = 0;
        me.radar_disabled = false;
        me.bombard_battle_plans = 0;
        me.hold_the_line_battle_plans = 0;
        me.search_and_destroy_battle_plans = 0;
        me.battle_plan_bonuses = None;
    }
    player_delete_upgrade_list(h, p)?;
    {
        let me = plm(h, p)?;
        me.energy.init(Some(p));
        me.mission_stats.init();
        me.build_list.clear();
        me.default_team = None;
    }
    if pl(h, p)?.ai.is_some() {
        h.ai_delete(p);
    }
    {
        let me = plm(h, p)?;
        me.ai = None;
        me.resource_gathering_manager = None;
        me.squads = vec![Squad::default(); NUM_HOTKEY_SQUADS - plant(9) as usize];
        me.current_selection = Some(Squad::default());
        me.tunnel_system = None;
        me.can_build_base = true;
        me.can_build_units = true;
        me.observer = false;
        me.cash_bounty_percent = 0.0;
        me.list_in_score_screen = !plant(8);
        me.units_should_hunt = false;
    }
    if let Some(t) = pt {
        let (side, base_side, cost, time, vet, color, money, handicap, observer) = {
            let tp = h.player_template(t);
            (
                tp.side,
                tp.base_side,
                tp.production_cost_changes,
                tp.production_time_changes,
                tp.production_veterancy_levels,
                if plant(1) { tp.preferred_color_as_int as u32 } else { (tp.preferred_color_as_int as u32) | 0xff00_0000 },
                tp.money,
                tp.handicap,
                tp.observer,
            )
        };
        let me = plm(h, p)?;
        me.side = side;
        me.base_side = base_side;
        me.production_cost_changes = cost;
        me.production_time_changes = time;
        me.production_veterancy_levels = vet;
        me.color = color;
        me.night_color = if plant(2) { 0 } else { color };
        me.money = money;
        if !plant(3) {
            me.money.set_player_index(p);
        }
        me.handicap = handicap;
        if me.money.count_money() == 0 {
            me.money = match g.game_info_starting_cash {
                Some(m) if !plant(4) => m,
                _ => g.default_starting_cash,
            };
        }
        me.display_name = UnicodeString::default();
        me.player_name.clear();
        me.name_key = 0;
        me.player_type = PlayerType::Computer;
        me.observer = observer;
        me.is_player_dead = observer && !plant(5);
    } else {
        let empty_key = key_of(h, b"");
        {
            let me = plm(h, p)?;
            me.side.clear();
            me.base_side.clear();
            me.production_cost_changes.clear();
            me.production_time_changes.clear();
            me.production_veterancy_levels.clear();
            me.color = NEUTRAL_PLAYER_COLOR;
            me.night_color = NEUTRAL_PLAYER_COLOR;
            me.money.init();
            me.handicap.init();
            me.display_name = UnicodeString::default();
            me.player_name.clear();
            me.name_key = empty_key.0 as i32;
            me.player_type = PlayerType::Computer;
        }
        // neutral is always "allied" with self -- this is the only thing ever allied with neutral!
        if !plant(6) {
            player_set_player_relationship(h, p, Some(p), Relationship::Allies)?;
        }
    }
    // reset each player
    if !plant(7) {
        h.score_reset(p, p);
    }
    plm(h, p)?.template = pt;

    // reset rank info
    player_reset_rank(h, p)?;
    {
        let me = plm(h, p)?;
        me.sciences_disabled.clear();
        me.sciences_hidden.clear();
        me.special_power_timers.clear();
        me.kind_of_production_changes.clear();
    }
    if !plant(10) {
        h.academy_init(p);
    }

    // Always off at the beginning of a game! Only GameLogic::update has the power to turn it on.
    plm(h, p)?.logical_retaliation_mode_enabled = false;
    Ok(())
}

/// `Player::deleteUpgradeList()`.
pub fn player_delete_upgrade_list<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    let me = plm(h, p)?;
    me.upgrades.clear();
    me.upgrades_in_progress = Default::default();
    me.upgrades_completed = Default::default();
    Ok(())
}

// ---- relationships ----------------------------------------------------------------------------------------------

/// `Player::getRelationship(that)`.
pub fn player_get_relationship<H: PlayerHost>(h: &H, p: PlayerIndex, that: Option<TeamId>) -> PResult<Relationship> {
    if let Some(t) = that {
        let me = pl(h, p)?;
        // do we have an override for that particular team? if so, return it.
        if !me.team_relations.map.is_empty() && !plant(11) {
            if let Some(r) = me.team_relations.map.get(&t) {
                return Ok(*r);
            }
        }
        // hummm... well, do we have something for that team's player?
        if !me.player_relations.map.is_empty() {
            let that_player = team_get_controlling_player(h, t)?;
            if let Some(tp) = that_player {
                if let Some(r) = me.player_relations.map.get(&tp) {
                    return Ok(*r);
                }
            }
        }
    }
    Ok(if plant(12) { Relationship::Allies } else { Relationship::Neutral })
}

/// `Player::setPlayerRelationship(that, r)`.
pub fn player_set_player_relationship<H: PlayerHost>(h: &mut H, p: PlayerIndex, that: Option<PlayerIndex>, r: Relationship) -> PResult<()> {
    if let Some(t) = that {
        pl(h, t)?;
        plm(h, p)?.player_relations.map.insert(t, r);
    }
    Ok(())
}

/// `Player::removePlayerRelationship(that)`.
pub fn player_remove_player_relationship<H: PlayerHost>(h: &mut H, p: PlayerIndex, that: Option<PlayerIndex>) -> PResult<bool> {
    if let Some(t) = that {
        pl(h, t)?;
    }
    let me = plm(h, p)?;
    if !me.player_relations.map.is_empty() {
        match that {
            None => {
                me.player_relations.map.clear();
                return Ok(!plant(13));
            }
            Some(t) => {
                if me.player_relations.map.remove(&t).is_some() {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// `Player::setTeamRelationship(that, r)`.
pub fn player_set_team_relationship<H: PlayerHost>(h: &mut H, p: PlayerIndex, that: Option<TeamId>, r: Relationship) -> PResult<()> {
    if let Some(t) = that {
        plm(h, p)?.team_relations.map.insert(t, r);
    }
    Ok(())
}

/// `Player::removeTeamRelationship(that)`.
pub fn player_remove_team_relationship<H: PlayerHost>(h: &mut H, p: PlayerIndex, that: Option<TeamId>) -> PResult<bool> {
    let me = plm(h, p)?;
    if !me.team_relations.map.is_empty() {
        match that {
            None => {
                me.team_relations.map.clear();
                return Ok(true);
            }
            Some(t) => {
                if me.team_relations.map.remove(&t).is_some() || plant(14) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

// ---- the build list -----------------------------------------------------------------------------------------------

/// `Player::setBuildList(pBuildList)`: the old list is deleted, the new one (the whole chain) is taken.
pub fn player_set_build_list<H: PlayerHost>(h: &mut H, p: PlayerIndex, list: Vec<BuildListInfo>) -> PResult<()> {
    plm(h, p)?.build_list = list;
    Ok(())
}

/// `Player::addToBuildList(obj)`: a new entry for the object, at the head.
pub fn player_add_to_build_list<H: PlayerHost>(h: &mut H, p: PlayerIndex, obj: ObjectId) -> PResult<()> {
    let template_name = h.obj_template(obj).map(|t| h.tmpl_name(t)).unwrap_or_default();
    let location = h.obj_position(obj);
    let angle = h.obj_orientation(obj);
    let mut info = BuildListInfo::new();
    info.object_id = obj;
    info.template_name = template_name;
    info.location = location;
    info.angle = angle;
    info.num_rebuilds = plant(16) as u32; // Can't rebuild.
    {
        let bl = &mut plm(h, p)?.build_list;
        let at = if plant(15) { bl.len() } else { 0 };
        bl.insert(at, info);
    }
    Ok(())
}

/// `Player::addToPriorityBuildList(templateName, pos, angle)`.
pub fn player_add_to_priority_build_list<H: PlayerHost>(h: &mut H, p: PlayerIndex, template_name: &[u8], pos: Coord3D, angle: f32) -> PResult<()> {
    let mut info = BuildListInfo::new();
    info.template_name = template_name.to_vec();
    info.location = pos;
    info.angle = angle;
    info.priority_build = !plant(17);
    info.num_rebuilds = 1; // Build once.
    plm(h, p)?.build_list.insert(0, info);
    Ok(())
}

// ---- update ------------------------------------------------------------------------------------------------------

/// `Player::update()`.
pub fn player_update<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    if pl(h, p)?.ai.is_some() && !plant(20) {
        h.ai_update(p);
    }
    // Allow the teams this player owns to update themselves.
    for proto in protos(h, p)? {
        let mut cur = h.teams().first_instance(proto);
        while let Some(t) = cur {
            team_update_generic_scripts(h, t)?;
            cur = h.teams().next_instance(t);
        }
    }
    let frame = h.frame();
    let sabotaged = pl(h, p)?.energy.power_sabotaged_till_frame();
    if sabotaged != 0 && (if plant(18) { frame >= sabotaged } else { frame > sabotaged }) {
        let me = plm(h, p)?;
        me.energy.set_power_sabotaged_till_frame(0); // Tells us we're no longer sabotaged for above check.
        let brown_out = !me.energy.has_sufficient_power(frame);
        player_on_power_brown_out_change(h, p, brown_out)?;
    }
    // Update the academy stats (this only checks applicable things that require a polling method)
    h.academy_update(p);

    // Kris: August 25, 2003 (DAY OF CODE LOCK -- NO NEW FEATURES, REALLY!)
    if h.players().local_index() == Some(p) {
        let now = h.frame();
        // Only check and post the message once every second so we don't spam the message stream to account for lag.
        if now.is_multiple_of(if plant(19) { 29 } else { LOGICFRAMES_PER_SECOND as u32 }) {
            let enabled = h.globals().client_retaliation_mode_enabled;
            if (enabled != pl(h, p)?.logical_retaliation_mode_enabled) != plant(70) {
                // Post a logical message that will switch the retaliation mode on or off.
                h.message_enable_retaliation_mode(p, enabled);
            }
        }
    }
    Ok(())
}

/// `Player::newMap()`.
pub fn player_new_map<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_new_map(p);
    }
    Ok(())
}

/// `Player::setPlayerType(t, skirmish)`.
pub fn player_set_player_type<H: PlayerHost>(h: &mut H, p: PlayerIndex, t: PlayerType, skirmish: bool) -> PResult<()> {
    plm(h, p)?.player_type = t;
    if pl(h, p)?.ai.is_some() && !plant(22) {
        h.ai_delete(p);
    }
    plm(h, p)?.ai = None;
    if t == PlayerType::Computer {
        let skirmish = skirmish || (h.globals().force_skirmish_ai && !plant(21));
        // create AIPlayer (or AISkirmishPlayer) and attach to this Player
        h.ai_create(p, skirmish);
        plm(h, p)?.ai = Some(AiPlayerRef { skirmish });
    }
    Ok(())
}

/// `Player::setDefaultTeam()`: this is called from `PlayerList->newGame()`.
pub fn player_set_default_team<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    let mut tname = b"team".to_vec();
    tname.extend_from_slice(&pl(h, p)?.player_name);
    let dt = factory_find_team(h, &tname)?;
    if let Some(t) = dt {
        plm(h, p)?.default_team = Some(t);
        if !plant(23) {
            team_set_active(h, t)?;
        }
    }
    Ok(())
}

// ---- the AI ------------------------------------------------------------------------------------------------------

/// `Player::isSkirmishAIPlayer()`.
pub fn player_is_skirmish_ai_player<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    Ok(if pl(h, p)?.ai.is_some() { h.ai_is_skirmish(p) } else { false })
}

/// `Player::computeSuperweaponTarget(power, retPos, playerNdx, weaponRadius)`.
pub fn player_compute_superweapon_target<H: PlayerHost>(
    h: &mut H,
    p: PlayerIndex,
    power: SpecialPowerTemplateRef,
    ret_pos: &mut Coord3D,
    player_ndx: i32,
    weapon_radius: f32,
) -> PResult<bool> {
    if pl(h, p)?.ai.is_some() {
        return Ok(h.ai_compute_superweapon_target(p, power, ret_pos, player_ndx, weapon_radius));
    }
    Ok(false)
}

/// `Player::getCurrentEnemy()`.
pub fn player_get_current_enemy<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<Option<PlayerIndex>> {
    Ok(if pl(h, p)?.ai.is_some() { h.ai_enemy(p) } else { None })
}

/// `Player::getPlayerDifficulty()`.
pub fn player_get_player_difficulty<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<GameDifficulty> {
    if pl(h, p)?.ai.is_some() && !plant(24) {
        return Ok(h.ai_difficulty(p));
    }
    Ok(h.globals().global_difficulty)
}

/// `Player::checkBridges(unit, way)`.
pub fn player_check_bridges<H: PlayerHost>(h: &mut H, p: PlayerIndex, unit: Option<ObjectId>, way: Option<WaypointId>) -> PResult<bool> {
    Ok(if pl(h, p)?.ai.is_some() { h.ai_check_bridges(p, unit, way) } else { false })
}

/// `Player::getAiBaseCenter(pos)`.
pub fn player_get_ai_base_center<H: PlayerHost>(h: &mut H, p: PlayerIndex, pos: &mut Coord3D) -> PResult<bool> {
    Ok(if pl(h, p)?.ai.is_some() { h.ai_base_center(p, pos) } else { false })
}

/// `Player::repairStructure(structureID)`.
pub fn player_repair_structure<H: PlayerHost>(h: &mut H, p: PlayerIndex, structure_id: ObjectId) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_repair_structure(p, structure_id);
    }
    Ok(())
}

/// `Player::onUnitCreated(factory, unit)`.
pub fn player_on_unit_created<H: PlayerHost>(h: &mut H, p: PlayerIndex, factory: Option<ObjectId>, unit: Option<ObjectId>) -> PResult<()> {
    h.script_notify_of_object_creation_or_destruction();
    if !plant(26) {
        h.score_add_object_built(p, unit);
    }
    if pl(h, p)?.ai.is_some() {
        h.ai_on_unit_produced(p, factory, unit);
    }
    Ok(())
}

/// `Player::isSupplySourceSafe(minSupplies)`.
pub fn player_is_supply_source_safe<H: PlayerHost>(h: &mut H, p: PlayerIndex, min_supplies: i32) -> PResult<bool> {
    Ok(if pl(h, p)?.ai.is_some() { h.ai_is_supply_source_safe(p, min_supplies) } else { !plant(25) })
}

/// `Player::isSupplySourceAttacked()`.
pub fn player_is_supply_source_attacked<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<bool> {
    Ok(if pl(h, p)?.ai.is_some() { h.ai_is_supply_source_attacked(p) } else { false })
}

/// `Player::setTeamDelaySeconds(delay)`.
pub fn player_set_team_delay_seconds<H: PlayerHost>(h: &mut H, p: PlayerIndex, delay: i32) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_set_team_delay_seconds(p, delay);
    }
    Ok(())
}

/// `Player::guardSupplyCenter(team, minSupplies)`.
pub fn player_guard_supply_center<H: PlayerHost>(h: &mut H, p: PlayerIndex, team: Option<TeamId>, min_supplies: i32) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_guard_supply_center(p, team, min_supplies);
    }
    Ok(())
}

/// `Player::preTeamDestroy(team)`.
pub fn player_pre_team_destroy<H: PlayerHost>(h: &mut H, p: PlayerIndex, team: Option<TeamId>) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_pre_team_destroy(p, team);
    }
    Ok(())
}

/// `Player::onStructureCreated(builder, structure)`: nothing.
pub fn player_on_structure_created<H: PlayerHost>(_h: &mut H, _p: PlayerIndex, _builder: Option<ObjectId>, _structure: Option<ObjectId>) {}

/// `Player::onStructureUndone(structure)`.
pub fn player_on_structure_undone<H: PlayerHost>(h: &mut H, p: PlayerIndex, structure: Option<ObjectId>) {
    if plant(27) {
        h.score_add_object_built(p, structure);
    } else {
        h.score_remove_object_built(p, structure);
    }
}

/// `Player::friend_setSkillset(skillSet)`.
pub fn player_friend_set_skillset<H: PlayerHost>(h: &mut H, p: PlayerIndex, skill_set: i32) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_select_skillset(p, skill_set);
    }
    Ok(())
}

/// `Player::buildSpecificTeam(teamProto)`.
pub fn player_build_specific_team<H: PlayerHost>(h: &mut H, p: PlayerIndex, proto: Option<TeamPrototypeId>) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_build_specific_team(p, proto, true);
    }
    Ok(())
}

/// `Player::buildBaseDefense(flank)`.
pub fn player_build_base_defense<H: PlayerHost>(h: &mut H, p: PlayerIndex, flank: bool) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_build_base_defense(p, flank);
    }
    Ok(())
}

/// `Player::buildBaseDefenseStructure(thingName, flank)`.
pub fn player_build_base_defense_structure<H: PlayerHost>(h: &mut H, p: PlayerIndex, thing_name: &[u8], flank: bool) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_build_base_defense_structure(p, thing_name, flank);
    }
    Ok(())
}

/// `Player::buildSpecificBuilding(thingName)`.
pub fn player_build_specific_building<H: PlayerHost>(h: &mut H, p: PlayerIndex, thing_name: &[u8]) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_build_specific_building(p, thing_name);
    }
    Ok(())
}

/// `Player::buildBySupplies(minimumCash, thingName)`.
pub fn player_build_by_supplies<H: PlayerHost>(h: &mut H, p: PlayerIndex, minimum_cash: i32, thing_name: &[u8]) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_build_by_supplies(p, minimum_cash, thing_name);
    }
    Ok(())
}

/// `Player::buildSpecificBuildingNearestTeam(thingName, team)`.
pub fn player_build_specific_building_nearest_team<H: PlayerHost>(h: &mut H, p: PlayerIndex, thing_name: &[u8], team: Option<TeamId>) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_build_specific_building_nearest_team(p, thing_name, team);
    }
    Ok(())
}

/// `Player::buildUpgrade(upgrade)`.
pub fn player_build_upgrade<H: PlayerHost>(h: &mut H, p: PlayerIndex, upgrade: &[u8]) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_build_upgrade(p, upgrade);
    }
    Ok(())
}

/// `Player::recruitSpecificTeam(teamProto, recruitRadius)`.
pub fn player_recruit_specific_team<H: PlayerHost>(h: &mut H, p: PlayerIndex, proto: Option<TeamPrototypeId>, recruit_radius: f32) -> PResult<()> {
    if pl(h, p)?.ai.is_some() {
        h.ai_recruit_specific_team(p, proto, recruit_radius);
    }
    Ok(())
}

/// `Player::calcClosestConstructionZoneLocation(constructTemplate, location)`.
pub fn player_calc_closest_construction_zone_location<H: PlayerHost>(h: &mut H, p: PlayerIndex, construct_template: TemplateRef, location: &mut Coord3D) -> PResult<bool> {
    if pl(h, p)?.ai.is_some() {
        return Ok(h.ai_calc_closest_construction_zone_location(p, construct_template, location));
    }
    Ok(false)
}

// ---- the team prototypes and the objects of a player ---------------------------------------------------------------

/// `Player::addTeamToList(team)`: nothing if it is already there.
pub fn player_add_team_to_list<H: PlayerHost>(h: &mut H, p: PlayerIndex, team: TeamPrototypeId) -> PResult<()> {
    let me = plm(h, p)?;
    if !me.team_prototypes.contains(&team) || plant(28) {
        me.team_prototypes.push(team);
    }
    Ok(())
}

/// `Player::removeTeamFromList(team)`: the first match only.
pub fn player_remove_team_from_list<H: PlayerHost>(h: &mut H, p: PlayerIndex, team: TeamPrototypeId) -> PResult<()> {
    let me = plm(h, p)?;
    if let Some(i) = me.team_prototypes.iter().position(|x| *x == team) {
        if plant(29) {
            me.team_prototypes.swap_remove(i);
        } else {
            me.team_prototypes.remove(i);
        }
    }
    Ok(())
}

/// `Player::healAllObjects()`.
pub fn player_heal_all_objects<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    for proto in protos(h, p)? {
        proto_heal_all_objects(h, proto);
    }
    Ok(())
}

/// `Player::iterateObjects(func, userData)`.
pub fn player_iterate_objects<H: PlayerHost>(h: &mut H, p: PlayerIndex, mut func: impl FnMut(&mut H, ObjectId)) -> PResult<()> {
    for proto in protos(h, p)? {
        proto_iterate_objects(h, proto, &mut func);
    }
    Ok(())
}

/// `Player::countObjectsByThingTemplate(numTmplates, things, ignoreDead, counts, ignoreUnderConstruction)`.
pub fn player_count_objects_by_thing_template<H: PlayerHost>(
    h: &H,
    p: PlayerIndex,
    things: &[Option<TemplateRef>],
    ignore_dead: bool,
    counts: &mut [i32],
    ignore_under_construction: bool,
) -> PResult<()> {
    for c in counts.iter_mut().take(things.len()) {
        if !plant(30) {
            *c = 0;
        }
    }
    for proto in protos(h, p)? {
        proto_count_objects_by_thing_template(h, proto, things, ignore_dead, counts, ignore_under_construction);
    }
    Ok(())
}

/// `Player::countBuildings()`.
pub fn player_count_buildings<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<i32> {
    let mut r = 0;
    for proto in protos(h, p)? {
        r += proto_count_buildings(h, proto) + plant(71) as i32;
    }
    Ok(r)
}

/// `Player::countObjects(setMask, clearMask)`.
pub fn player_count_objects<H: PlayerHost>(h: &H, p: PlayerIndex, set_mask: &KindOfMaskType, clear_mask: &KindOfMaskType) -> PResult<i32> {
    let mut r = 0;
    for proto in protos(h, p)? {
        r += proto_count_objects(h, proto, set_mask, clear_mask);
    }
    Ok(r)
}

/// `Player::findClosestByKindOf(queryObject, setMask, clearMask)`: `ClosestKindOfData` starts at `FLT_MAX`.
pub fn player_find_closest_by_kind_of<H: PlayerHost>(
    h: &mut H,
    p: PlayerIndex,
    query_object: Option<ObjectId>,
    set_mask: &KindOfMaskType,
    clear_mask: &KindOfMaskType,
) -> PResult<Option<ObjectId>> {
    let Some(source) = query_object else { return Ok(None) };
    let mut closest: Option<ObjectId> = None;
    let mut closest_dist_sq = f32::MAX;
    player_iterate_objects(h, p, |h, obj| {
        if !h.obj_is_kind_of_multi(obj, set_mask, clear_mask) && !plant(32) {
            return; // Do nothing to the magic running total pointer man.
        }
        // is this the closest one so far
        let dist_sq = h.partition_distance_squared_2d(source, obj);
        if if plant(31) { dist_sq <= closest_dist_sq } else { dist_sq < closest_dist_sq } {
            closest = Some(obj);
            closest_dist_sq = dist_sq;
        }
    })?;
    Ok(closest)
}

/// `Player::hasAnyBuildings()`.
pub fn player_has_any_buildings<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    for proto in protos(h, p)? {
        if proto_has_any_buildings(h, proto) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `Player::hasAnyBuildings(kindOf)`.
pub fn player_has_any_buildings_kind_of<H: PlayerHost>(h: &H, p: PlayerIndex, kind_of: &KindOfMaskType) -> PResult<bool> {
    for proto in protos(h, p)? {
        if if plant(33) { proto_has_any_buildings(h, proto) } else { proto_has_any_buildings_kind_of(h, proto, kind_of) } {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `Player::hasAnyUnits()`.
pub fn player_has_any_units<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    for proto in protos(h, p)? {
        if proto_has_any_units(h, proto) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `Player::hasAnyObjects()`.
pub fn player_has_any_objects<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    for proto in protos(h, p)? {
        if proto_has_any_objects(h, proto) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `Player::hasAnyBuildFacility()`.
pub fn player_has_any_build_facility<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    for proto in protos(h, p)? {
        if proto_has_any_build_facility(h, proto)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `Player::updateTeamStates()`.
pub fn player_update_team_states<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    for proto in protos(h, p)? {
        proto_update_state(h, proto)?;
    }
    Ok(())
}

/// `Player::isLocalPlayer()`.
pub fn player_is_local_player<H: PlayerHost>(h: &H, p: PlayerIndex) -> bool {
    h.players().local_index() == Some(p)
}

/// `Player::setListInScoreScreen(listInScoreScreen)`.
pub fn player_set_list_in_score_screen<H: PlayerHost>(h: &mut H, p: PlayerIndex, v: bool) -> PResult<()> {
    plm(h, p)?.list_in_score_screen = v;
    Ok(())
}

/// `Player::getSupplyBoxValue()`.
pub fn player_get_supply_box_value<H: PlayerHost>(h: &H) -> u32 {
    h.globals().base_value_per_supply_box
}

/// `Player::getProductionCostChangePercent(buildTemplateName)`.
pub fn player_get_production_cost_change_percent<H: PlayerHost>(h: &mut H, p: PlayerIndex, name: &[u8]) -> PResult<f32> {
    let k = key_of(h, name);
    Ok(pl(h, p)?.production_cost_changes.get(&k).copied().unwrap_or(0.0))
}

/// `Player::getProductionTimeChangePercent(buildTemplateName)`.
pub fn player_get_production_time_change_percent<H: PlayerHost>(h: &mut H, p: PlayerIndex, name: &[u8]) -> PResult<f32> {
    let k = key_of(h, name);
    Ok(pl(h, p)?.production_time_changes.get(&k).copied().unwrap_or(0.0))
}

/// `Player::getProductionVeterancyLevel(buildTemplateName)`.
pub fn player_get_production_veterancy_level<H: PlayerHost>(h: &mut H, p: PlayerIndex, name: &[u8]) -> PResult<VeterancyLevel> {
    let k = key_of(h, name);
    Ok(pl(h, p)?.production_veterancy_levels.get(&k).copied().unwrap_or(if plant(34) { VeterancyLevel::Veteran } else { VeterancyLevel::Regular }))
}

/// `Player::setObjectsEnabled(templateTypeToAffect, enable)`.
pub fn player_set_objects_enabled<H: PlayerHost>(h: &mut H, p: PlayerIndex, template_type_to_affect: &[u8], enable: bool) -> PResult<()> {
    for_each_member(h, p, |h, obj| {
        if let Some(t) = h.obj_template(obj) {
            if h.tmpl_name(t) == template_type_to_affect {
                h.obj_set_script_status_disabled(obj, enable == plant(35));
            }
        }
    })
}

/// `Player::allowedToBuild(tmplate)`.
pub fn player_allowed_to_build<H: PlayerHost>(h: &H, p: PlayerIndex, tmplate: TemplateRef) -> PResult<bool> {
    let me = pl(h, p)?;
    if !(if plant(36) { me.can_build_units } else { me.can_build_base }) && h.tmpl_is_kind_of(tmplate, KINDOF_STRUCTURE) {
        return Ok(false);
    }
    if !me.can_build_units && !h.tmpl_is_kind_of(tmplate, KINDOF_STRUCTURE) {
        return Ok(false);
    }
    Ok(true)
}

// ---- radar and power ----------------------------------------------------------------------------------------------

/// `Player::okToPlayRadarEdgeSound()`.
pub fn player_ok_to_play_radar_edge_sound<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    let g = h.globals();
    Ok(!h.victory_has_single_player_been_defeated(p) && !pl(h, p)?.is_player_dead && !g.client_quiet && g.in_game_logic_update && (plant(37) || h.frame() > 0))
}

/// `Player::hasRadar()`.
pub fn player_has_radar<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    let me = pl(h, p)?;
    if me.radar_disabled && (me.disable_proof_radar_count == 0 || plant(41)) {
        return Ok(false); // Nope, no matter how many you have, if I say no, you don't have it
    }
    Ok(me.radar_count > 0)
}

fn radar_sound<H: PlayerHost>(h: &mut H, p: PlayerIndex, online: bool) {
    h.client_event(ClientEvent::Player(PlayerClientEvent::RadarSound { online, player_index: p }));
}

/// `Player::addRadar(disableProof)`.
pub fn player_add_radar<H: PlayerHost>(h: &mut H, p: PlayerIndex, disable_proof: bool) -> PResult<()> {
    let had_radar = player_has_radar(h, p)?;
    {
        let me = plm(h, p)?;
        me.radar_count = me.radar_count.wrapping_add(1);
        if disable_proof && !plant(38) {
            me.disable_proof_radar_count = me.disable_proof_radar_count.wrapping_add(1); // Disable proof is also in the normal refcount
        }
    }
    if !had_radar && player_has_radar(h, p)? && player_ok_to_play_radar_edge_sound(h, p)? {
        radar_sound(h, p, true);
    }
    Ok(())
}

/// `Player::removeRadar(disableProof)`.
pub fn player_remove_radar<H: PlayerHost>(h: &mut H, p: PlayerIndex, disable_proof: bool) -> PResult<()> {
    let had_radar = player_has_radar(h, p)?;
    {
        let me = plm(h, p)?;
        me.radar_count = me.radar_count.wrapping_sub(1);
        if disable_proof {
            me.disable_proof_radar_count = me.disable_proof_radar_count.wrapping_sub(1);
        }
    }
    if (had_radar || plant(39)) && !player_has_radar(h, p)? && player_ok_to_play_radar_edge_sound(h, p)? {
        radar_sound(h, p, false);
    }
    Ok(())
}

/// `Player::disableRadar()`.
pub fn player_disable_radar<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    let had_radar = player_has_radar(h, p)?;
    plm(h, p)?.radar_disabled = true;
    if had_radar && !player_has_radar(h, p)? && (plant(40) || player_ok_to_play_radar_edge_sound(h, p)?) {
        radar_sound(h, p, false);
    }
    Ok(())
}

/// `Player::enableRadar()`.
pub fn player_enable_radar<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    let had_radar = player_has_radar(h, p)?;
    plm(h, p)?.radar_disabled = false;
    if !had_radar && player_has_radar(h, p)? && player_ok_to_play_radar_edge_sound(h, p)? {
        radar_sound(h, p, true);
    }
    Ok(())
}

/// `Player::onPowerBrownOutChange(brownOut)`.
pub fn player_on_power_brown_out_change<H: PlayerHost>(h: &mut H, p: PlayerIndex, brown_out: bool) -> PResult<()> {
    if brown_out != plant(42) {
        player_disable_radar(h, p)?;
    } else {
        player_enable_radar(h, p)?; // This doesn't give radar necessarily, it just removes the restriction
    }
    player_iterate_objects(h, p, |h, obj| {
        // doPowerDisable
        if h.obj_is_kind_of(obj, KINDOF_POWERED) || plant(43) {
            if brown_out {
                h.obj_set_disabled_underpowered(obj); // set disabled has a pauseAllSpecialPowers that prevents double pausing
            } else {
                h.obj_clear_disabled_underpowered(obj);
            }
        }
    })
}

/// What `Energy`'s methods ask of their owner (`m_owner->onPowerBrownOutChange(!hasSufficientPower())`): the caller
/// that changed the energy of player `p` hands the notice here. It goes to the energy's **owner**, which is `p` unless a
/// load (`Energy::xfer` reads the owner's index from the file) has made it another player's; ~~to `p`~~ **CORRECTED
/// 2026-10-20** (found by the `Player::xfer` differential: loading one player's save into another).
pub fn player_deliver_brown_out_notice<H: PlayerHost>(h: &mut H, p: PlayerIndex, notice: super::energy::OwnerNotice) -> PResult<()> {
    if let Some(n) = notice {
        let owner = pl(h, p)?.energy.owner().unwrap_or(p);
        player_on_power_brown_out_change(h, owner, n.brown_out != plant(69))?;
    }
    Ok(())
}

// ---- special power timers ------------------------------------------------------------------------------------------

// --- p1w11-sonnet --- the four operations on the timer list, over the list alone, so that `World` (whose modules need them before the world
// implements `PlayerHost`) and the host-generic functions below are one piece of logic
/// `Player::addNewSharedSpecialPowerTimer(temp, frame)` on the list.
pub fn special_power_timers_add_new(timers: &mut Vec<SpecialPowerReadyTimer>, template_id: u32, frame: u32) {
    timers.push(SpecialPowerReadyTimer { template_id, ready_frame: frame });
}

/// `Player::resetOrStartSpecialPowerReadyFrame(temp)` on the list: `now + reload_time` for a known template, else a new timer at `now`.
pub fn special_power_timers_reset_or_start(timers: &mut Vec<SpecialPowerReadyTimer>, template_id: u32, reload_time: u32, now: u32) {
    for timer in timers.iter_mut() {
        if timer.template_id == template_id {
            timer.ready_frame = if plant(44) { now } else { now.wrapping_add(reload_time) };
            return;
        }
    }
    special_power_timers_add_new(timers, template_id, now);
}

/// `Player::expressSpecialPowerReadyFrame(temp, frame)` on the list.
pub fn special_power_timers_express(timers: &mut Vec<SpecialPowerReadyTimer>, template_id: u32, frame: u32) {
    for timer in timers.iter_mut() {
        if timer.template_id == template_id {
            timer.ready_frame = frame;
            return;
        }
    }
    special_power_timers_add_new(timers, template_id, frame);
}

/// `Player::getOrStartSpecialPowerReadyFrame(temp)` on the list: the timer's frame, or a new timer at `now` (and `now`).
pub fn special_power_timers_get_or_start(timers: &mut Vec<SpecialPowerReadyTimer>, template_id: u32, now: u32) -> u32 {
    for timer in timers.iter() {
        if timer.template_id == template_id {
            return if plant(45) { now } else { timer.ready_frame };
        }
    }
    special_power_timers_add_new(timers, template_id, now);
    now
}
// --- end p1w11-sonnet ---

/// `Player::addNewSharedSpecialPowerTimer(temp, frame)`.
pub fn player_add_new_shared_special_power_timer<H: PlayerHost>(h: &mut H, p: PlayerIndex, temp: SpecialPowerTemplateRef, frame: u32) -> PResult<()> {
    let id = h.special_power_template(temp).id;
    special_power_timers_add_new(&mut plm(h, p)?.special_power_timers, id, frame);
    Ok(())
}

/// `Player::resetOrStartSpecialPowerReadyFrame(temp)`.
pub fn player_reset_or_start_special_power_ready_frame<H: PlayerHost>(h: &mut H, p: PlayerIndex, temp: SpecialPowerTemplateRef) -> PResult<()> {
    let v = h.special_power_template(temp);
    let now = h.frame();
    special_power_timers_reset_or_start(&mut plm(h, p)?.special_power_timers, v.id, v.reload_time, now);
    Ok(())
}

/// `Player::expressSpecialPowerReadyFrame(temp, frame)`.
pub fn player_express_special_power_ready_frame<H: PlayerHost>(h: &mut H, p: PlayerIndex, temp: SpecialPowerTemplateRef, frame: u32) -> PResult<()> {
    let id = h.special_power_template(temp).id;
    special_power_timers_express(&mut plm(h, p)?.special_power_timers, id, frame);
    Ok(())
}

/// `Player::getOrStartSpecialPowerReadyFrame(temp)`.
pub fn player_get_or_start_special_power_ready_frame<H: PlayerHost>(h: &mut H, p: PlayerIndex, temp: SpecialPowerTemplateRef) -> PResult<u32> {
    let id = h.special_power_template(temp).id;
    let now = h.frame();
    Ok(special_power_timers_get_or_start(&mut plm(h, p)?.special_power_timers, id, now))
}

// ---- production changes by kind -------------------------------------------------------------------------------------

/// `Player::addKindOfProductionCostChange(kindOf, percent)`.
pub fn player_add_kind_of_production_cost_change<H: PlayerHost>(h: &mut H, p: PlayerIndex, kind_of: KindOfMaskType, percent: f32) -> PResult<()> {
    let me = plm(h, p)?;
    for tof in me.kind_of_production_changes.iter_mut() {
        if tof.percent == percent && tof.kind_of == kind_of {
            if !plant(46) {
                tof.ref_count = tof.ref_count.wrapping_add(1);
            }
            return Ok(());
        }
    }
    me.kind_of_production_changes.push(KindOfPercentProductionChange { kind_of, percent, ref_count: 1 });
    Ok(())
}

/// `Player::removeKindOfProductionCostChange(kindOf, percent)`: nothing for an entry that is not there (upstream
/// asserts, which the release build skips).
pub fn player_remove_kind_of_production_cost_change<H: PlayerHost>(h: &mut H, p: PlayerIndex, kind_of: KindOfMaskType, percent: f32) -> PResult<()> {
    let me = plm(h, p)?;
    if let Some(i) = me.kind_of_production_changes.iter().position(|t| t.percent == percent && t.kind_of == kind_of) {
        let r = me.kind_of_production_changes[i].ref_count.wrapping_sub(if plant(47) { 2 } else { 1 });
        me.kind_of_production_changes[i].ref_count = r;
        if r == 0 {
            me.kind_of_production_changes.remove(i);
        }
    }
    Ok(())
}

/// `Player::getProductionCostChangeBasedOnKindOf(kindOf)`: `TEST_KINDOFMASK_MULTI(kindOf, tof->m_kindOf,
/// KINDOFMASK_NONE)` is `kindOf.testSetAndClear(tof.kindOf, NONE)`: every bit of the entry is set in `kindOf`.
pub fn player_get_production_cost_change_based_on_kind_of<H: PlayerHost>(h: &H, p: PlayerIndex, kind_of: &KindOfMaskType) -> PResult<f32> {
    let mut start = 1.0f32;
    for tof in pl(h, p)?.kind_of_production_changes.iter() {
        if kind_of.test_set_and_clear(&tof.kind_of, &crate::common::kind_of::KINDOFMASK_NONE) {
            start *= if plant(48) { tof.percent } else { 1.0 + tof.percent };
        }
    }
    Ok(start)
}

// ---- attacked by, status ------------------------------------------------------------------------------------------

/// `Player::setAttackedBy(playerNdx)`: an index outside `0..MAX_PLAYER_COUNT` writes outside the array upstream.
pub fn player_set_attacked_by<H: PlayerHost>(h: &mut H, p: PlayerIndex, player_ndx: i32) -> PResult<()> {
    let frame = h.frame();
    let me = plm(h, p)?;
    *me.attacked_by.get_mut(usize::try_from(player_ndx).map_err(|_| PlayerError::NoPlayer)?).ok_or(PlayerError::NoPlayer)? = true;
    if !plant(49) {
        me.attacked_frame = frame;
    }
    Ok(())
}

/// `Player::getAttackedBy(playerNdx)`.
pub fn player_get_attacked_by<H: PlayerHost>(h: &H, p: PlayerIndex, player_ndx: i32) -> PResult<bool> {
    pl(h, p)?.attacked_by.get(usize::try_from(player_ndx).map_err(|_| PlayerError::NoPlayer)?).copied().ok_or(PlayerError::NoPlayer)
}

/// `Player::isPlayerObserver()`.
pub fn player_is_player_observer<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    Ok(pl(h, p)?.observer)
}

/// `Player::isPlayerDead()`.
pub fn player_is_player_dead<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    Ok(pl(h, p)?.is_player_dead)
}

/// `Player::isPlayerActive()`.
pub fn player_is_player_active<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    let me = pl(h, p)?;
    Ok(!me.observer && (plant(50) || !me.is_player_dead))
}

/// `Player::isPlayableSide()`.
pub fn player_is_playable_side<H: PlayerHost>(h: &H, p: PlayerIndex) -> PResult<bool> {
    Ok(match pl(h, p)?.template {
        Some(t) => h.player_template(t).playable_side,
        None => false,
    })
}

// ---- sciences and ranks -------------------------------------------------------------------------------------------

/// `Player::hasScience(t)`.
pub fn player_has_science<H: PlayerHost>(h: &H, p: PlayerIndex, t: ScienceType) -> PResult<bool> {
    Ok(pl(h, p)?.sciences.contains(&t))
}

/// `Player::isScienceDisabled(t)`.
pub fn player_is_science_disabled<H: PlayerHost>(h: &H, p: PlayerIndex, t: ScienceType) -> PResult<bool> {
    Ok(pl(h, p)?.sciences_disabled.contains(&t))
}

/// `Player::isScienceHidden(t)`.
pub fn player_is_science_hidden<H: PlayerHost>(h: &H, p: PlayerIndex, t: ScienceType) -> PResult<bool> {
    Ok(pl(h, p)?.sciences_hidden.contains(&t))
}

/// `Player::hasPrereqsForScience(t)`.
pub fn player_has_prereqs_for_science<H: PlayerHost>(h: &H, p: PlayerIndex, t: ScienceType) -> bool {
    h.science_has_prereqs(p, t)
}

/// `Player::setScienceAvailability(science, type)`.
pub fn player_set_science_availability<H: PlayerHost>(h: &mut H, p: PlayerIndex, science: ScienceType, ty: ScienceAvailabilityType) -> PResult<()> {
    let me = plm(h, p)?;
    let mut found = false;
    if let Some(i) = me.sciences_disabled.iter().position(|s| *s == science) {
        if !plant(67) {
            me.sciences_disabled.remove(i);
        }
        found = true;
    }
    if !found {
        if let Some(i) = me.sciences_hidden.iter().position(|s| *s == science) {
            me.sciences_hidden.remove(i);
        }
    }
    match ty {
        ScienceAvailabilityType::Disabled => me.sciences_disabled.push(science),
        ScienceAvailabilityType::Hidden => me.sciences_hidden.push(science),
        _ => {}
    }
    Ok(())
}

/// `Player::getScienceAvailabilityTypeFromString(name)`: case-insensitive.
pub fn player_get_science_availability_type_from_string(name: &[u8]) -> ScienceAvailabilityType {
    for (i, n) in SCIENCE_AVAILABILITY_NAMES.iter().enumerate() {
        if if plant(68) { name == n.as_bytes() } else { name.eq_ignore_ascii_case(n.as_bytes()) } {
            return match i {
                0 => ScienceAvailabilityType::Available,
                1 => ScienceAvailabilityType::Disabled,
                _ => ScienceAvailabilityType::Hidden,
            };
        }
    }
    ScienceAvailabilityType::Invalid
}

/// `Player::addScience(science)`: gives the science; false if the player already had it.
pub fn player_add_science<H: PlayerHost>(h: &mut H, p: PlayerIndex, science: ScienceType) -> PResult<bool> {
    if player_has_science(h, p, science)? && !plant(72) {
        return Ok(false);
    }
    plm(h, p)?.sciences.push(science);
    for proto in protos(h, p)? {
        let mut cur_t = h.teams().first_instance(proto);
        while let Some(t) = cur_t {
            let mut cur_o = h.teams().first_member(t);
            while let Some(o) = cur_o {
                for m in h.obj_special_power_modules(o) {
                    if h.sp_required_science(m) == science || (plant(73) && h.sp_required_science(m).0 == science.0 + 1) {
                        h.sp_on_special_power_creation(m);
                        let f = h.frame();
                        h.sp_set_ready_frame(m, f);
                    }
                }
                cur_o = h.teams().next_member(o);
            }
            cur_t = h.teams().next_instance(t);
        }
        if !plant(52) {
            h.client_event(ClientEvent::Player(PlayerClientEvent::ControlBarDirty)); // Refresh the UI to show new cameos, etc
        }
    }
    if !plant(51) {
        h.script_notify_of_acquired_science(p, science);
    }
    Ok(true)
}

/// `Player::addSciencePurchasePoints(delta)`.
pub fn player_add_science_purchase_points<H: PlayerHost>(h: &mut H, p: PlayerIndex, delta: i32) -> PResult<()> {
    let me = plm(h, p)?;
    let old = me.science_purchase_points;
    me.science_purchase_points = me.science_purchase_points.wrapping_add(delta);
    if me.science_purchase_points < 0 && !plant(53) {
        me.science_purchase_points = 0;
    }
    if old != me.science_purchase_points || plant(54) {
        h.client_event(ClientEvent::Player(PlayerClientEvent::SciencePurchasePointsChanged { player_index: p }));
    }
    Ok(())
}

/// `Player::isCapableOfPurchasingScience(science)`.
pub fn player_is_capable_of_purchasing_science<H: PlayerHost>(h: &H, p: PlayerIndex, science: ScienceType) -> PResult<bool> {
    if science == SCIENCE_INVALID {
        return Ok(false);
    }
    if player_has_science(h, p, science)? {
        return Ok(false);
    }
    if player_is_science_disabled(h, p, science)? || player_is_science_hidden(h, p, science)? {
        return Ok(false);
    }
    if !player_has_prereqs_for_science(h, p, science) {
        return Ok(false);
    }
    let cost = h.science_purchase_cost(science);
    if cost == 0 || (if plant(55) { cost >= pl(h, p)?.science_purchase_points } else { cost > pl(h, p)?.science_purchase_points }) {
        return Ok(false);
    }
    Ok(true)
}

/// `Player::attemptToPurchaseScience(science)`.
pub fn player_attempt_to_purchase_science<H: PlayerHost>(h: &mut H, p: PlayerIndex, science: ScienceType) -> PResult<bool> {
    if !player_is_capable_of_purchasing_science(h, p, science)? {
        return Ok(false);
    }
    let cost = h.science_purchase_cost(science);
    player_add_science_purchase_points(h, p, cost.wrapping_neg())?;
    player_add_science(h, p, science)?;
    if !plant(56) {
        h.academy_record_generals_points_spent(p, cost);
    }
    if h.players().local_index() == Some(p) {
        h.client_event(ClientEvent::Player(PlayerClientEvent::ControlBarDirty));
    }
    Ok(true)
}

/// `Player::grantScience(science)`.
pub fn player_grant_science<H: PlayerHost>(h: &mut H, p: PlayerIndex, science: ScienceType) -> PResult<bool> {
    if !h.science_is_grantable(science) && !plant(57) {
        return Ok(false); // it's not grantable, so tough, can't have it, even via this method.
    }
    player_add_science(h, p, science)
}

/// `Player::resetSciences()`: back to the intrinsic ones of the template and those of the ranks reached.
pub fn player_reset_sciences<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    let intrinsic = pl(h, p)?.template.map(|t| h.player_template(t).intrinsic_sciences);
    {
        let me = plm(h, p)?;
        me.sciences.clear();
        if let Some(v) = intrinsic {
            if !plant(58) {
                me.sciences = v;
            }
        }
    }
    let rank_level = pl(h, p)?.rank_level;
    let mut i = 1;
    while i <= rank_level {
        if let Some(rank) = h.rank_info(i) {
            for s in rank.sciences_granted.iter() {
                player_add_science(h, p, *s)?;
            }
        }
        i += 1;
    }
    for s in pl(h, p)?.sciences.clone() {
        h.script_notify_of_acquired_science(p, s);
    }
    Ok(())
}

/// `Player::resetRank()`.
pub fn player_reset_rank<H: PlayerHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    let next_rank = h.rank_info(if plant(76) { 3 } else { 2 });
    let cur_rank = h.rank_info(1);
    let intrinsic_spp = match pl(h, p)?.template {
        Some(t) => h.player_template(t).intrinsic_science_purchase_points,
        None => 0,
    };
    let general_name = h.game_text_general_name();
    {
        let me = plm(h, p)?;
        me.rank_level = 1;
        me.skill_points = 0;
        me.level_up = next_rank.map(|r| r.skill_points_needed).unwrap_or(i32::MAX);
        me.level_down = plant(77) as i32;
        me.sciences.clear();
        me.science_purchase_points = intrinsic_spp.wrapping_add(if plant(59) { 0 } else { cur_rank.map(|r| r.science_purchase_points_granted).unwrap_or(0) });
        me.general_name = general_name;
    }
    player_reset_sciences(h, p)
}

/// `Player::setRankLevel(newLevel)`: true if the rank level really changed.
pub fn player_set_rank_level<H: PlayerHost>(h: &mut H, p: PlayerIndex, new_level: i32) -> PResult<bool> {
    let mut new_level = new_level;
    if new_level < 1 {
        new_level = 1;
    } else if new_level > h.rank_level_count() {
        new_level = h.rank_level_count();
    }
    if new_level > h.globals().rank_level_limit && !plant(60) {
        new_level = h.globals().rank_level_limit;
    }
    if new_level == pl(h, p)?.rank_level {
        return Ok(false);
    }
    let old_spp = pl(h, p)?.science_purchase_points;
    if new_level < pl(h, p)?.rank_level
        && !plant(75) {
            player_reset_rank(h, p)?;
        }
    let mut i = pl(h, p)?.rank_level + 1;
    while i <= new_level {
        if let Some(rank) = h.rank_info(i) {
            let me = plm(h, p)?;
            me.science_purchase_points = me.science_purchase_points.wrapping_add(rank.science_purchase_points_granted);
            if me.science_purchase_points < 0 {
                me.science_purchase_points = 0;
            }
            if me.skill_points < rank.skill_points_needed && !plant(61) {
                me.skill_points = rank.skill_points_needed;
            }
            for s in rank.sciences_granted.iter() {
                player_add_science(h, p, *s)?;
            }
            if !plant(62) {
                plm(h, p)?.level_down = rank.skill_points_needed;
            }
        }
        i += 1;
    }
    let next_rank = h.rank_info(new_level.wrapping_add(1));
    {
        let me = plm(h, p)?;
        me.level_up = next_rank.map(|r| r.skill_points_needed).unwrap_or(i32::MAX);
        me.rank_level = new_level + plant(74) as i32;
    }
    // TheControlBar != NULL
    if (pl(h, p)?.level_up != 0 || plant(63)) && h.players().local_index() == Some(p) {
        h.client_event(ClientEvent::Player(PlayerClientEvent::Eva(EvaMessage::GeneralLevelUp)));
    }
    h.client_event(ClientEvent::Player(PlayerClientEvent::RankChanged { player_index: p }));
    if old_spp != pl(h, p)?.science_purchase_points {
        h.client_event(ClientEvent::Player(PlayerClientEvent::SciencePurchasePointsChanged { player_index: p }));
    }
    Ok(true)
}

/// `Player::addSkillPoints(delta)`: true if the player gained levels.
pub fn player_add_skill_points<H: PlayerHost>(h: &mut H, p: PlayerIndex, delta: i32) -> PResult<bool> {
    let modifier = pl(h, p)?.skill_points_modifier;
    let delta = if plant(64) { crate::common::base_type::real_to_int(modifier * (delta as f32)) } else { real_to_int_ceil(modifier * (delta as f32)) };
    if delta == 0 {
        return Ok(false);
    }
    let level_cap = if plant(65) { h.rank_level_count() } else { h.globals().rank_level_limit.min(h.rank_level_count()) };
    let point_cap = h.rank_info(level_cap).ok_or(PlayerError::NoRank)?.skill_points_needed; // Cap at lowest point of cap level, not highest.
    let mut level_gained = false;
    {
        let me = plm(h, p)?;
        me.skill_points = point_cap.min(me.skill_points.wrapping_add(delta));
    }
    let mut rounds = 0;
    while pl(h, p)?.skill_points >= pl(h, p)?.level_up {
        rounds += 1;
        if rounds > SPIN_LIMIT {
            return Err(PlayerError::Hang);
        }
        let next = pl(h, p)?.rank_level.wrapping_add(1);
        player_set_rank_level(h, p, next)?;
        level_gained = true;
    }
    Ok(level_gained)
}

/// `Player::addSkillPointsForKill(killer, victim)`: true if the player gained levels.
pub fn player_add_skill_points_for_kill<H: PlayerHost>(h: &mut H, p: PlayerIndex, killer: Option<ObjectId>, victim: Option<ObjectId>) -> PResult<bool> {
    let (Some(_), Some(v)) = (killer, victim) else { return Ok(false) };
    if h.obj_test_status(v, OBJECT_STATUS_UNDER_CONSTRUCTION) && !plant(66) {
        return Ok(false);
    }
    let victim_level = h.obj_veterancy_level(v);
    let skill_value = match h.obj_template(v) {
        Some(t) => h.tmpl_skill_point_value(t, victim_level),
        None => return Err(PlayerError::NoRank),
    };
    player_add_skill_points(h, p, skill_value)
}

/// (test-only) the planted bug that is switched on by `PLAYER_PLANT`.
pub fn plant(k: u32) -> bool {
    use std::sync::OnceLock;
    static P: OnceLock<u32> = OnceLock::new();
    *P.get_or_init(|| std::env::var("PLAYER_PLANT").ok().and_then(|s| s.parse().ok()).unwrap_or(0)) == k
}
