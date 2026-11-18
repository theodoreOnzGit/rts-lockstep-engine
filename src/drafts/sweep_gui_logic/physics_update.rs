//! UNTESTED DRAFT (#202, #213, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PhysicsUpdate.cpp (PhysicsBehavior::getZFriction,
//!   isCurrentlyOverlapped, wasPreviouslyOverlapped)
//!
//! The verified `crate::logic::modules::physics_behavior::PhysicsBehavior` has the other three frictions
//! (`get_aerodynamic_friction`, `get_forward_friction`, `get_lateral_friction`, private) and the overlap ids
//! (`current_overlap`, `previous_overlap`, private, read through `peek_overlaps`) but not these three. They are
//! drafted as functions of the values they read, because the fields are private to that module.

use crate::common::game_type::ObjectId;

/// `MIN_NON_AERO_FRICTION` (PhysicsUpdate.cpp), also private in the verified module.
const MIN_NON_AERO_FRICTION: f32 = 0.01;
/// `MAX_FRICTION`.
const MAX_FRICTION: f32 = 0.99;

/// `PhysicsBehavior::getZFriction()` (PhysicsUpdate.cpp:310): "m_ZFriction + m_extraFriction", clamped to
/// `[MIN_NON_AERO_FRICTION, MAX_FRICTION]`. `z_friction` is the module data's `m_ZFriction`, `extra_friction` the
/// behaviour's `m_extraFriction`.
pub fn get_z_friction(z_friction: f32, extra_friction: f32) -> f32 {
    let mut f = z_friction + extra_friction;
    if f < MIN_NON_AERO_FRICTION {
        f = MIN_NON_AERO_FRICTION;
    }
    if f > MAX_FRICTION {
        f = MAX_FRICTION;
    }
    f
}

/// `PhysicsBehavior::isCurrentlyOverlapped(obj)` (PhysicsUpdate.cpp:1007): `obj != NULL && obj->getID() ==
/// m_currentOverlap`.
pub fn is_currently_overlapped(current_overlap: ObjectId, obj: Option<ObjectId>) -> bool {
    obj.is_some_and(|id| id == current_overlap)
}

/// `PhysicsBehavior::wasPreviouslyOverlapped(obj)` (PhysicsUpdate.cpp:1013): `obj != NULL && obj->getID() ==
/// m_previousOverlap`.
pub fn was_previously_overlapped(previous_overlap: ObjectId, obj: Option<ObjectId>) -> bool {
    obj.is_some_and(|id| id == previous_overlap)
}
