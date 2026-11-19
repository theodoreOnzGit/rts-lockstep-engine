//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_hosts.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/ChinookAIUpdate.cpp
//!   (ChinookAIUpdate::private___TellPortableStructureToAttackWithMe)

use crate::common::game_common::CommandSourceType;
use crate::common::game_type::ObjectId;
use crate::logic::modules::plant;

/// What the function asks of the world.
pub trait ChinookPortableStructureHost {
    /// `getObject()->getContain()->friend_getRider()`: the rider of the chinook's contain, `None` if there is no
    /// contain or no rider.
    fn contain_rider(&self, chinook: ObjectId) -> Option<ObjectId>;
    /// `rider->isKindOf(KINDOF_PORTABLE_STRUCTURE)`.
    fn is_portable_structure(&self, rider: ObjectId) -> bool;
    /// `rider->isDisabledByType(DISABLED_HACKED)`, `DISABLED_EMP`, `DISABLED_SUBDUED`, `DISABLED_PARALYZED`, in that order.
    fn is_disabled_hacked(&self, rider: ObjectId) -> bool;
    fn is_disabled_emp(&self, rider: ObjectId) -> bool;
    fn is_disabled_subdued(&self, rider: ObjectId) -> bool;
    fn is_disabled_paralyzed(&self, rider: ObjectId) -> bool;
    /// `riderAI->aiAttackObject(victim, maxShotsToFire, cmdSource)` if the rider has an AI; false if it has none.
    fn rider_ai_attack_object(&mut self, rider: ObjectId, victim: ObjectId, max_shots_to_fire: i32, cmd_source: CommandSourceType) -> bool;
}

/// `ChinookAIUpdate::private___TellPortableStructureToAttackWithMe(victim, maxShotsToFire, cmdSource)`
/// (ChinookAIUpdate.cpp:1490): "the gattling upgrade or the guys in the bunker_not_a_bunker": the portable structure
/// riding the chinook, if it is not hacked, EMP'd, subdued or paralyzed, is told to attack the victim.
pub fn private_tell_portable_structure_to_attack_with_me<H: ChinookPortableStructureHost>(
    h: &mut H,
    chinook: ObjectId,
    victim: ObjectId,
    max_shots_to_fire: i32,
    cmd_source: CommandSourceType,
) {
    let Some(rider) = h.contain_rider(chinook) else { return };
    if (plant(15726) || h.is_portable_structure(rider))
        && (plant(15724) || !h.is_disabled_hacked(rider))
        && !h.is_disabled_emp(rider)
        && !h.is_disabled_subdued(rider)
        && (plant(15723) || !h.is_disabled_paralyzed(rider))
    {
        if plant(15727) {
            let _ = h.rider_ai_attack_object(rider, max_shots_to_fire, victim, cmd_source);
        } else if plant(15728) {
            let _ = h.rider_ai_attack_object(rider, victim, max_shots_to_fire, CommandSourceType::FromAi);
        } else {
            let _ = h.rider_ai_attack_object(rider, victim, max_shots_to_fire, cmd_source);
        }
    }
}
