// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file (this repository's own design, not upstream code):
// the cut points of `Object` (layer 8). Where a default implementation here
// stands in for a unit that is another layer's, the comment names the
// upstream code it is a minimal copy of.

//! The cut points of `Object`: every call `Object.cpp` makes into a unit that
//! is not translated yet (or is another wave's), listed in one place.
//!
//! # The two kinds
//!
//! - **Actions** are [`CutCall`]s: what upstream does to a player, a team, the
//!   partition manager, the pathfinder, the AI, the script engine and the
//!   weapon set. Each is recorded in [`ObjectCuts::log`] (when it is `Some`)
//!   in call order, and does nothing else. The unit that owns the call
//!   implements it in place of the `World::cut` call; the enum is the list.
//! - **Queries** are methods of [`ObjectCuts`] over plain tables the game
//!   setup (or a test) fills: which player controls a team, the relationship
//!   of two teams, a player's mask, colour and completed upgrades. They have
//!   upstream's defaults for what is not set (a team nobody controls, neutral
//!   relationships). Queries are not recorded.
//!
//! The values of `TheGlobalData`, `TheAI->getAiData()` and the script engine
//! that `Object.cpp` reads are [`ObjectGlobals`].
//!
//! # What is cut, by unit (the list the lead asked for)
//!
//! | unit (upstream) | calls | here |
//! |---|---|---|
//! | `Player` (layer 7) | `becomingTeamMember`, `getScoreKeeper()->add*`, `addSkillPointsForKill`, `doBountyForKill`, `friend_applyDifficultyBonusesForObject`, `applyBattlePlanBonusesForObject`, `removeRadar/addRadar`, `getRelationship`, `getPlayerMask`, `getPlayerColor`, `getCompletedUpgradeMask`, `isPlayableSide`, `isPlayerActive`, `isSkirmishAIPlayer`, `getDefaultTeam`, `getProductionVeterancyLevel`, `getNumBattlePlansActive` | `CutCall::Player*` and the player queries |
//! | `Team` (layer 7) | `getControllingPlayer`, `getName`, `getRelationship`, the member list, `notifyTeamOfObjectDeath`, `setEnteredExited`, `getPrototype()->getTemplateInfo()`, the attack priority | the team queries, `CutCall::Team*` |
//! | `PlayerList` | `getNeutralPlayer()->getDefaultTeam()`, `getPlayersWithRelationship` | `neutral_default_team`, `players_with_relationship` |
//! | `Energy` (wave 6 `energy`) | `objectEnteringInfluence`, `objectLeavingInfluence`, `adjustPower` | `CutCall::Energy*` |
//! | `PartitionManager` (wave 6 `partition`) | `registerObject`, `unRegisterObject`, `PartitionData::makeDirty`, `doShroudReveal`, `queueUndoShroudReveal`, `doShroudCover`, `undoShroudCover`, `do/undoValueAffect`, `do/undoThreatAffect`, `iteratePotentialCollisions`, `iterateObjectsInRange` | `CutCall::Partition*`, the two scripted iterations |
//! | `ExperienceTracker` (wave 6 `trackers`) | `setVeterancyLevel`, `setExperienceAndLevel`, `addExperiencePoints`, `getExperienceValue`, `isAcceptingExperiencePoints` | default copies of the tracker's logic over `Object::experience`, below |
//! | `FiringTracker`, the helpers (layer 9) | `sleepUntil`, `shotFired`, `startDefectionTimer`, `notifySubdualDamage`, `doStatusDamage`, `doTempWeaponBonus` | `make_helper` makes a stub; the calls are `ModuleCall`s |
//! | `WeaponSet`, `Weapon` (layer 8, not translated) | `updateWeaponSet`, `weaponSetOnWeaponBonusChange`, the slot queries of `isAbleToAttack`, `fireCurrentWeapon` and the other 25 forwards | `CutCall::WeaponSet*`, `weapon_in_slot`, `weapon_set_has_any_weapon`; ~~**the forwards are not translated**~~ **CORRECTED 2026-10-20 (`p1w8-sonnet`)** — the forwards are `logic::world::object_weapons`, generic over a host the full-game adapter provides; `Object` holds the real `WeaponSet`, and the two table queries here are only consulted in addition to it |
//! | `AIUpdateInterface`, `TheAI` (layer 10) | `setAttitude`, `setAttackInfo`, `aiIdle`, `transferAttack`, `getTurretRotAndPitch`, `getWhichTurretForWeaponSlot`, `isTurretEnabled`, `getCurLocomotor`, `getDozerAIInterface`, `pathfinder()->updatePos/removePos/addObjectToPathfindMap/removeObjectFromPathfindMap`, `AIGroup::remove` | `CutCall::Ai*`, `CutCall::Pathfinder*`, `CutCall::Dozer*` |
//! | `ScriptEngine` (layer 11) | `notifyOfObjectCreationOrDestruction`, `notifyOfObjectDestruction`, `getAttackInfo`, `getObjectsShouldReceiveDifficultyBonus`, `AppendDebugMessage` | `CutCall::ScriptEngine*`, `ObjectGlobals` |
//! | `GameLogic` | `updateObjectsChangedTriggerAreas`, `deselectObject`, `getDrawIconUI` | `CutCall::GameLogic*`, `ObjectGlobals` |
//! | `BuildAssistant` | `sellObject` | `CutCall::BuildAssistantSellObject` |
//! | `RankInfo`/academy | `AcademyStats::recordMine` | `CutCall::AcademyStatsRecordMine` |
//! | `SpecialPowerStore` | `canUseSpecialPower` | `ObjectCuts::special_power_can_use` |
//! | `TerrainLogic` bridges, pathfinder wall | `getLayerHeight` on the wall layer | `wall_height` |
//! | `Drawable` (client) | every `m_drawable->...` call | `ObjectClientEvent` |
//! | `TheAudio`, `TheRadar`, `TheInGameUI`, `TheControlBar`, `TheEva`, `TheGameClient` | see `object_events.rs` | `ObjectClientEvent` |
//! | module factory, `ModuleInfo` | `newModule` | the caller (`NewObject::modules`) |
//! | `CommandButton`, `TheControlBar->findCommandSet` | `doCommandButton*`, `canProduceUpgrade` | **not translated** (client/GUI types) |
//! | bone positions (`Drawable::getPristineBonePositions` and friends) | `getSingleLogicalBonePosition*`, `getMultiLogicalBonePosition` | **not translated** (the model's bone data is a client read) |

use std::collections::{BTreeMap, BTreeSet};

use crate::common::game_common::{ObjectShroudStatus, PlayerMaskType, Relationship, VeterancyLevel};
use crate::common::game_type::{DrawableId, ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};
use crate::common::upgrade::UpgradeMaskType;
use crate::logic::client_event::ClientEvent;
use crate::logic::world::game_logic::World;
use crate::logic::world::ids::{PlayerIndex, TeamId};
use crate::logic::world::module_call::{ModuleCall, ModuleHandle};
use crate::logic::world::object::HelperKind;
use crate::logic::world::object_create::NewModule;
use crate::logic::world::object_events::ObjectClientEvent;

/// `ALLOW_SAME_PLAYER` (`PlayerList.h`).
pub const ALLOW_SAME_PLAYER: u32 = 0x01;
/// `ALLOW_ALLIES`.
pub const ALLOW_ALLIES: u32 = 0x02;
/// `ALLOW_ENEMIES`.
pub const ALLOW_ENEMIES: u32 = 0x04;
/// `ALLOW_NEUTRAL`.
pub const ALLOW_NEUTRAL: u32 = 0x08;

// --- p1w12-sonnet ---
/// What a module reads of `Object::getCurrentWeapon()`: the weapon's slot, `getLastShotFrame()` and `getPossibleNextShotFrame()`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponFacts {
    /// `getWeaponSlot()`.
    pub slot: i32,
    /// `getLastShotFrame()`.
    pub last_shot_frame: u32,
    /// `getPossibleNextShotFrame()`.
    pub possible_next_shot_frame: u32,
}

impl World {
    /// `Object::getCurrentWeapon()` as a module reads it: the weapon set's current weapon when it has one, else the table `cuts.cur_weapon` (see
    /// `modules::fire_ocl_after_weapon_cooldown_update`). `None` is `NULL`.
    pub fn current_weapon_facts(&self, id: ObjectId) -> Option<WeaponFacts> {
        let o = self.objects.find(id)?;
        if let Some(w) = o.weapon_set.get_cur_weapon() {
            return Some(WeaponFacts { slot: w.weapon_slot(), last_shot_frame: w.last_shot_frame(), possible_next_shot_frame: w.possible_next_shot_frame() });
        }
        self.cuts.cur_weapon.get(&id).copied()
    }

    /// `Object::getWeaponInWeaponSlot(slot) != NULL`: the weapon set's, or the table `cuts.weapons`.
    pub fn weapon_in_slot_exists(&self, id: ObjectId, slot: i32) -> bool {
        self.objects.find(id).is_some_and(|o| o.weapon_set.get_weapon_in_weapon_slot(slot).is_some()) || self.cuts.weapon_in_slot(id, slot)
    }

    /// `obj->getBodyModule()->getHealth()` and `getMaxHealth()`; `None` for an object with no body module (`NULL`).
    pub fn body_health(&mut self, id: ObjectId) -> Option<(f32, f32)> {
        let body = self.get_body_module(id)?;
        let health = self.module_call(id, body, ModuleCall::GetHealth).as_real(0.0);
        let max_health = self.module_call(id, body, ModuleCall::GetMaxHealth).as_real(0.0);
        Some((health, max_health))
    }

    /// `TheTerrainLogic->findClosestEdgePoint(&pos)`: `pos` moved by `cuts.edge_point_offset` (the real terrain logic is the adapter's).
    pub fn terrain_find_closest_edge_point(&self, pos: crate::common::base_type::Coord3D) -> crate::common::base_type::Coord3D {
        let o = self.cuts.edge_point_offset;
        crate::common::base_type::Coord3D { x: pos.x + o.x, y: pos.y + o.y, z: pos.z + o.z }
    }

    /// `TheAudio->addAudioEvent(&AudioEventRTS(name, object))`: the handle the audio manager gave it (never 0).
    pub fn audio_add_event(&mut self, name: Vec<u8>, object_id: ObjectId) -> u32 {
        self.emit(ObjectClientEvent::AudioAddEvent { name, object_id, position: None, player_index: None });
        self.cuts.last_audio_handle = self.cuts.last_audio_handle.wrapping_add(1);
        self.cuts.last_audio_handle
    }

    /// `TheAudio->removeAudioEvent(handle)`.
    pub fn audio_remove_event(&mut self, handle: u32) {
        self.emit(ObjectClientEvent::AudioRemoveEvent { handle });
    }

    /// `TheAI->findClosestEnemy(obj, range, qualifiers) != NULL`.
    pub fn ai_find_closest_enemy(&mut self, id: ObjectId, range: f32, qualifiers: u32) -> bool {
        let minor_radius = self.objects.find(id).map_or(0.0, |o| o.geometry_info.get_minor_radius());
        self.cut(CutCall::AiFindClosestEnemy { object: id, range, qualifiers, minor_radius });
        self.cuts.ai_enemy_near.contains(&id)
    }

    /// `TheAI->findClosestAlly(obj, range, qualifiers) != NULL`.
    pub fn ai_find_closest_ally(&mut self, id: ObjectId, range: f32, qualifiers: u32) -> bool {
        let minor_radius = self.objects.find(id).map_or(0.0, |o| o.geometry_info.get_minor_radius());
        self.cut(CutCall::AiFindClosestAlly { object: id, range, qualifiers, minor_radius });
        self.cuts.ai_ally_near.contains(&id)
    }

    /// `TheParticleSystemManager->createParticleSystem(tmpl)`, `system->setPosition(&position)` and `system->getSystemID()`: the id the client gave it.
    pub fn create_particle_system(&mut self, template: Vec<u8>, position: crate::common::base_type::Coord3D) -> u32 {
        self.cuts.last_particle_system_id = self.cuts.last_particle_system_id.wrapping_add(1);
        let id = self.cuts.last_particle_system_id;
        self.cut(CutCall::ParticleSystemCreate { template, position, id });
        id
    }
}
// --- end p1w12-sonnet ---
// --- p1w13-sonnet ---
/// The cell of the cut-point terrain tables a position falls in: an integer hash of the bits of (x, y), so that both sides of a differential test
/// (which share no terrain) answer alike. (A stand-in only until the layer-12 adapter answers from the real terrain.)
pub fn terrain_cell(x: f32, y: f32) -> usize {
    ((x.to_bits() ^ y.to_bits().rotate_left(13)).wrapping_mul(2_654_435_761) >> 28) as usize & 7
}

impl World {
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    pub fn terrain_get_ground_height(&mut self, x: f32, y: f32) -> f32 {
        self.cut(CutCall::TerrainGetGroundHeight { x, y });
        self.cuts.terrain_ground[terrain_cell(x, y)]
    }

    /// `TheTerrainLogic->getHighestLayerForDestination(pos)`.
    pub fn terrain_get_highest_layer_for_destination(&mut self, pos: &crate::common::base_type::Coord3D) -> crate::common::game_type::PathfindLayer {
        self.cut(CutCall::TerrainGetHighestLayerForDestination { x: pos.x, y: pos.y, z: pos.z });
        self.cuts.terrain_layer[terrain_cell(pos.x, pos.y)]
    }

    /// `TheTerrainLogic->getLayerHeight(x, y, layer)`.
    pub fn terrain_get_layer_height(&mut self, x: f32, y: f32, layer: crate::common::game_type::PathfindLayer) -> f32 {
        self.cut(CutCall::TerrainGetLayerHeight { x, y, layer });
        self.cuts.terrain_layer_height[terrain_cell(x, y)]
    }

    /// `TheTerrainLogic->isUnderwater(x, y, &waterZ)`: the water height if there is water at (x, y), `None` if the call leaves `waterZ` as it was.
    pub fn terrain_is_underwater_water_z(&mut self, x: f32, y: f32) -> Option<f32> {
        self.cut(CutCall::TerrainIsUnderwater { x, y });
        let cell = terrain_cell(x, y);
        self.cuts.terrain_has_water[cell].then(|| self.cuts.terrain_water[cell])
    }

    /// `ThePartitionManager->iterateObjectsInRange(object, range, FROM_BOUNDINGSPHERE_3D, { PartitionFilterAcceptByKindOf(STRUCTURE) })`: the objects, in
    /// iteration order.
    pub fn partition_iterate_structures_in_range(&mut self, object: ObjectId, range: f32) -> Vec<ObjectId> {
        self.cut(CutCall::PartitionIterateStructuresInRange { object, range });
        self.cuts.range_scan.clone()
    }

    /// `ThePartitionManager->getClosestObject(object, range, FROM_CENTER_3D, filters)`: the candidates the manager would test the filters on, nearest first (the
    /// caller's own filter chooses the first that passes, which is the closest object that passes all).
    pub fn partition_closest_candidates(&mut self, object: ObjectId, range: f32) -> Vec<ObjectId> {
        self.cut(CutCall::PartitionGetClosestObject { object, range });
        self.cuts.range_scan.clone()
    }

    /// `TheParticleSystemManager->destroyAttachedSystems(obj)`.
    pub fn particle_destroy_attached_systems(&mut self, object: ObjectId) {
        self.cut(CutCall::ParticleDestroyAttachedSystems { object });
    }
}
// --- end p1w13-sonnet ---

/// What `Object` does to a unit that is not translated. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub enum CutCall {
    // ---- Player ----
    /// `player->becomingTeamMember(obj, incoming)`.
    PlayerBecomingTeamMember { player: PlayerIndex, object: ObjectId, incoming: bool },
    /// `player->getScoreKeeper()->addObjectCaptured(obj)`.
    PlayerScoreAddObjectCaptured { player: PlayerIndex, object: ObjectId },
    /// `player->getScoreKeeper()->addObjectLost(obj)`.
    PlayerScoreAddObjectLost { player: PlayerIndex, object: ObjectId },
    /// `player->getScoreKeeper()->addObjectDestroyed(obj)`.
    PlayerScoreAddObjectDestroyed { player: PlayerIndex, object: ObjectId },
    /// `player->addSkillPointsForKill(killer, victim)`.
    PlayerAddSkillPointsForKill { player: PlayerIndex, killer: ObjectId, victim: ObjectId },
    /// `player->doBountyForKill(killer, victim)`.
    PlayerDoBountyForKill { player: PlayerIndex, killer: ObjectId, victim: ObjectId },
    /// `player->friend_applyDifficultyBonusesForObject(obj, apply)`.
    PlayerApplyDifficultyBonusesForObject { player: PlayerIndex, object: ObjectId, apply: bool },
    /// `player->applyBattlePlanBonusesForObject(obj)`.
    PlayerApplyBattlePlanBonusesForObject { player: PlayerIndex, object: ObjectId },
    /// `player->removeRadar(disableProof)`.
    PlayerRemoveRadar { player: PlayerIndex, disable_proof: bool },
    /// `player->addRadar(disableProof)`.
    PlayerAddRadar { player: PlayerIndex, disable_proof: bool },
    // --- p1l9-upgrades ---
    /// `player->grantScience(science)` (`GrantScienceUpgrade`).
    PlayerGrantScience { player: PlayerIndex, science: i32 },
    // --- end p1l9-upgrades ---
    // --- p1w10-sonnet ---
    /// `player->addUpgrade(upgrade, UPGRADE_STATUS_COMPLETE)` (`GrantUpgradeCreate`); `upgrade` is the template's
    /// name key.
    PlayerAddUpgradeComplete { player: PlayerIndex, upgrade: u32 },
    /// `player->getAcademyStats()->recordUpgrade(upgrade, granted)` (`GrantUpgradeCreate`).
    PlayerAcademyRecordUpgrade { player: PlayerIndex, upgrade: u32, granted: bool },
    /// `adjustModelConditionForWeaponStatus()` (`ObjectWeaponStatusHelper::update`, every frame of an object that can have a
    /// weapon): the weapon half of the object is layer 12's.
    AdjustModelConditionForWeaponStatus { object: ObjectId },
    /// `ai->setLocomotorUpgrade(true)` (`LocomotorSetUpgrade`).
    AiSetLocomotorUpgrade { object: ObjectId },
    /// `contain->setPassengerAllowedToFire(TRUE)` (`PassengersFireUpgrade`).
    ContainSetPassengerAllowedToFire { object: ObjectId },
    /// `spawnBehavior->giveSlavesStealthUpgrade(TRUE)` (`StealthUpgrade`).
    SpawnGiveSlavesStealthUpgrade { object: ObjectId },
    /// `ObjectCreationList::create(ocl, obj, NULL)` for the list named `ocl` (`ObjectCreationUpgrade`); `OclHost` answers it, and a name that is
    /// no list makes nothing (upstream's pointer was `NULL`).
    OclCreate { object: ObjectId, ocl: Vec<u8> },
    // --- end p1w10-sonnet ---
    // --- p1w11-sonnet ---
    /// `player->addKindOfProductionCostChange(kindOf, percent)` (`CostModifierUpgrade`).
    PlayerAddKindOfProductionCostChange { player: PlayerIndex, kind_of: crate::common::kind_of::KindOfMaskType, percent: f32 },
    /// `player->removeKindOfProductionCostChange(kindOf, percent)` (`CostModifierUpgrade`).
    PlayerRemoveKindOfProductionCostChange { player: PlayerIndex, kind_of: crate::common::kind_of::KindOfMaskType, percent: f32 },
    /// `player->addPowerBonus(obj)` (`PowerPlantUpgrade`): `m_energy.addPowerBonus(obj)`.
    PlayerAddPowerBonus { player: PlayerIndex, object: ObjectId },
    /// `player->removePowerBonus(obj)` (`PowerPlantUpgrade`).
    PlayerRemovePowerBonus { player: PlayerIndex, object: ObjectId },
    /// `player->onStructureConstructionComplete(builder, structure, isRebuild)` (`ReplaceObjectUpgrade`).
    PlayerOnStructureConstructionComplete { player: PlayerIndex, builder: ObjectId, structure: ObjectId, is_rebuild: bool },
    /// `ObjectCreationList::create(ocl, primary, secondary)` for the list named `ocl` with a secondary object (`EjectPilotDie`'s damage dealer;
    /// `INVALID_ID` for `NULL`); like [`CutCall::OclCreate`], a name that is no list makes nothing.
    OclCreateWithSecondary { primary: ObjectId, secondary: ObjectId, ocl: Vec<u8> },
    /// `TheScriptEngine->notifyOfCompletedSpecialPower(playerIndex, specialPowerName, sourceObject)` (`SpecialPowerCompletionDie`).
    ScriptNotifyOfCompletedSpecialPower { player: PlayerIndex, name: Vec<u8>, creator: ObjectId },
    /// `player->healAllObjects()` (`HealCrateCollide`).
    PlayerHealAllObjects { player: PlayerIndex },
    /// `ThePartitionManager->revealMapForPlayer(playerIndex)` (`ShroudCrateCollide`).
    PartitionRevealMapForPlayer { player: PlayerIndex },
    /// `player->getMoney()->deposit(amount)`, with the cash sound (`MoneyCrateCollide`).
    PlayerMoneyDeposit { player: PlayerIndex, amount: u32 },
    /// `player->getScoreKeeper()->addMoneyEarned(amount)` (`MoneyCrateCollide`).
    PlayerScoreAddMoneyEarned { player: PlayerIndex, amount: i32 },
    /// `TheScriptEngine->notifyOfTriggeredSpecialPower(playerIndex, specialPowerName, sourceObject)` (`SpecialPowerModule::aboutToDoSpecialPower`).
    ScriptNotifyOfTriggeredSpecialPower { player: PlayerIndex, name: Vec<u8>, source: ObjectId },
    /// `player->getAcademyStats()->recordSpecialPowerUsed(template)` (`SpecialPowerModule::initiateIntentToDoSpecialPower`), the template by id.
    PlayerAcademyRecordSpecialPowerUsed { player: PlayerIndex, template: u32 },
    /// `TheAnim2DCollection->findTemplate(name)` and `TheInGameUI->addWorldAnimation(template, pos, WORLD_ANIM_FADE_ON_EXPIRE, time, zRise)`
    /// (`CrateCollide::onCollide`).
    InGameUiAddWorldAnimation { template: Vec<u8>, position: crate::common::base_type::Coord3D, display_time_in_seconds: f32, z_rise_per_second: f32 },
    /// `player->setUnitsVisionSpied(setting, whichUnits, byWhom)` (`SpyVisionUpdate::doActivationWork`).
    PlayerSetUnitsVisionSpied { player: PlayerIndex, setting: bool, which_units: crate::common::kind_of::KindOfMaskType, by_whom: PlayerIndex },
    /// `TheWeaponStore->createAndFireTempWeapon(weapon, source, &position)` (`FireWeaponWhenDeadBehavior`), the weapon by its name in the store.
    WeaponCreateAndFireTemp { weapon: Vec<u8>, source: ObjectId, position: crate::common::base_type::Coord3D },
    /// `TheInGameUI->addFloatingText(text, &position, color)` with `text` the format `GUI:AddCash` given `amount` (`AutoDepositUpdate`).
    InGameUiAddFloatingText { amount: i32, position: crate::common::base_type::Coord3D, color: i32 },
    // --- end p1w11-sonnet ---
    // --- p1w12-sonnet ---
    /// `ObjectCreationList::create(ocl, primary, secondary, lifetimeFrames)` for the list named `ocl` (`FireOCLAfterWeaponCooldownUpdate`); like
    /// [`CutCall::OclCreate`], a name that is no list makes nothing.
    OclCreateWithLifetime { primary: ObjectId, secondary: ObjectId, ocl: Vec<u8>, lifetime_frames: u32 },
    /// `TheParticleSystemManager->createParticleSystem(tmpl)` and `system->setPosition(&position)`; `id` is the system's id (`AutoHealBehavior`).
    ParticleSystemCreate { template: Vec<u8>, position: crate::common::base_type::Coord3D, id: u32 },
    /// `TheAI->findClosestEnemy(obj, range, qualifiers)` (`CheckpointUpdate`); `ObjectCuts::ai_enemy_near` answers.
    /// `minor_radius` is the object's geometry at the call, which the search measures from.
    AiFindClosestEnemy { object: ObjectId, range: f32, qualifiers: u32, minor_radius: f32 },
    /// `TheAI->findClosestAlly(obj, range, qualifiers)` (`CheckpointUpdate`); `ObjectCuts::ai_ally_near` answers.
    AiFindClosestAlly { object: ObjectId, range: f32, qualifiers: u32, minor_radius: f32 },
    /// `ObjectCreationList::create(ocl, primary, &primaryPos, &secondaryPos, angle)` for the list named `ocl` (`OCLUpdate`); like [`CutCall::OclCreate`], a
    /// name that is no list makes nothing.
    OclCreateAt { primary: ObjectId, ocl: Vec<u8>, primary_pos: crate::common::base_type::Coord3D, secondary_pos: crate::common::base_type::Coord3D, angle: f32 },
    // --- end p1w12-sonnet ---
    // --- p1w13-sonnet ---
    /// `TheTerrainLogic->getGroundHeight(x, y)` (`HeightDieUpdate`); `ObjectCuts::terrain_ground` answers.
    TerrainGetGroundHeight { x: f32, y: f32 },
    /// `TheTerrainLogic->getHighestLayerForDestination(pos)` (`HeightDieUpdate`); `ObjectCuts::terrain_layer` answers.
    TerrainGetHighestLayerForDestination { x: f32, y: f32, z: f32 },
    /// `TheTerrainLogic->getLayerHeight(x, y, layer)` (`HeightDieUpdate`); `ObjectCuts::terrain_layer_height` answers.
    TerrainGetLayerHeight { x: f32, y: f32, layer: i32 },
    /// `ThePartitionManager->iterateObjectsInRange(obj, range, FROM_BOUNDINGSPHERE_3D, { AcceptByKindOf(STRUCTURE) })` (`HeightDieUpdate`);
    /// `ObjectCuts::range_scan` answers.
    PartitionIterateStructuresInRange { object: ObjectId, range: f32 },
    /// `TheParticleSystemManager->destroyAttachedSystems(obj)` (`HeightDieUpdate`).
    ParticleDestroyAttachedSystems { object: ObjectId },
    /// What `ActiveBody` did to a unit that is not translated (see `body_cuts`).
    Body(crate::logic::world::body_cuts::BodyCall),
    /// What a contain module did to a unit that is not translated (see `contain_cuts`).
    Contain(crate::logic::world::contain_cuts::ContainCall),
    /// `ThePartitionManager->getClosestObject(obj, range, FROM_CENTER_3D, filters)` (`FireSpreadUpdate`); `ObjectCuts::range_scan` answers with the candidates, nearest first.
    PartitionGetClosestObject { object: ObjectId, range: f32 },
    /// `TheTerrainLogic->isUnderwater(x, y, &waterZ)` (`FloatUpdate`); `ObjectCuts::terrain_has_water` and `terrain_water` answer.
    TerrainIsUnderwater { x: f32, y: f32 },
    // --- end p1w13-sonnet ---
    // ---- Team ----
    /// `team->notifyTeamOfObjectDeath()`.
    TeamNotifyTeamOfObjectDeath { team: TeamId },
    /// `team->setEnteredExited()`.
    TeamSetEnteredExited { team: TeamId },
    // ---- Energy ----
    /// `player->getEnergy()->objectEnteringInfluence(obj)`.
    EnergyObjectEnteringInfluence { player: PlayerIndex, object: ObjectId },
    /// `player->getEnergy()->objectLeavingInfluence(obj)`.
    EnergyObjectLeavingInfluence { player: PlayerIndex, object: ObjectId },
    /// `player->getEnergy()->adjustPower(amount, add)`.
    EnergyAdjustPower { player: PlayerIndex, amount: i32, add: bool },
    // ---- PartitionManager ----
    /// `ThePartitionManager->registerObject(obj)`.
    PartitionRegisterObject { object: ObjectId },
    /// `ThePartitionManager->unRegisterObject(obj)`.
    PartitionUnregisterObject { object: ObjectId },
    /// `m_partitionData->makeDirty(true)`.
    PartitionMakeDirty { object: ObjectId },
    /// `ThePartitionManager->doShroudReveal(x, y, range, mask)`.
    PartitionDoShroudReveal { x: f32, y: f32, range: f32, mask: PlayerMaskType },
    /// `ThePartitionManager->queueUndoShroudReveal(x, y, range, mask)`.
    PartitionQueueUndoShroudReveal { x: f32, y: f32, range: f32, mask: PlayerMaskType },
    /// `ThePartitionManager->doShroudCover(x, y, range, mask)`.
    PartitionDoShroudCover { x: f32, y: f32, range: f32, mask: PlayerMaskType },
    /// `ThePartitionManager->undoShroudCover(x, y, range, mask)`.
    PartitionUndoShroudCover { x: f32, y: f32, range: f32, mask: PlayerMaskType },
    /// `ThePartitionManager->doValueAffect(x, y, howFar, data, forWhom)`.
    PartitionDoValueAffect { x: f32, y: f32, how_far: f32, data: u32, for_whom: PlayerMaskType },
    /// `ThePartitionManager->undoValueAffect(x, y, howFar, data, forWhom)`.
    PartitionUndoValueAffect { x: f32, y: f32, how_far: f32, data: u32, for_whom: PlayerMaskType },
    /// `ThePartitionManager->doThreatAffect(x, y, howFar, data, forWhom)`.
    PartitionDoThreatAffect { x: f32, y: f32, how_far: f32, data: u32, for_whom: PlayerMaskType },
    /// `ThePartitionManager->undoThreatAffect(x, y, howFar, data, forWhom)`.
    PartitionUndoThreatAffect { x: f32, y: f32, how_far: f32, data: u32, for_whom: PlayerMaskType },
    // ---- AI and pathfinder ----
    /// `ai->setAttitude(team's initial attitude)`.
    AiSetAttitude { object: ObjectId, team: TeamId },
    /// `ai->setAttackInfo(info of the team's attack priority)`.
    AiSetAttackInfo { object: ObjectId, team: TeamId },
    /// `ai->aiIdle(CMD_FROM_AI)`.
    AiIdle { object: ObjectId },
    /// `ai->transferAttack(oldVictim, newVictim)`.
    AiTransferAttack { object: ObjectId, old_victim: ObjectId, new_victim: ObjectId },
    /// `group->remove(obj)` (`AIGroup`).
    AiGroupRemove { group: u32, object: ObjectId },
    /// `ai->setLocomotorGoalNone(); ai->destroyPath()` (`GameLogic::destroyObject`).
    AiStopForDestroy { object: ObjectId },
    /// `TheAI->pathfinder()->removeWallPiece(obj)`.
    PathfinderRemoveWallPiece { object: ObjectId },
    /// `dozerAI->cancelTask(dozerAI->getCurrentTask())`.
    DozerCancelTask { object: ObjectId },
    /// `TheAI->pathfinder()->updatePos(obj, pos)`.
    PathfinderUpdatePos { object: ObjectId },
    /// `TheAI->pathfinder()->removePos(obj)`.
    PathfinderRemovePos { object: ObjectId },
    /// `TheAI->pathfinder()->addObjectToPathfindMap(obj)`.
    PathfinderAddObjectToPathfindMap { object: ObjectId },
    /// `TheAI->pathfinder()->removeObjectFromPathfindMap(obj)`.
    PathfinderRemoveObjectFromPathfindMap { object: ObjectId },
    // ---- ScriptEngine, GameLogic, others ----
    /// `TheScriptEngine->notifyOfObjectCreationOrDestruction()`.
    ScriptEngineNotifyOfObjectCreationOrDestruction,
    /// `TheScriptEngine->notifyOfObjectDestruction(obj)`.
    ScriptEngineNotifyOfObjectDestruction { object: ObjectId },
    /// `TheScriptEngine->AppendDebugMessage("***WARNING - Too many nested trigger areas. ***", true)`.
    ScriptEngineAppendDebugMessage { too_many_trigger_areas: bool },
    /// `TheGameLogic->updateObjectsChangedTriggerAreas()`.
    GameLogicUpdateObjectsChangedTriggerAreas,
    /// `TheBuildAssistant->sellObject(obj)`.
    BuildAssistantSellObject { object: ObjectId },
    /// `ThePlayerList->getNeutralPlayer()->getAcademyStats()->recordMine()`.
    AcademyStatsRecordMine,
    /// `TheGameLogic->deselectObject(obj, playerMask, affectClient)`.
    GameLogicDeselectObject { object: ObjectId, player_mask: PlayerMaskType, affect_client: bool },
    // ---- WeaponSet and helpers ----
    /// `m_weaponSet.updateWeaponSet(obj)`.
    WeaponSetUpdate { object: ObjectId },
    /// `m_weaponSet.weaponSetOnWeaponBonusChange(obj)`.
    WeaponSetOnWeaponBonusChange { object: ObjectId },
    /// `helper->sleepUntil(when)` for that helper.
    HelperSleepUntil { object: ObjectId, kind: HelperKind, when: u32 },
}

/// The values of `TheGlobalData`, `TheAI->getAiData()`, `TheGameLogic` and
/// `TheScriptEngine` that `Object.cpp` reads. Filled by the game setup from
/// the real stores; the defaults are the headless run's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectGlobals {
    /// `TheGlobalData->m_gravity` (`GlobalData::m_gravity`, -1 by default).
    pub gravity: f32,
    /// `TheGlobalData->m_forceModelsToFollowTimeOfDay`.
    pub force_models_to_follow_time_of_day: bool,
    /// `TheGlobalData->m_timeOfDay == TIME_OF_DAY_NIGHT`.
    pub time_of_day_is_night: bool,
    /// `TheGlobalData->m_forceModelsToFollowWeather`.
    pub force_models_to_follow_weather: bool,
    /// `TheGlobalData->m_weather == WEATHER_SNOWY`.
    pub weather_is_snowy: bool,
    /// `TheGlobalData->m_levelGainAnimationName.isEmpty() == FALSE` (and the
    /// animation collection exists).
    pub level_gain_animation_name_set: bool,
    /// `TheAI->getAiData()->m_enableRepulsors`.
    pub enable_repulsors: bool,
    /// `TheGameLogic->getDrawIconUI()`.
    pub draw_icon_ui: bool,
    // --- p1w11-sonnet ---
    /// `TheAnim2DCollection != NULL` (`CrateCollide::onCollide` shows its execute animation only then).
    pub anim2d_collection_present: bool,
    // --- end p1w11-sonnet ---
    // --- p1w12-sonnet ---
    /// `TheGlobalData->m_getHealedAnimationDisplayTimeInSeconds`.
    pub get_healed_animation_display_time_in_seconds: f32,
    /// `TheGlobalData->m_getHealedAnimationZRisePerSecond`.
    pub get_healed_animation_z_rise_per_second: f32,
    // --- end p1w12-sonnet ---
    /// `TheGlobalData->m_maxTunnelCapacity` (`TunnelTracker::getContainMax`).
    pub max_tunnel_capacity: i32,
    /// `TheScriptEngine->getObjectsShouldReceiveDifficultyBonus()`.
    pub objects_should_receive_difficulty_bonus: bool,
    // --- p1w10-sonnet ---
    /// `TheGameLogic->getHulkMaxLifetimeOverride()` (-1: none; set by the script action).
    pub hulk_max_lifetime_override: i32,
    /// `TheGlobalData->m_baseRegenHealthPercentPerSecond` (`BaseRegenerateUpdate`).
    pub base_regen_health_percent_per_second: f32,
    /// `TheGlobalData->m_baseRegenDelay` (frames).
    pub base_regen_delay: u32,
    // --- p1w13-sonnet ---
    /// `TheGameLODManager->getSlowDeathScale()` (`SlowDeathBehavior`): 1 is the normal speed of a death, 0 an instant one.
    pub slow_death_scale: f32,
    // --- end p1w13-sonnet ---
    // --- end p1w10-sonnet ---
}

impl Default for ObjectGlobals {
    fn default() -> Self {
        ObjectGlobals {
            gravity: -1.0,
            force_models_to_follow_time_of_day: true,
            time_of_day_is_night: false,
            force_models_to_follow_weather: true,
            weather_is_snowy: false,
            level_gain_animation_name_set: false,
            enable_repulsors: false,
            draw_icon_ui: true,
            // --- p1w11-sonnet ---
            anim2d_collection_present: true,
            // --- end p1w11-sonnet ---
            // --- p1w12-sonnet ---
            get_healed_animation_display_time_in_seconds: 0.0,
            get_healed_animation_z_rise_per_second: 0.0,
            // --- end p1w12-sonnet ---
            max_tunnel_capacity: 0,
            objects_should_receive_difficulty_bonus: false,
            // --- p1w10-sonnet ---
            hulk_max_lifetime_override: -1,
            base_regen_health_percent_per_second: 0.0,
            base_regen_delay: 0,
            // --- end p1w10-sonnet ---
            // --- p1w13-sonnet ---
            slow_death_scale: 1.0,
            // --- end p1w13-sonnet ---
        }
    }
}

/// The tables and logs behind the cut points. See the module docs.
#[derive(Clone, Debug)]
pub struct ObjectCuts {
    /// When `Some`, every [`CutCall`] is appended.
    pub log: Option<Vec<CutCall>>,
    /// When `Some`, one line per cut call (`cut ...`), per `Object` client event (`evt ...`) and per call a probe
    /// module receives (`mod Name`), in the order they happen (the differential test compares it with the
    /// records of the compiled `Object.cpp`).
    pub trace: Option<Vec<String>>,
    /// The module calls that were **not** delivered because the module's body
    /// was out of its slot and had not lent itself (see `module_call.rs`,
    /// "Re-entrancy"). Upstream delivers every such call, so a non-empty list
    /// is a defect in a module that called into the world without lending
    /// itself: a tripwire, empty in every correct run.
    pub reentrant_skips: Vec<(ModuleHandle, ModuleCall)>,
    // --- p1w7-sonnet ---
    /// `TheActionManager->canTransferSuppliesAt(obj, transferObj)`, as the set of
    /// `(obj, transferObj)` pairs for which it is true, until `ActionManager` is translated
    /// (`ResourceGatheringManager` asks).
    pub can_transfer_supplies: BTreeSet<(ObjectId, ObjectId)>,
    // --- end p1w7-sonnet ---
    /// The module calls that ran **at once, inside a call to the same
    /// module** (the body lent itself, see `World::lend_body`): upstream's
    /// plain recursion, counted so a test can show that it was exercised.
    pub reentrant_delivered: u32,
    // --- p1w10-sonnet ---
    /// An explicit ablation: make the helpers of a new object as the stand-in of [`ObjectCuts::make_helper`] (a name and a tag,
    /// nothing else) instead of the translated modules. Off by default: an object has the helpers upstream gives it. The
    /// differential test of `Object.cpp` sets it, because its C++ side stands in for the helpers too.
    pub stub_helpers: bool,
    // --- end p1w10-sonnet ---
    /// The modules whose `module_call` is running, outermost first.
    pub module_stack: Vec<ModuleHandle>,
    // --- p1l9-example ---
    /// What upstream throws from inside the game loop and nothing catches (an
    /// upgrade module naming an upgrade that does not exist): recorded with the
    /// module instead of crashing. See `logic::modules::upgrade_body`.
    pub module_errors: Vec<(ModuleHandle, crate::common::ini::IniError)>,
    // --- end p1l9-example ---
    // --- p1w11-sonnet ---
    /// The classes `World::thing_factory_new_object` made as stubs because they are not translated (`ThingFactory::newObject`), in order.
    pub untranslated_modules: Vec<Vec<u8>>,
    /// A [`ModuleProbe`](crate::logic::world::module_probe::ProbeModule) writes the lines `pause`, `extendrods` and `extendradar` to `trace`
    /// (off by default: `differential_object` compares the probe's trace to the C++ log, which has no such lines).
    pub probe_effect_trace: bool,
    /// The function-local `static` of `MoneyCrateCollide::getUpgradedSupplyBoost`: `None` until control first reaches it, then the upgrade mask of
    /// the template the first pair named (`Some(None)` for a name that is no upgrade). See `modules::money_crate_collide`.
    pub money_crate_upgrade_static: Option<Option<crate::common::upgrade::UpgradeMaskType>>,
    /// The function-local `static` of `AutoDepositUpdate::getUpgradedSupplyBoost` (its own, not `MoneyCrateCollide`'s), the same way.
    pub auto_deposit_upgrade_static: Option<Option<crate::common::upgrade::UpgradeMaskType>>,
    /// `TheGlobalData->m_specialPowerViewObjectName` (`SpecialPowerModule::createViewObject`); empty for none.
    pub special_power_view_object_name: Vec<u8>,
    // --- end p1w11-sonnet ---
    /// The values of the global stores.
    pub globals: ObjectGlobals,
    /// `ThePlayerList->getNeutralPlayer()->getDefaultTeam()`.
    pub neutral_default_team: TeamId,
    /// `Team::getControllingPlayer()`.
    pub team_players: BTreeMap<TeamId, PlayerIndex>,
    /// `Team::getName()`.
    pub team_names: BTreeMap<TeamId, Vec<u8>>,
    /// `Player::getDefaultTeam()`.
    pub player_default_teams: BTreeMap<PlayerIndex, TeamId>,
    /// Players for which `isPlayerActive()` is false.
    pub inactive_players: BTreeSet<PlayerIndex>,
    /// Players for which `isPlayableSide()` is false.
    pub unplayable_players: BTreeSet<PlayerIndex>,
    /// Players for which `isSkirmishAIPlayer()` is true.
    pub skirmish_ai_players: BTreeSet<PlayerIndex>,
    /// `Team::getRelationship(that)`: (this team, that team) -> relationship.
    pub team_relationships: BTreeMap<(TeamId, TeamId), Relationship>,
    /// `Player::getRelationship(thatTeam)`: (player, team) -> relationship.
    pub player_relationships: BTreeMap<(PlayerIndex, TeamId), Relationship>,
    /// `Player::getPlayerMask()` (default `1 << index`).
    pub player_masks: BTreeMap<PlayerIndex, PlayerMaskType>,
    /// `Player::getPlayerColor()`.
    pub player_colors: BTreeMap<PlayerIndex, i32>,
    /// `Player::getPlayerNightColor()`.
    pub player_night_colors: BTreeMap<PlayerIndex, i32>,
    /// `Player::getCompletedUpgradeMask()`.
    pub player_upgrades: BTreeMap<PlayerIndex, UpgradeMaskType>,
    /// `Player::getNumBattlePlansActive()`.
    pub player_battle_plans: BTreeMap<PlayerIndex, i32>,
    /// `Player::getProductionVeterancyLevel(templateName)`: (player, template name) -> level.
    pub player_production_levels: BTreeMap<(PlayerIndex, Vec<u8>), VeterancyLevel>,
    /// The object lists of the teams (`DLINK` `TeamMemberList`), newest first.
    pub team_members: BTreeMap<TeamId, Vec<ObjectId>>,
    /// Teams whose prototype has a template info (`proto->getTemplateInfo()`).
    pub teams_with_template_info: BTreeSet<TeamId>,
    /// Teams whose attack priority names a known attack info.
    pub teams_with_attack_info: BTreeSet<TeamId>,
    /// `TheUpgradeCenter->findVeterancyUpgrade(level)`'s mask, by level.
    pub veterancy_upgrades: [Option<UpgradeMaskType>; 4],
    /// `TheSpecialPowerStore->canUseSpecialPower` answers `false` for these objects.
    pub special_power_denied: BTreeSet<ObjectId>,
    /// `weapon != NULL` per (object, slot).
    pub weapons: BTreeSet<(ObjectId, i32)>,
    /// `m_weaponSet.hasAnyWeapon()` per object.
    pub weapon_sets_with_weapons: BTreeSet<ObjectId>,
    // --- p1w12-sonnet ---
    /// The facts the modules read of an object's current weapon (`getCurrentWeapon()`), until the weapon set holds real weapons: see
    /// [`World::current_weapon_facts`].
    pub cur_weapon: BTreeMap<ObjectId, WeaponFacts>,
    /// What `ThePartitionManager->iterateObjectsInRange(..)` returns for a module that scans around its object (`AutoHealBehavior`'s allies in a
    /// radius), in the order it is visited: the filters are the partition manager's.
    pub range_scan: Vec<ObjectId>,
    /// `Player::getPlayerTemplate()->getSide()` per player (`None`: no template).
    pub player_sides: BTreeMap<PlayerIndex, Vec<u8>>,
    /// What `TheTerrainLogic->findClosestEdgePoint(pos)` adds to `pos` until the layer-12 adapter plugs the terrain in.
    pub edge_point_offset: crate::common::base_type::Coord3D,
    /// The handle of the last audio event added (`AudioManager::addAudioEvent` hands out 1, 2, 3, ...).
    pub last_audio_handle: u32,
    /// The objects for which `TheAI->findClosestEnemy` finds an enemy.
    pub ai_enemy_near: BTreeSet<ObjectId>,
    /// The objects for which `TheAI->findClosestAlly` finds an ally.
    pub ai_ally_near: BTreeSet<ObjectId>,
    /// `TheGlobalData->m_getHealedAnimationName` (`AutoHealBehavior`'s icon).
    pub get_healed_animation_name: Vec<u8>,
    /// `Player::iterateObjects` order, per player.
    pub player_object_list: BTreeMap<PlayerIndex, Vec<ObjectId>>,
    /// `ParticleSystemManager::m_uniqueSystemID`: the id of the last system made (ids run 1, 2, 3, ...).
    pub last_particle_system_id: u32,
    // --- end p1w12-sonnet ---
    // --- p1w13-sonnet ---
    /// The ground height `TheTerrainLogic->getGroundHeight(x, y)` answers, by [`terrain_cell`] of (x, y), until the layer-12 adapter plugs the terrain in.
    pub terrain_ground: [f32; 8],
    /// The layer `getHighestLayerForDestination(pos)` answers, by [`terrain_cell`] of (x, y).
    pub terrain_layer: [i32; 8],
    /// The height `getLayerHeight(x, y, layer)` answers, by [`terrain_cell`] of (x, y).
    pub terrain_layer_height: [f32; 8],
    /// Whether `isUnderwater(x, y, &waterZ)` finds water (and so assigns `waterZ`), by [`terrain_cell`] of (x, y).
    pub terrain_has_water: [bool; 8],
    /// The water height `isUnderwater` assigns, by [`terrain_cell`] of (x, y).
    pub terrain_water: [f32; 8],
    /// The cut points of `ActiveBody`: see `body_cuts`.
    pub body: crate::logic::world::body_cuts::BodyCutTables,
    /// The cut points of the contain modules: see `contain_cuts`.
    pub contain: crate::logic::world::contain_cuts::ContainCutTables,
    // --- end p1w13-sonnet ---
    /// `ai->getWhichTurretForWeaponSlot(slot)` != `TURRET_INVALID`: (object, slot) -> turret.
    pub ai_turrets: BTreeMap<(ObjectId, i32), i32>,
    /// `ai->isTurretEnabled(turret)` is false for these (object, turret).
    pub ai_turrets_disabled: BTreeSet<(ObjectId, i32)>,
    /// What `iteratePotentialCollisions(pos, geom, angle)` returns, in order.
    pub potential_collisions: Vec<ObjectId>,
    /// What `iterateObjectsInRange(..)` with the booby-trap filters returns, nearest first.
    pub booby_traps_in_range: Vec<ObjectId>,
    /// Objects whose AI has a dozer interface (`ai->getDozerAIInterface()` is non-null; its current task is a real one).
    pub dozer_ais: BTreeSet<ObjectId>,
    /// Objects whose AI's current locomotor flies (`getLegalSurfaces() & LOCOMOTORSURFACE_AIR`).
    pub ai_airborne: BTreeSet<ObjectId>,
    /// `ai->getTurretRotAndPitch(turret)`'s rotation: (object, turret) -> radians.
    pub ai_turret_rotation: BTreeMap<(ObjectId, i32), f32>,
    /// `m_partitionData->getShroudedStatus(playerIndex)`: (object, player) -> status.
    pub shrouded_status: BTreeMap<(ObjectId, i32), ObjectShroudStatus>,
    /// `pathfinder->getWallHeight()`, for `getLayerHeight` on the wall layer.
    pub wall_height: f32,
    /// The client's drawable id counter (`GameClient::allocateNewDrawableID`).
    next_drawable_id: DrawableId,
}

impl Default for ObjectCuts {
    fn default() -> Self {
        ObjectCuts {
            log: None,
            trace: None,
            reentrant_skips: Vec::new(),
            reentrant_delivered: 0,
            // --- p1w10-sonnet ---
            stub_helpers: false,
            // --- end p1w10-sonnet ---
            can_transfer_supplies: BTreeSet::new(),
            module_stack: Vec::new(),
            // --- p1l9-example ---
            module_errors: Vec::new(),
            // --- p1w11-sonnet ---
            untranslated_modules: Vec::new(),
            probe_effect_trace: false,
            money_crate_upgrade_static: None,
            auto_deposit_upgrade_static: None,
            special_power_view_object_name: Vec::new(),
            // --- end p1w11-sonnet ---
            // --- end p1l9-example ---
            globals: ObjectGlobals::default(),
            neutral_default_team: 0,
            team_players: BTreeMap::new(),
            team_names: BTreeMap::new(),
            player_default_teams: BTreeMap::new(),
            inactive_players: BTreeSet::new(),
            unplayable_players: BTreeSet::new(),
            skirmish_ai_players: BTreeSet::new(),
            team_relationships: BTreeMap::new(),
            player_relationships: BTreeMap::new(),
            player_masks: BTreeMap::new(),
            player_colors: BTreeMap::new(),
            player_night_colors: BTreeMap::new(),
            player_upgrades: BTreeMap::new(),
            player_battle_plans: BTreeMap::new(),
            player_production_levels: BTreeMap::new(),
            team_members: BTreeMap::new(),
            teams_with_template_info: BTreeSet::new(),
            teams_with_attack_info: BTreeSet::new(),
            veterancy_upgrades: [None; 4],
            special_power_denied: BTreeSet::new(),
            weapons: BTreeSet::new(),
            weapon_sets_with_weapons: BTreeSet::new(),
            // --- p1w12-sonnet ---
            cur_weapon: BTreeMap::new(),
            range_scan: Vec::new(),
            player_sides: BTreeMap::new(),
            edge_point_offset: crate::common::base_type::Coord3D { x: 0.0, y: 0.0, z: 0.0 },
            last_audio_handle: 0,
            ai_enemy_near: BTreeSet::new(),
            ai_ally_near: BTreeSet::new(),
            get_healed_animation_name: Vec::new(),
            player_object_list: BTreeMap::new(),
            last_particle_system_id: 0,
            // --- end p1w12-sonnet ---
            // --- p1w13-sonnet ---
            terrain_ground: [0.0; 8],
            terrain_layer: [crate::common::game_type::LAYER_GROUND; 8],
            terrain_layer_height: [0.0; 8],
            terrain_has_water: [false; 8],
            terrain_water: [0.0; 8],
            body: Default::default(),
            contain: Default::default(),
            // --- end p1w13-sonnet ---
            ai_turrets: BTreeMap::new(),
            ai_turrets_disabled: BTreeSet::new(),
            dozer_ais: BTreeSet::new(),
            ai_airborne: BTreeSet::new(),
            ai_turret_rotation: BTreeMap::new(),
            shrouded_status: BTreeMap::new(),
            potential_collisions: Vec::new(),
            booby_traps_in_range: Vec::new(),
            wall_height: 0.0,
            next_drawable_id: 1,
        }
    }
}

impl ObjectCuts {
    /// `Team::getControllingPlayer()`: `None` for a team no player controls.
    pub fn team_player(&self, team: TeamId) -> Option<PlayerIndex> {
        self.team_players.get(&team).copied()
    }

    /// `Team::getName()`: empty if unknown.
    pub fn team_name(&self, team: TeamId) -> Vec<u8> {
        self.team_names.get(&team).cloned().unwrap_or_default()
    }

    /// `TheTeamFactory->findTeam(name)`: the first team (lowest id) with that name.
    pub fn find_team_by_name(&self, name: &[u8]) -> Option<TeamId> {
        self.team_names.iter().find(|(_, n)| n.as_slice() == name).map(|(t, _)| *t)
    }

    /// `Player::isPlayerActive()`.
    pub fn player_is_active(&self, p: PlayerIndex) -> bool {
        !self.inactive_players.contains(&p)
    }

    /// `Player::isPlayableSide()`.
    pub fn player_is_playable_side(&self, p: PlayerIndex) -> bool {
        !self.unplayable_players.contains(&p)
    }

    /// `Player::isSkirmishAIPlayer()`.
    pub fn player_is_skirmish_ai(&self, p: PlayerIndex) -> bool {
        self.skirmish_ai_players.contains(&p)
    }

    /// `Player::getDefaultTeam()`: 0 if unset.
    pub fn player_default_team(&self, p: PlayerIndex) -> TeamId {
        self.player_default_teams.get(&p).copied().unwrap_or(0)
    }

    /// `Team::getRelationship(that)`: `NEUTRAL` unless set. `that` of `None` is `NULL`.
    pub fn team_relationship(&self, team: TeamId, that: Option<TeamId>) -> Relationship {
        match that {
            Some(that) => self.team_relationships.get(&(team, that)).copied().unwrap_or(Relationship::Neutral),
            None => Relationship::Neutral,
        }
    }

    /// `Player::getRelationship(thatTeam)`: `NEUTRAL` unless set; `None` is `NULL`.
    pub fn player_relationship_to_team(&self, p: PlayerIndex, that: Option<TeamId>) -> Relationship {
        match that {
            Some(that) => self.player_relationships.get(&(p, that)).copied().unwrap_or(Relationship::Neutral),
            None => Relationship::Neutral,
        }
    }

    /// `p->getRelationship(q->getDefaultTeam())`.
    pub fn player_relationship_to_player_default_team(&self, p: PlayerIndex, q: PlayerIndex) -> Relationship {
        let team = self.player_default_team(q);
        self.player_relationship_to_team(p, if team != 0 { Some(team) } else { None })
    }

    /// `Player::getPlayerMask()`: `1 << index` unless set.
    pub fn player_mask(&self, p: PlayerIndex) -> PlayerMaskType {
        self.player_masks.get(&p).copied().unwrap_or_else(|| 1u16.checked_shl(p as u32).unwrap_or(0))
    }

    /// `Player::getPlayerColor()`.
    pub fn player_color(&self, p: PlayerIndex) -> i32 {
        self.player_colors.get(&p).copied().unwrap_or(0)
    }

    /// `Player::getPlayerNightColor()`.
    pub fn player_night_color(&self, p: PlayerIndex) -> i32 {
        self.player_night_colors.get(&p).copied().unwrap_or(0)
    }

    /// `Player::getPlayerTemplate()->getSide()`; `None` for no template.
    pub fn player_side(&self, p: PlayerIndex) -> Option<Vec<u8>> {
        self.player_sides.get(&p).cloned()
    }

    /// `Player::getCompletedUpgradeMask()`.
    pub fn player_completed_upgrade_mask(&self, p: PlayerIndex) -> UpgradeMaskType {
        self.player_upgrades.get(&p).copied().unwrap_or_default()
    }

    /// `Player::getNumBattlePlansActive()`.
    pub fn player_num_battle_plans_active(&self, p: PlayerIndex) -> i32 {
        self.player_battle_plans.get(&p).copied().unwrap_or(0)
    }

    /// `Player::getProductionVeterancyLevel(templateName)`.
    pub fn player_production_veterancy_level(&self, p: PlayerIndex, template_name: &[u8]) -> VeterancyLevel {
        self.player_production_levels.get(&(p, template_name.to_vec())).copied().unwrap_or(VeterancyLevel::Regular)
    }

    /// `team->isInList_TeamMemberList(obj)`.
    pub fn team_member_list_contains(&self, team: TeamId, obj: ObjectId) -> bool {
        self.team_members.get(&team).is_some_and(|l| l.contains(&obj))
    }

    /// `team->removeFrom_TeamMemberList(obj)`.
    pub fn team_member_list_remove(&mut self, team: TeamId, obj: ObjectId) {
        if let Some(l) = self.team_members.get_mut(&team) {
            l.retain(|&o| o != obj);
        }
    }

    /// `team->prependTo_TeamMemberList(obj)`: the list runs newest first.
    pub fn team_member_list_prepend(&mut self, team: TeamId, obj: ObjectId) {
        self.team_members.entry(team).or_default().insert(0, obj);
    }

    /// `proto && proto->getTemplateInfo()`.
    pub fn team_has_template_info(&self, team: TeamId) -> bool {
        self.teams_with_template_info.contains(&team)
    }

    /// The team prototype's attack priority names an existing attack info with a name.
    pub fn team_attack_info(&self, team: TeamId) -> bool {
        self.teams_with_attack_info.contains(&team)
    }

    /// `TheUpgradeCenter->findVeterancyUpgrade(level)`'s mask.
    pub fn veterancy_upgrade(&self, level: VeterancyLevel) -> Option<UpgradeMaskType> {
        self.veterancy_upgrades[level as usize]
    }

    /// `TheSpecialPowerStore->canUseSpecialPower(obj, template)`.
    pub fn special_power_can_use(&self, obj: ObjectId, _template: Option<u32>) -> bool {
        !self.special_power_denied.contains(&obj)
    }

    /// `getWeaponInWeaponSlot(slot) != NULL`.
    pub fn weapon_in_slot(&self, obj: ObjectId, slot: i32) -> bool {
        self.weapons.contains(&(obj, slot))
    }

    /// `m_weaponSet.hasAnyWeapon()`.
    pub fn weapon_set_has_any_weapon(&self, obj: ObjectId) -> bool {
        self.weapon_sets_with_weapons.contains(&obj)
    }

    /// `ai->getWhichTurretForWeaponSlot(slot)`: `None` is `TURRET_INVALID`.
    pub fn ai_turret_for_slot(&self, obj: ObjectId, slot: i32) -> Option<i32> {
        self.ai_turrets.get(&(obj, slot)).copied()
    }

    /// `ai->isTurretEnabled(turret)`.
    pub fn ai_turret_enabled(&self, obj: ObjectId, turret: i32) -> bool {
        !self.ai_turrets_disabled.contains(&(obj, turret))
    }

    /// `ai->getTurretRotAndPitch(turret)`'s rotation (0 if unset).
    pub fn ai_turret_rotation(&self, obj: ObjectId, turret: i32) -> f32 {
        self.ai_turret_rotation.get(&(obj, turret)).copied().unwrap_or(0.0)
    }

    /// `GameClient::allocateNewDrawableID()`: 1, 2, 3, ...
    pub fn allocate_drawable_id(&mut self) -> DrawableId {
        let id = self.next_drawable_id;
        self.next_drawable_id = self.next_drawable_id.wrapping_add(1);
        if id == INVALID_DRAWABLE_ID {
            return self.allocate_drawable_id();
        }
        id
    }

    /// Makes the helper of that kind: until layer 9 translates the helpers, a
    /// stub with no behaviour and no update interface.
    pub fn make_helper(&self, kind: HelperKind) -> NewModule {
        NewModule::helper_stub(kind)
    }

    /// `ThePlayerList->getPlayersWithRelationship(src, allowed)` for the
    /// players `0..player_count`.
    pub fn players_with_relationship(&self, src: PlayerIndex, player_count: i32, allowed: u32) -> PlayerMaskType {
        let mut ret: PlayerMaskType = 0;
        if allowed == 0 {
            return ret;
        }
        if allowed & ALLOW_SAME_PLAYER != 0 {
            ret |= self.player_mask(src);
        }
        for i in 0..player_count {
            if i == src {
                continue;
            }
            match self.player_relationship_to_player_default_team(src, i) {
                Relationship::Enemies => {
                    if allowed & ALLOW_ENEMIES != 0 {
                        ret |= self.player_mask(i);
                    }
                }
                Relationship::Allies => {
                    if allowed & ALLOW_ALLIES != 0 {
                        ret |= self.player_mask(i);
                    }
                }
                Relationship::Neutral => {
                    if allowed & ALLOW_NEUTRAL != 0 {
                        ret |= self.player_mask(i);
                    }
                }
            }
        }
        ret
    }
}

impl World {
    /// Records an action cut point (when logging is on).
    pub fn cut(&mut self, call: CutCall) {
        if let Some(t) = &mut self.cuts.trace {
            t.push(format!("cut {call:?}"));
        }
        if let Some(log) = &mut self.cuts.log {
            log.push(call);
        }
    }

    /// Sends one `Object` notification to the client.
    pub fn emit(&mut self, e: ObjectClientEvent) {
        if let Some(t) = &mut self.cuts.trace {
            t.push(format!("evt {e:?}"));
        }
        self.events.push(ClientEvent::Object(e));
    }

    /// `ThePlayerList->getPlayersWithRelationship(src, ALLOW_ENEMIES | ALLOW_NEUTRAL)`
    /// and its variants, with the list's current player count.
    pub fn cut_players_with_relationship(&self, src: PlayerIndex, enemies: bool, neutral: bool) -> PlayerMaskType {
        let mut allowed = 0;
        if enemies {
            allowed |= ALLOW_ENEMIES;
        }
        if neutral {
            allowed |= ALLOW_NEUTRAL;
        }
        self.cuts.players_with_relationship(src, self.players.player_count(), allowed)
    }

    /// `ThePartitionManager->iteratePotentialCollisions(..)` (scripted).
    pub fn cut_partition_potential_collisions(&self, _obj: ObjectId) -> Vec<ObjectId> {
        self.cuts.potential_collisions.clone()
    }

    /// `ThePartitionManager->iterateObjectsInRange(pos, radius, FROM_CENTER_2D,
    /// {booby trap kind, same map status}, ITER_SORTED_NEAR_TO_FAR)` (scripted).
    pub fn cut_partition_booby_traps_in_range(&self, _obj: ObjectId, _radius: f32) -> Vec<ObjectId> {
        self.cuts.booby_traps_in_range.clone()
    }

    /// `TheSpecialPowerStore->canUseSpecialPower(this, template)`.
    pub fn cut_special_power_can_use(&self, obj: ObjectId, template: Option<u32>) -> bool {
        self.cuts.special_power_can_use(obj, template)
    }

    // ---- ExperienceTracker (default copies of its logic) -------------------------------------------

    /// `ExperienceTracker::isAcceptingExperiencePoints()`: trainable, or has a sink.
    pub fn cut_experience_is_accepting_points(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.template().is_trainable() || o.experience.sink != INVALID_ID)
    }

    /// `ExperienceTracker::getExperienceValue(killer)`: "no experience for
    /// killing an ally, cheater"; else what the template says for the
    /// victim's level.
    pub fn cut_experience_value(&self, victim: ObjectId, killer: ObjectId) -> i32 {
        if self.get_relationship(killer, victim) == Relationship::Allies {
            return 0;
        }
        self.objects.find(victim).map_or(0, |o| o.template().get_experience_value(o.experience.level))
    }

    /// `ExperienceTracker::setVeterancyLevel(newLevel, provideFeedback)`.
    pub fn cut_experience_set_veterancy_level(&mut self, id: ObjectId, new_level: VeterancyLevel, provide_feedback: bool) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if o.experience.level != new_level {
            let old_level = o.experience.level;
            o.experience.level = new_level;
            o.experience.points = o.template().get_experience_required(new_level); // minimum for this level
            self.on_veterancy_level_changed(id, old_level, new_level, provide_feedback);
        }
    }

    /// `ExperienceTracker::setExperienceAndLevel(experienceIn, provideFeedback)`.
    pub fn cut_experience_set_experience_and_level(&mut self, id: ObjectId, experience_in: i32, provide_feedback: bool) {
        let Some(o) = self.objects.find(id) else { return };
        let sink = o.experience.sink;
        if sink != INVALID_ID && self.objects.find(sink).is_some() {
            // I have been set up to give my experience to someone else
            self.cut_experience_set_experience_and_level(sink, experience_in, provide_feedback);
            return;
        }
        let o = self.objects.find_mut(id).expect("checked above");
        if !o.template().is_trainable() {
            return; // safety
        }
        let old_level = o.experience.level;
        o.experience.points = experience_in;
        let mut level_index = 0;
        while (level_index + 1) < crate::common::game_common::LEVEL_COUNT
            && o.experience.points >= o.template().get_experience_required(crate::logic::world::object::veterancy_from_i32(level_index + 1))
        {
            // If there is a level to qualify for, and I qualify for it, advance the index
            level_index += 1;
        }
        let new_level = crate::logic::world::object::veterancy_from_i32(level_index);
        o.experience.level = new_level;
        if old_level != new_level {
            // Edge trigger special level gain effects. (paradox! this may be a level lost!)
            self.on_veterancy_level_changed(id, old_level, new_level, provide_feedback);
        }
    }

    /// `ExperienceTracker::addExperiencePoints(experienceGain, canScaleForBonus)`.
    pub fn cut_experience_add_points(&mut self, id: ObjectId, experience_gain: i32, can_scale_for_bonus: bool) {
        let Some(o) = self.objects.find(id) else { return };
        let sink = o.experience.sink;
        let scalar = o.experience.scalar;
        if sink != INVALID_ID && self.objects.find(sink).is_some() {
            // I have been set up to give my experience to someone else
            // (`experienceGain * m_experienceScalar` is a Real, converted to the Int parameter)
            self.cut_experience_add_points(sink, (experience_gain as f32 * scalar) as i32, can_scale_for_bonus);
            return;
        }
        let o = self.objects.find_mut(id).expect("checked above");
        if !o.template().is_trainable() {
            return; // safety
        }
        let old_level = o.experience.level;
        let mut amount_to_gain = experience_gain;
        if can_scale_for_bonus {
            amount_to_gain = (amount_to_gain as f32 * scalar) as i32;
        }
        o.experience.points = o.experience.points.wrapping_add(amount_to_gain);
        let mut level_index = 0;
        while (level_index + 1) < crate::common::game_common::LEVEL_COUNT
            && o.experience.points >= o.template().get_experience_required(crate::logic::world::object::veterancy_from_i32(level_index + 1))
        {
            // If there is a higher level to qualify for, and I qualify for it, advance the index
            level_index += 1;
        }
        let new_level = crate::logic::world::object::veterancy_from_i32(level_index);
        o.experience.level = new_level;
        if old_level != new_level {
            // Edge trigger special level gain effects.
            self.on_veterancy_level_changed(id, old_level, new_level, true);
        }
    }
}

// --- p1w7-sonnet ---
impl World {
    /// `TheActionManager->canTransferSuppliesAt(obj, transferObj)`: a table of the world (see
    /// [`ObjectCuts::can_transfer_supplies`]).
    pub fn can_transfer_supplies_at(&self, obj: ObjectId, transfer_obj: ObjectId) -> bool {
        self.cuts.can_transfer_supplies.contains(&(obj, transfer_obj))
    }
}
// --- end p1w7-sonnet ---
