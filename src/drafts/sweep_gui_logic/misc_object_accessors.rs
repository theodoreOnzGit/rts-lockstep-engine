//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/RailroadGuideAIUpdate.cpp (TrainTrack::incReference,
//!   releaseReference)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/HackInternetAIUpdate.cpp (isHacking)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/WorkerAIUpdate.cpp (getRepairHealthPerSecond, getBoredTime)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/DozerAIUpdate.cpp (getRepairHealthPerSecond, getBoredTime)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/JetAIUpdate.cpp (friend_setAllowAirLoco,
//!   friend_addWaypointToGoalPath)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ParticleUplinkCannonUpdate.cpp (createGroundHitParticleSystem)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/GhostObject.cpp (GhostObjectManager::updateOrphanedObjects)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SalvageCrateCollide.cpp (doLevelGain)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/CreateCrateDie.cpp (testVeterancyLevel)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp (getLastVictimID)
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/SpecialPowerModule.cpp (getPowerName)
//!
//! Small accessors that the verified port reads as fields; named here so that each upstream function has a Rust one.
//! (One .cpp would be one .rs; these share a file because each is a line or two. The module split is the sweep's.)

use crate::common::game_common::VeterancyLevel;
use crate::common::game_type::{ObjectId, INVALID_ID};

/// `TrainTrack`'s reference count (`m_refCount`). The verified port needs none: `TrainTrack` is shared by `Arc`
/// (`crate::logic::modules::railroad_behavior::TrainTrack`), whose count is the reference count; drafted as the record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrainTrackRefCount {
    pub ref_count: i32,
}

impl TrainTrackRefCount {
    /// `TrainTrack::incReference()` (RailroadGuideAIUpdate.cpp:73): `++m_refCount`.
    pub fn inc_reference(&mut self) {
        self.ref_count += 1;
    }
    /// `TrainTrack::releaseReference()` (RailroadGuideAIUpdate.cpp:78): `return ( --m_refCount == 0 )`, true when the
    /// last reference went.
    pub fn release_reference(&mut self) -> bool {
        self.ref_count -= 1;
        self.ref_count == 0
    }
}

/// `HackInternetAIUpdate::isHacking()` (HackInternetAIUpdate.cpp:84): the state machine is in state `HACK_INTERNET`.
pub fn is_hacking(current_state_id: u32, hack_internet_state_id: u32) -> bool {
    current_state_id == hack_internet_state_id
}

/// `WorkerAIUpdate::getRepairHealthPerSecond()` (WorkerAIUpdate.cpp:171) and `DozerAIUpdate::getRepairHealthPerSecond()`
/// (DozerAIUpdate.cpp:2319): the module data's `m_repairHealthPercentPerSecond`.
pub fn get_repair_health_per_second(repair_health_percent_per_second: f32) -> f32 {
    repair_health_percent_per_second
}

/// `WorkerAIUpdate::getBoredTime()` (WorkerAIUpdate.cpp:176) and `DozerAIUpdate::getBoredTime()` (DozerAIUpdate.cpp:2324):
/// the module data's `m_boredTime`.
pub fn get_bored_time(bored_time: f32) -> f32 {
    bored_time
}

/// `JetAIUpdate::friend_setAllowAirLoco(allowAirLoco)` (JetAIUpdate.cpp:2489): `setFlag(ALLOW_AIR_LOCO, allowAirLoco)`.
/// `flags` is the jet's flag word, `allow_air_loco_bit` `crate::logic::ai::jet_ai_update::ALLOW_AIR_LOCO`.
pub fn friend_set_allow_air_loco(flags: &mut u32, allow_air_loco_bit: u32, allow_air_loco: bool) {
    if allow_air_loco {
        *flags |= allow_air_loco_bit;
    } else {
        *flags &= !allow_air_loco_bit;
    }
}

/// `JetAIUpdate::friend_addWaypointToGoalPath(bestPos)` (JetAIUpdate.cpp:2518): `privateFollowPathAppend(&bestPos,
/// CMD_FROM_AI)`; the verified port's is `crate::logic::ai::ai_commands::private_follow_path_append`.
pub fn friend_add_waypoint_to_goal_path<F: FnOnce(&crate::common::base_type::Coord3D)>(follow_path_append: F, best_pos: &crate::common::base_type::Coord3D) {
    follow_path_append(best_pos);
}

/// `ParticleUplinkCannonUpdate::createGroundHitParticleSystem(intensity)` (ParticleUplinkCannonUpdate.cpp:1015): the
/// body is empty upstream.
pub fn create_ground_hit_particle_system(_intensity: i32) {}

/// `GhostObjectManager::updateOrphanedObjects(playerIndexList, numNonLocalPlayers)` (GhostObject.cpp:176): the body is
/// empty upstream.
pub fn update_orphaned_objects(_player_index_list: &[i32], _num_non_local_players: i32) {}

/// `SalvageCrateCollide::doLevelGain(other)` (SalvageCrateCollide.cpp:230): `other->getExperienceTracker()->
/// gainExpForLevel(1)`; the verified port's is `crate::logic::experience_tracker::gain_exp_for_level` with 1 level.
pub fn do_level_gain<F: FnOnce(ObjectId, i32)>(gain_exp_for_level: F, other: ObjectId) {
    gain_exp_for_level(other, 1);
}

/// `CreateCrateDie::testVeterancyLevel(currentCrateData)` (CreateCrateDie.cpp:137): the crate's `m_veterancyLevel`
/// equals the dying object's.
pub fn test_veterancy_level(crate_veterancy_level: VeterancyLevel, object_veterancy_level: VeterancyLevel) -> bool {
    crate_veterancy_level == object_veterancy_level
}

/// `Object::getLastVictimID()` (Object.cpp:1564): `m_firingTracker ? m_firingTracker->getLastShotVictim() :
/// INVALID_ID`.
pub fn get_last_victim_id(last_shot_victim_of_firing_tracker: Option<ObjectId>) -> ObjectId {
    last_shot_victim_of_firing_tracker.unwrap_or(INVALID_ID)
}

/// `SpecialPowerModule::getPowerName()` (SpecialPowerModule.cpp:257): the name of the module data's special power
/// template, `getSpecialPowerModuleData()->m_specialPowerTemplate->getName()`.
pub fn get_power_name(special_power_template_name: &[u8]) -> Vec<u8> {
    special_power_template_name.to_vec()
}
