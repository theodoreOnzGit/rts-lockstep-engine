//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_hosts.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/BattlePlanUpdate.cpp (BattlePlanUpdate::createVisionObject)
//!
//! The verified `crate::logic::modules::battle_plan_update` keeps `vision_object_id` (`m_visionObjectID`) and the
//! module data's `vision_object_name`, and does not create the object. This draft is the missing function; it replaces
//! nothing (there is no stub), its caller `BattlePlanUpdate::setStatus` is to call it.

use crate::common::base_type::Coord3D;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::logic::modules::plant;

/// What `createVisionObject` asks of the world.
pub trait BattlePlanVisionHost {
    /// `TheThingFactory->findTemplate(name)`: the template's id, `None` if there is none.
    fn find_template(&mut self, name: &[u8]) -> Option<u16>;
    /// `ThePlayerList->getNeutralPlayer()`'s default team, `None` if there is no neutral player.
    fn neutral_player_default_team(&mut self) -> Option<u32>;
    /// `TheThingFactory->newObject(tt, team)`.
    fn new_object(&mut self, template: u16, team: u32) -> Option<ObjectId>;
    /// `visionObject->setPosition(pos)`.
    fn set_position(&mut self, object: ObjectId, pos: &Coord3D);
    /// `visionObject->setShroudClearingRange(range)`.
    fn set_shroud_clearing_range(&mut self, object: ObjectId, range: f32);
    /// `obj->getPosition()`.
    fn position(&self, object: ObjectId) -> Coord3D;
    /// `obj->getGeometryInfo().getBoundingSphereRadius()`.
    fn bounding_sphere_radius(&self, object: ObjectId) -> f32;
}

/// `BattlePlanUpdate::createVisionObject()` (BattlePlanUpdate.cpp:408): a neutral object of `m_visionObjectName` at the
/// bombardment's position, whose shroud clearing range is the owner's bounding sphere radius, so that the plan sees what
/// it covers. `vision_object_id` is `m_visionObjectID`; `owner` is `getObject()`.
pub fn create_vision_object<H: BattlePlanVisionHost>(
    h: &mut H,
    owner: ObjectId,
    vision_object_name: &[u8],
    vision_object_id: &mut ObjectId,
) {
    if if plant(15717) { *vision_object_id == INVALID_ID } else { *vision_object_id != INVALID_ID } {
        // don't want two.
        return;
    }
    // get template of object to create
    let Some(tt) = h.find_template(vision_object_name) else {
        // DEBUG_ASSERTCRASH( tt, ("BattlePlanUpdate::setStatus - Invalid vision object name '%s'\n", ...) );
        return;
    };
    // Player *pPlayer = ThePlayerList->getNeutralPlayer(); // sanity
    let team = match h.neutral_player_default_team() {
        Some(t) => t,
        None if plant(15722) => 0,
        None => return,
    };
    // create object for this player
    if let Some(vision_object) = h.new_object(tt, team) {
        // record we have an object
        if !plant(15720) {
            *vision_object_id = vision_object;
        }
        // set position
        if plant(15721) {
            // wrong order: the range before the position
            let radius = h.bounding_sphere_radius(owner);
            h.set_shroud_clearing_range(vision_object, radius);
            let pos = h.position(owner);
            h.set_position(vision_object, &pos);
            return;
        }
        let pos = h.position(owner);
        h.set_position(vision_object, &pos);
        // set the shroud clearing range
        let radius = h.bounding_sphere_radius(owner);
        h.set_shroud_clearing_range(vision_object, if plant(15719) { radius * 2.0 } else { radius });
    }
}
