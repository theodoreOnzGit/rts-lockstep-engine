// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Locomotor.h
//   GeneralsMD/Code/GameEngine/Include/GameLogic/LocomotorSet.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Locomotor.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, Feb 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `LocomotorTemplate` (the `Locomotor` INI block), `LocomotorStore`
//! (`TheLocomotorStore`), `Locomotor` (the movement physics of every
//! appearance: legs, wheels, treads, hover, thrust, wings, climber, other) and
//! `LocomotorSet`.
//!
//! # What a `Locomotor` does
//!
//! Each frame the AI asks the object's current `Locomotor` to move it toward a
//! position ([`Locomotor::loco_update_move_towards_position`]), toward an angle
//! ([`Locomotor::loco_update_move_towards_angle`]), or to stay where it is
//! ([`Locomotor::loco_update_maintain_current_position`]). The `Locomotor`
//! never integrates anything: it turns the object (writing its orientation or
//! transform directly), applies forces to the object's `PhysicsBehavior`
//! (`applyMotiveForce`), and for the height of airborne or hovering things
//! applies lift forces or sets the position's Z. Physics integrates the forces.
//!
//! # The cut points: the `Object` is a [`LocomotorHost`]
//!
//! `Locomotor.cpp` reaches into five other subsystems. None of them is
//! translated yet at this layer (the `Object` is being grown in parallel,
//! `PhysicsBehavior` is layer 9, the pathfinder is layer 10), so every call
//! that crosses the boundary is a method of the trait [`LocomotorHost`],
//! implemented by whoever owns the object. A generic parameter, not a trait
//! object. The host stands for ONE object (the `Object* obj` argument of every
//! upstream function) together with its physics behaviour and body module.
//! The complete list, with the upstream call each replaces:
//!
//! | group | host method | upstream call |
//! |---|---|---|
//! | Object | `position`, `set_position` | `getPosition`, `setPosition` |
//! | Object | `orientation`, `set_orientation` | `getOrientation`, `setOrientation` |
//! | Object | `unit_direction_vector_2d` | `getUnitDirectionVector2D` |
//! | Object | `transform_matrix`, `set_transform_matrix` | `getTransformMatrix`, `setTransformMatrix` |
//! | Object | `layer` | `getLayer` |
//! | Object | `is_kind_of` | `isKindOf` (`KINDOF_PROJECTILE`, `KINDOF_DOZER`) |
//! | Object | `status_test`, `set_status` | `getStatusBits().test`, `setStatus` (`OBJECT_STATUS_BRAKING`, `OBJECT_STATUS_DECK_HEIGHT_OFFSET`) |
//! | Object | `carrier_deck_height` | `getCarrierDeckHeight` |
//! | Object | `major_radius`, `bounding_circle_radius` | `getGeometryInfo().getMajorRadius / getBoundingCircleRadius` |
//! | Object | `is_disabled_held` | `isDisabledByType(DISABLED_HELD)` |
//! | Object | `set_model_condition_state`, `clear_model_condition_state` | `setModelConditionState`, `clearModelConditionState` (`MODELCONDITION_OVER_WATER`) |
//! | Object | `is_above_terrain` | `isAboveTerrain` |
//! | Object | `has_physics` | `getPhysics() != NULL` |
//! | Body (layer 9) | `damage_state` | `getBodyModule()->getDamageState()` |
//! | Physics (layer 9) | `is_stunned`, `apply_motive_force`, `scrub_velocity_2d`, `set_turning` | `getIsStunned`, `applyMotiveForce`, `scrubVelocity2D`, `setTurning` |
//! | Physics (layer 9) | `velocity`, `velocity_magnitude`, `forward_speed_2d`, `forward_speed_3d`, `mass`, `is_motive` | the same-named getters |
//! | Physics (layer 9) | `set_extra_friction`, `set_allow_airborne_friction`, `set_stick_to_ground` | the same-named setters (`setPhysicsOptions`) |
//! | World | `frame` | `TheGameLogic->getFrame()` |
//! | World | `gravity`, `movement_penalty_damage_state` | `TheGlobalData->m_gravity`, `m_movementPenaltyDamageState` ([`LocomotorGlobals`]) |
//! | Terrain | `ground_height`, `layer_height`, `layer_height_unclipped`, `underwater`, `highest_layer_for_destination` | `TheTerrainLogic->getGroundHeight / getLayerHeight / isUnderwater / getHighestLayerForDestination` (translated: `logic::terrain_logic::TerrainLogic`) |
//! | Partition (layer 8) | `ground_or_structure_height` | `ThePartitionManager->getGroundOrStructureHeight` |
//! | AI (layer 10) | `valid_movement_terrain` | `TheAI->pathfinder()->validMovementTerrain(layer, loco, pos)` |
//!
//! `PhysicsTurningType` is defined here ([`PhysicsTurningType`]) because
//! `Locomotor.h` only forward-declares it and layer 9 has not been written;
//! `PhysicsUpdate` should re-export this definition.
//!
//! # Translation choices
//!
//! - **`OVERRIDE<LocomotorTemplate>`.** A `Locomotor` holds a pointer to a
//!   template in the store and reads its final override at every access. Here
//!   it holds an `Arc<LocomotorTemplate>`: a snapshot of the final override
//!   taken by [`LocomotorStore::new_locomotor`]. The two differ only if a
//!   template is overridden while a `Locomotor` made from it exists. Overrides
//!   are created by `INI_LOAD_CREATE_OVERRIDES` loads (map loading), before
//!   objects exist, and `reset()` destroys the objects first. *Not
//!   re-checked*: that no later INI load creates overrides.
//! - **Maths.** All elementary functions are `petir::Real`'s. Upstream's
//!   global `sqrt`, `atan2` and `fabs` are the C `double` functions (MSVC's
//!   `<math.h>`, see `differential/shim/math.h`): `sqrt(float)` is
//!   `(float)sqrt((double)x)`, and wherever the result takes part in a larger
//!   expression the expression is evaluated in `double`. Those places are
//!   marked `// double:` below and are exactly (a quotient of two floats rounded once
//!   from `double` equals the `float` quotient, so the two quotient sites below cannot be
//!   told apart from a `float` evaluation by any input - a planted `float` version was not
//!   caught, and cannot be; they are kept as written): `calcSlowDownDist`'s quotient,
//!   `calcLiftToUseAtPt`'s `brakeDist`, `moveTowardsPositionTreads`'s
//!   `angleCoeff > 0.05` and `2.0*slowDownDist`, the same two doubles in the
//!   wheels, and `moveTowardsPositionClimb`'s `pos.z-0.1`. No fused
//!   multiply-add anywhere.
//! - **`Locomotor(const Locomotor&)`** (protected, unused by this translation)
//!   is not translated: it drops `m_maintainPos` and `m_donutTimer`.
//!   `Locomotor` is `Clone` instead, which keeps everything.
//! - **`m_maintainPos`** is left uninitialised by upstream's constructor
//!   (`MAINTAIN_POS_IS_VALID` guards every read); here it starts at zero.
//! - **NULL checks.** `obj == NULL` cannot happen. `getPhysics() == NULL`
//!   (`DEBUG_CRASH` in debug builds, then an early return) is
//!   [`LocomotorHost::has_physics`].
//! - `DEBUG_CRASH`/`DEBUG_ASSERTCRASH` compile to nothing in the shipped
//!   release build and are not translated; the code after them (the
//!   `WINGS`/`THRUST` healing in `validate`) runs.
//! - `CIRCLE_FOR_LANDING` and `USE_ZDIR_DAMPING` are not defined upstream; the
//!   code under them is not translated.
//!
//! # Where upstream is undefined
//!
//! - `LOCO_LEGS_TWO` and `LOCO_CLIMBER` locomotors with `MinSpeed > 0` recurse
//!   without end (`moveTowardsPosition` -> `moveTowardsPositionLegs` ->
//!   `moveTowardsAngle` -> `moveTowardsPosition`): a stack overflow upstream and
//!   here. No shipped data has them.
//! - `normalizeAngle`/`stdAngleDiff` of an infinite angle never return
//!   upstream; `common::game_common::normalize_angle` panics instead.
//! - `moveTowardsPositionTreads` declares `Real relAngle` without a value and passes it to
//!   `rotateTowardsPosition`, which does not write it when the locomotor pivots
//!   (`TurnPivotOffset != 0`, not braking) and the object's pivot point is within 0.1 of the
//!   goal in both axes (`rotateObjAroundLocoPivot` returns `TURN_NONE` early). The value read
//!   is then whatever the stack held. Here it is 0, and [`Locomotor::undefined_inputs`]
//!   counts the event.
//! - `LocomotorStore::reset()` erases a template that was itself made by an override load
//!   (`m_locomotorTemplates.erase(it)`) and goes on with the erased iterator, reading freed
//!   memory and calling `deleteOverrides` on a deleted object. The Rust removes the template
//!   and its chain and continues with the next one.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::common::base_type::{clamp, sqr, Coord3D, PI};
use crate::common::game_common::{
    normalize_angle, std_angle_diff, LOGICFRAMES_PER_SECOND, SECONDS_PER_LOGICFRAME_REAL,
};
use crate::common::game_type::{PathfindLayer, LAYER_GROUND};
use crate::common::global_data::GlobalData;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, Undefined, UserData,
};
use crate::common::kind_of::{KindOfType, KINDOF_DOZER, KINDOF_PROJECTILE};
use crate::common::model_state::{ModelConditionFlagType, MODELCONDITION_OVER_WATER};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::object_status_types::{
    ObjectStatusTypes, OBJECT_STATUS_BRAKING, OBJECT_STATUS_DECK_HEIGHT_OFFSET,
};
use crate::common::overridable::Overridable;
use crate::common::random_value::RandomValues;
use crate::common::trig;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::module::body_module::{is_condition_better, BodyDamageType};
use crate::logic::terrain_logic::{Underwater, PATHFIND_CELL_SIZE_F};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use petir::Real;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// `DONUT_TIME_DELAY_SECONDS`.
const DONUT_TIME_DELAY_SECONDS: f32 = 2.5;
/// `DONUT_DISTANCE`: `4.0*PATHFIND_CELL_SIZE_F` (a `double` product stored in
/// a `Real`; exact).
const DONUT_DISTANCE: f32 = 40.0;
/// `MAX_BRAKING_FACTOR`.
const MAX_BRAKING_FACTOR: f32 = 5.0;
/// `BIGNUM`.
pub const BIGNUM: f32 = 99999.0;
/// `PATHFIND_CELL_SIZE` (an `int`; `AIPathfind.h`).
const PATHFIND_CELL_SIZE: f32 = 10.0;
/// A NaN payload that marks `relAngle` as not written by `rotateObjAroundLocoPivot`.
const UNWRITTEN_REL_ANGLE: u32 = 0x7fc0_dead;

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/// `enum LocomotorAppearance`: how the `Drawable` imitates the motion (and
/// which movement function runs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum LocomotorAppearance {
    LocoLegsTwo = 0,
    LocoWheelsFour = 1,
    LocoTreads = 2,
    LocoHover = 3,
    LocoThrust = 4,
    LocoWings = 5,
    /// Human climber: backs down cliffs.
    LocoClimber = 6,
    LocoOther = 7,
    LocoMotorcycle = 8,
}

impl LocomotorAppearance {
    /// The enumerator with this index (`TheLocomotorAppearanceNames`).
    pub fn from_index(i: i32) -> Option<Self> {
        use LocomotorAppearance::*;
        Some(match i {
            0 => LocoLegsTwo,
            1 => LocoWheelsFour,
            2 => LocoTreads,
            3 => LocoHover,
            4 => LocoThrust,
            5 => LocoWings,
            6 => LocoClimber,
            7 => LocoOther,
            8 => LocoMotorcycle,
            _ => return None,
        })
    }
}

/// `TheLocomotorAppearanceNames`.
pub const LOCO_APPEARANCE_NAMES: [&str; 9] = [
    "TWO_LEGS",
    "FOUR_WHEELS",
    "TREADS",
    "HOVER",
    "THRUST",
    "WINGS",
    "CLIMBER",
    "OTHER",
    "MOTORCYCLE",
];

/// `enum LocomotorPriority`: where in a group this one moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum LocomotorPriority {
    LocoMovesBack = 0,
    LocoMovesMiddle = 1,
    LocoMovesFront = 2,
}

impl LocomotorPriority {
    /// The enumerator with this index (`TheLocomotorPriorityNames`).
    pub fn from_index(i: i32) -> Option<Self> {
        Some(match i {
            0 => LocomotorPriority::LocoMovesBack,
            1 => LocomotorPriority::LocoMovesMiddle,
            2 => LocomotorPriority::LocoMovesFront,
            _ => return None,
        })
    }
}

/// `TheLocomotorPriorityNames`.
pub const LOCO_PRIORITY_NAMES: [&str; 3] = ["MOVES_BACK", "MOVES_MIDDLE", "MOVES_FRONT"];

/// `enum LocomotorBehaviorZ`: the Z-axis behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum LocomotorBehaviorZ {
    /// Does whatever physics tells it, but has no z-force of its own.
    ZNoZMotiveForce = 0,
    /// Keep at surface-of-water level.
    ZSeaLevel = 1,
    /// Try to follow a specific height relative to terrain/water height.
    ZSurfaceRelativeHeight = 2,
    /// Try to follow a specific height regardless of terrain/water height.
    ZAbsoluteHeight = 3,
    /// Stays fixed at a surface-relative height, regardless of physics.
    ZFixedSurfaceRelativeHeight = 4,
    /// Stays fixed at an absolute height, regardless of physics.
    ZFixedAbsoluteHeight = 5,
    /// Stays fixed at a surface-relative height including buildings.
    ZRelativeToGroundAndBuildings = 6,
    /// Try to follow a height relative to the highest layer.
    ZSmoothRelativeToHighestLayer = 7,
}

impl LocomotorBehaviorZ {
    /// The enumerator with this index (`TheLocomotorBehaviorZNames`).
    pub fn from_index(i: i32) -> Option<Self> {
        use LocomotorBehaviorZ::*;
        Some(match i {
            0 => ZNoZMotiveForce,
            1 => ZSeaLevel,
            2 => ZSurfaceRelativeHeight,
            3 => ZAbsoluteHeight,
            4 => ZFixedSurfaceRelativeHeight,
            5 => ZFixedAbsoluteHeight,
            6 => ZRelativeToGroundAndBuildings,
            7 => ZSmoothRelativeToHighestLayer,
            _ => return None,
        })
    }
}

/// `TheLocomotorBehaviorZNames`.
pub const LOCO_Z_NAMES: [&str; 8] = [
    "NO_Z_MOTIVE_FORCE",
    "SEA_LEVEL",
    "SURFACE_RELATIVE_HEIGHT",
    "ABSOLUTE_HEIGHT",
    "FIXED_SURFACE_RELATIVE_HEIGHT",
    "FIXED_ABSOLUTE_HEIGHT",
    "FIXED_RELATIVE_TO_GROUND_AND_BUILDINGS",
    "RELATIVE_TO_HIGHEST_LAYER",
];

/// `enum PhysicsTurningType` (`PhysicsUpdate.h`): which way the object is
/// turning, as told to physics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PhysicsTurningType {
    TurnNegative = -1,
    TurnNone = 0,
    TurnPositive = 1,
}

/// `LocomotorSurfaceTypeMask` (`Int`): which kinds of surface a locomotor can use.
pub type LocomotorSurfaceTypeMask = i32;
/// `LOCOMOTORSURFACE_GROUND`: clear, unobstructed ground.
pub const LOCOMOTORSURFACE_GROUND: LocomotorSurfaceTypeMask = 1 << 0;
/// `LOCOMOTORSURFACE_WATER`.
pub const LOCOMOTORSURFACE_WATER: LocomotorSurfaceTypeMask = 1 << 1;
/// `LOCOMOTORSURFACE_CLIFF`: steep altitude change.
pub const LOCOMOTORSURFACE_CLIFF: LocomotorSurfaceTypeMask = 1 << 2;
/// `LOCOMOTORSURFACE_AIR`.
pub const LOCOMOTORSURFACE_AIR: LocomotorSurfaceTypeMask = 1 << 3;
/// `LOCOMOTORSURFACE_RUBBLE`: building rubble.
pub const LOCOMOTORSURFACE_RUBBLE: LocomotorSurfaceTypeMask = 1 << 4;
/// `NO_SURFACES`.
pub const NO_SURFACES: LocomotorSurfaceTypeMask = 0x0000;
/// `ALL_SURFACES`.
pub const ALL_SURFACES: LocomotorSurfaceTypeMask = 0xffff;

/// `TheLocomotorSurfaceTypeNames`.
pub const LOCOMOTOR_SURFACE_TYPE_NAMES: [&str; 5] = ["GROUND", "WATER", "CLIFF", "AIR", "RUBBLE"];

// ---------------------------------------------------------------------------
// Cut points
// ---------------------------------------------------------------------------

/// The two `TheGlobalData` fields the locomotors read.
pub trait LocomotorGlobals {
    /// `TheGlobalData->m_gravity`: acceleration per frame squared (negative).
    fn gravity(&self) -> f32;
    /// `TheGlobalData->m_movementPenaltyDamageState`.
    fn movement_penalty_damage_state(&self) -> BodyDamageType;
}

impl LocomotorGlobals for GlobalData {
    fn gravity(&self) -> f32 {
        self.gravity
    }
    fn movement_penalty_damage_state(&self) -> BodyDamageType {
        self.movement_penalty_damage_state
    }
}

/// The object a [`Locomotor`] drives, with its physics behaviour, its body
/// module and the world services it reads: every call `Locomotor.cpp` makes
/// into another subsystem. See the module docs for the table.
///
/// The methods are the upstream calls, not abstractions: an implementation
/// forwards each to the object or subsystem. The setters that upstream does
/// through a pointer (`obj->setPosition(&pos)`) take the value.
pub trait LocomotorHost: LocomotorGlobals {
    // ---- Object ----
    /// `getPhysics() != NULL`.
    fn has_physics(&self) -> bool;
    /// `getPosition()`.
    fn position(&self) -> Coord3D;
    /// `setPosition(&pos)`.
    fn set_position(&mut self, pos: &Coord3D);
    /// `getOrientation()`.
    fn orientation(&self) -> f32;
    /// `setOrientation(angle)`.
    fn set_orientation(&mut self, angle: f32);
    /// `getUnitDirectionVector2D()`.
    fn unit_direction_vector_2d(&mut self) -> Coord3D;
    /// `*getTransformMatrix()`.
    fn transform_matrix(&self) -> Matrix3D;
    /// `setTransformMatrix(&mtx)`.
    fn set_transform_matrix(&mut self, mtx: &Matrix3D);
    /// `getLayer()`.
    fn layer(&self) -> PathfindLayer;
    /// `isKindOf(kind)`.
    fn is_kind_of(&self, kind: KindOfType) -> bool;
    /// `getStatusBits().test(status)`.
    fn status_test(&self, status: ObjectStatusTypes) -> bool;
    /// `setStatus(MAKE_OBJECT_STATUS_MASK(status), set)` (`set` defaults to true upstream).
    fn set_status(&mut self, status: ObjectStatusTypes, set: bool);
    /// `getCarrierDeckHeight()`.
    fn carrier_deck_height(&self) -> f32;
    /// `getGeometryInfo().getMajorRadius()`.
    fn major_radius(&self) -> f32;
    /// `getGeometryInfo().getBoundingCircleRadius()`.
    fn bounding_circle_radius(&self) -> f32;
    /// `isDisabledByType(DISABLED_HELD)`.
    fn is_disabled_held(&self) -> bool;
    /// `setModelConditionState(condition)`.
    fn set_model_condition_state(&mut self, condition: ModelConditionFlagType);
    /// `clearModelConditionState(condition)`.
    fn clear_model_condition_state(&mut self, condition: ModelConditionFlagType);
    /// `isAboveTerrain()`.
    fn is_above_terrain(&mut self) -> bool;

    // ---- Body module (layer 9) ----
    /// `getBodyModule()->getDamageState()`.
    fn damage_state(&self) -> BodyDamageType;

    // ---- PhysicsBehavior (layer 9) ----
    /// `getIsStunned()`.
    fn is_stunned(&self) -> bool;
    /// `applyMotiveForce(&force)`.
    fn apply_motive_force(&mut self, force: &Coord3D);
    /// `scrubVelocity2D(desiredSpeed)`.
    fn scrub_velocity_2d(&mut self, desired_speed: f32);
    /// `setTurning(turning)`.
    fn set_turning(&mut self, turning: PhysicsTurningType);
    /// `*getVelocity()`.
    fn velocity(&self) -> Coord3D;
    /// `getVelocityMagnitude()`.
    fn velocity_magnitude(&self) -> f32;
    /// `getForwardSpeed2D()`.
    fn forward_speed_2d(&self) -> f32;
    /// `getForwardSpeed3D()`.
    fn forward_speed_3d(&self) -> f32;
    /// `getMass()`.
    fn mass(&self) -> f32;
    /// `isMotive()`.
    fn is_motive(&self) -> bool;
    /// `setExtraFriction(friction)`.
    fn set_extra_friction(&mut self, friction: f32);
    /// `setAllowAirborneFriction(allow)`.
    fn set_allow_airborne_friction(&mut self, allow: bool);
    /// `setStickToGround(stick)`.
    fn set_stick_to_ground(&mut self, stick: bool);

    // ---- World ----
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;

    // ---- TerrainLogic (translated: logic::terrain_logic) ----
    /// `TheTerrainLogic->getGroundHeight(x, y)` (no normal wanted).
    fn ground_height(&mut self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->getLayerHeight(x, y, layer)` (no normal, `clip` true).
    fn layer_height(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32;
    /// `TheTerrainLogic->getLayerHeight(x, y, layer, &normal, false)`: the
    /// height even if off the edge of the bridge proper.
    fn layer_height_unclipped(&mut self, x: f32, y: f32, layer: PathfindLayer) -> f32;
    /// `TheTerrainLogic->isUnderwater(x, y, &waterZ, &terrainZ)`.
    fn underwater(&mut self, x: f32, y: f32) -> Underwater;
    /// `TheTerrainLogic->getHighestLayerForDestination(&pos)` (healthy bridges not required).
    fn highest_layer_for_destination(&mut self, pos: &Coord3D) -> PathfindLayer;

    // ---- PartitionManager (layer 8) ----
    /// `ThePartitionManager->getGroundOrStructureHeight(x, y)`.
    fn ground_or_structure_height(&mut self, x: f32, y: f32) -> f32;

    // ---- AI / Pathfinder (layer 10) ----
    /// `TheAI->pathfinder()->validMovementTerrain(layer, loco, &pos)`.
    fn valid_movement_terrain(
        &mut self,
        layer: PathfindLayer,
        loco: &Locomotor,
        pos: &Coord3D,
    ) -> bool;
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// `(float)sqrt((double)x)`: the global `sqrt` on a `Real`.
#[inline]
fn sqrt_d(x: f32) -> f32 {
    Real::sqrt(f64::from(x)) as f32
}

/// `atan2(y, x)` on `Real`s: the C `double` function, stored in a `Real`.
#[inline]
fn atan2_d(y: f32, x: f32) -> f32 {
    Real::atan2(f64::from(y), f64::from(x)) as f32
}

/// `calcSlowDownDist`.
fn calc_slow_down_dist(cur_speed: f32, desired_speed: f32, max_braking: f32) -> f32 {
    let delta = cur_speed - desired_speed;
    if delta <= 0.0 {
        return 0.0;
    }

    // double: `(sqr(delta) / fabs(maxBraking)) * 0.5f` (`fabs` returns a double)
    let dist = ((f64::from(sqr(delta)) / f64::from(max_braking.abs())) * f64::from(0.5f32)) as f32;

    // use a little fudge so that things can stop "on a dime" more easily...
    const FUDGE: f32 = 1.05;
    dist * FUDGE
}

/// `isNearlyZero`.
#[inline]
fn is_nearly_zero(a: f32) -> bool {
    const TINY_EPSILON: f32 = 0.001;
    a.abs() < TINY_EPSILON
}

/// `tryToRotateVector3D`: return the angle delta (in 3-space) we turned.
/// `max_angle` negative is a fraction (0...1) of the distance to rotate.
fn try_to_rotate_vector_3d(
    mut max_angle: f32,
    in_cur_dir: &Vector3,
    in_goal_dir: &Vector3,
    actual_dir: &mut Vector3,
) -> f32 {
    if is_nearly_zero(max_angle) {
        *actual_dir = *in_cur_dir;
        return 0.0;
    }

    let mut cur_dir = *in_cur_dir;
    cur_dir.normalize();

    let mut goal_dir = *in_goal_dir;
    goal_dir.normalize();

    // dot of two unit vectors is cos of angle between them.
    let cosine = Vector3::dot_product(&cur_dir, &goal_dir);
    // bound it in case of numerical error
    let mut angle_between = trig::acos(clamp(-1.0f32, cosine, 1.0f32));

    if max_angle < 0.0 {
        max_angle = -max_angle * angle_between;
        if is_nearly_zero(max_angle) {
            *actual_dir = *in_cur_dir;
            return 0.0;
        }
    }

    if angle_between.abs() <= max_angle {
        // close enough
        *actual_dir = goal_dir;
    } else {
        // nah, try as much as we can in the right dir.
        // we need to rotate around the axis perpendicular to these two vecs.
        // but: cross of two vectors is the perpendicular axis!
        let obj_cross_goal = Vector3::normalized_cross_product(&cur_dir, &goal_dir);

        angle_between = max_angle;
        let rot_mtx = Matrix3D::from_axis_angle(&obj_cross_goal, angle_between);
        *actual_dir = rot_mtx.rotate_vector(&cur_dir);
    }

    angle_between
}

/// `tryToOrientInThisDirection3D(obj, maxTurnRate, desiredDir)`.
fn try_to_orient_in_this_direction_3d<H: LocomotorHost>(
    obj: &mut H,
    max_turn_rate: f32,
    desired_dir: &Vector3,
) -> f32 {
    let mut actual_dir = Vector3::default();
    let x_vector = obj.transform_matrix().get_x_vector();
    let rel_angle = try_to_rotate_vector_3d(max_turn_rate, &x_vector, desired_dir, &mut actual_dir);
    if rel_angle != 0.0 {
        let p = obj.position();
        let obj_pos = Vector3::new(p.x, p.y, p.z);

        let mut new_xform = Matrix3D::default();
        new_xform.build_transform_matrix(&obj_pos, &actual_dir);

        obj.set_transform_matrix(&new_xform);
    }
    rel_angle
}

/// `calcDirectionToApplyThrust`.
fn calc_direction_to_apply_thrust<H: LocomotorHost>(
    obj: &H,
    ingoal_pos: &Coord3D,
    max_accel: f32,
    goal_dir: &mut Vector3,
) {
    /*
        our meta-goal here is to calculate the direction we should apply our motive force
        in order to minimize the angle between (our velocity) and (direction towards goalpos).

        this is complicated by the fact that we generally have an intrinsic velocity already,
        that must be accounted for, and by the fact that we can only apply force in our
        forward-x-direction (with a thrust-angle-range), and (due to limited range) might not
        be able to apply the force in the optimal direction!
    */

    // convert to Vector3, to use all its handy stuff
    let p = obj.position();
    let obj_pos = Vector3::new(p.x, p.y, p.z);
    let goal_pos = Vector3::new(ingoal_pos.x, ingoal_pos.y, ingoal_pos.z);

    let vec_to_goal = goal_pos - obj_pos;
    if is_nearly_zero(vec_to_goal.length2()) {
        // goal pos is essentially same as current pos, so just stay the same & return
        *goal_dir = obj.transform_matrix().get_x_vector();
        return;
    }

    // get our cur vel into a useful Vector3 form
    let v = obj.velocity();
    let mut cur_vel = Vector3::new(v.x, v.y, v.z);

    // add gravity to our vel so that we account for it in our calcs
    cur_vel.z += obj.gravity();

    let mut found_solution = false;
    let dist_to_goal_sqr = vec_to_goal.length2();
    let dist_to_goal = sqrt_d(dist_to_goal_sqr);
    let cur_vel_mag_sqr = cur_vel.length2();
    let cur_vel_mag = sqrt_d(cur_vel_mag_sqr);
    let max_accel_sqr = sqr(max_accel);

    let denom = cur_vel_mag_sqr - max_accel_sqr;
    if !is_nearly_zero(denom) {
        // solve the (greatly simplified) quadratic...
        let mut t = (dist_to_goal * (cur_vel_mag + max_accel)) / denom;
        let t2 = (dist_to_goal * (cur_vel_mag - max_accel)) / denom;
        if t >= 0.0 || t2 >= 0.0 {
            // choose the smallest positive t.
            if t < 0.0 || (t2 >= 0.0 && t2 < t) {
                t = t2;
            }

            // plug it in.
            if !is_nearly_zero(t) {
                goal_dir.x = (vec_to_goal.x / t) - cur_vel.x;
                goal_dir.y = (vec_to_goal.y / t) - cur_vel.y;
                goal_dir.z = (vec_to_goal.z / t) - cur_vel.z;
                goal_dir.normalize();
                found_solution = true;
            }
        }
    }
    if !found_solution {
        // Doh... no (useful) solution. revert to dumb.
        *goal_dir = vec_to_goal;
        goal_dir.normalize();
    }
}

// ---------------------------------------------------------------------------
// LocomotorTemplate
// ---------------------------------------------------------------------------

/// `class LocomotorTemplate`: the values of one `Locomotor` INI block.
///
/// Units: velocity is dist/frame, acceleration dist/(frame*frame), forces
/// (mass*dist)/(frame*frame).
#[derive(Clone, Debug, PartialEq)]
pub struct LocomotorTemplate {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_surfaces`: the kinds of surface we can use.
    pub surfaces: LocomotorSurfaceTypeMask,
    /// `m_maxSpeed`.
    pub max_speed: f32,
    /// `m_maxSpeedDamaged`: max speed when "damaged".
    pub max_speed_damaged: f32,
    /// `m_minSpeed`: we should never brake past this.
    pub min_speed: f32,
    /// `m_maxTurnRate`: rads/frame.
    pub max_turn_rate: f32,
    /// `m_maxTurnRateDamaged`.
    pub max_turn_rate_damaged: f32,
    /// `m_acceleration`.
    pub acceleration: f32,
    /// `m_accelerationDamaged`.
    pub acceleration_damaged: f32,
    /// `m_lift`: max lifting acceleration (flying objects only).
    pub lift: f32,
    /// `m_liftDamaged`.
    pub lift_damaged: f32,
    /// `m_braking`: max braking (deceleration).
    pub braking: f32,
    /// `m_minTurnSpeed`: we must be going >= this speed in order to turn.
    pub min_turn_speed: f32,
    /// `m_preferredHeight`.
    pub preferred_height: f32,
    /// `m_preferredHeightDamping`: 1.0 = very aggressive, 0.1 = gradual.
    pub preferred_height_damping: f32,
    /// `m_circlingRadius`: pos = cw, neg = ccw, 0 = smallest possible.
    pub circling_radius: f32,
    /// `m_speedLimitZ`.
    pub speed_limit_z: f32,
    /// `m_extra2DFriction`: extra 2D friction to apply (via physics).
    pub extra_2d_friction: f32,
    /// `m_maxThrustAngle`: THRUST locos only.
    pub max_thrust_angle: f32,
    /// `m_behaviorZ`.
    pub behavior_z: LocomotorBehaviorZ,
    /// `m_appearance`.
    pub appearance: LocomotorAppearance,
    /// `m_movePriority`.
    pub move_priority: LocomotorPriority,
    /// `m_accelPitchLimit`.
    pub accel_pitch_limit: f32,
    /// `m_decelPitchLimit`.
    pub decel_pitch_limit: f32,
    /// `m_bounceKick`.
    pub bounce_kick: f32,
    /// `m_pitchStiffness`.
    pub pitch_stiffness: f32,
    /// `m_rollStiffness`.
    pub roll_stiffness: f32,
    /// `m_pitchDamping`.
    pub pitch_damping: f32,
    /// `m_rollDamping`.
    pub roll_damping: f32,
    /// `m_pitchByZVelCoef`.
    pub pitch_by_z_vel_coef: f32,
    /// `m_thrustRoll`.
    pub thrust_roll: f32,
    /// `m_wobbleRate`.
    pub wobble_rate: f32,
    /// `m_minWobble`.
    pub min_wobble: f32,
    /// `m_maxWobble`.
    pub max_wobble: f32,
    /// `m_forwardVelCoef`.
    pub forward_vel_coef: f32,
    /// `m_lateralVelCoef`.
    pub lateral_vel_coef: f32,
    /// `m_forwardAccelCoef`.
    pub forward_accel_coef: f32,
    /// `m_lateralAccelCoef`.
    pub lateral_accel_coef: f32,
    /// `m_uniformAxialDamping`.
    pub uniform_axial_damping: f32,
    /// `m_turnPivotOffset`: -1.0 = rear, 0.0 = center, 1.0 = front.
    pub turn_pivot_offset: f32,
    /// `m_airborneTargetingHeight`.
    pub airborne_targeting_height: i32,
    /// `m_closeEnoughDist`.
    pub close_enough_dist: f32,
    /// `m_isCloseEnoughDist3D`.
    pub is_close_enough_dist_3d: bool,
    /// `m_ultraAccurateSlideIntoPlaceFactor`.
    pub ultra_accurate_slide_into_place_factor: f32,
    /// `m_locomotorWorksWhenDead`.
    pub locomotor_works_when_dead: bool,
    /// `m_allowMotiveForceWhileAirborne`.
    pub allow_motive_force_while_airborne: bool,
    /// `m_apply2DFrictionWhenAirborne`.
    pub apply_2d_friction_when_airborne: bool,
    /// `m_downhillOnly`: pinewood derby, moves only by gravity pulling downhill.
    pub downhill_only: bool,
    /// `m_stickToGround`.
    pub stick_to_ground: bool,
    /// `m_canMoveBackward`.
    pub can_move_backward: bool,
    /// `m_hasSuspension`.
    pub has_suspension: bool,
    /// `m_maximumWheelExtension` (a negative value).
    pub maximum_wheel_extension: f32,
    /// `m_maximumWheelCompression` (a positive value).
    pub maximum_wheel_compression: f32,
    /// `m_wheelTurnAngle`.
    pub wheel_turn_angle: f32,
    /// `m_wanderWidthFactor`.
    pub wander_width_factor: f32,
    /// `m_wanderLengthFactor`.
    pub wander_length_factor: f32,
    /// `m_wanderAboutPointRadius`.
    pub wander_about_point_radius: f32,
    /// `m_rudderCorrectionDegree`.
    pub rudder_correction_degree: f32,
    /// `m_rudderCorrectionRate`.
    pub rudder_correction_rate: f32,
    /// `m_elevatorCorrectionDegree`.
    pub elevator_correction_degree: f32,
    /// `m_elevatorCorrectionRate`.
    pub elevator_correction_rate: f32,
}

impl Default for LocomotorTemplate {
    /// `LocomotorTemplate::LocomotorTemplate()`.
    fn default() -> Self {
        LocomotorTemplate {
            name: Vec::new(),
            // these values mean "make the same as undamaged if not explicitly specified"
            max_speed_damaged: -1.0,
            max_turn_rate_damaged: -1.0,
            acceleration_damaged: -1.0,
            lift_damaged: -1.0,

            surfaces: 0,
            max_speed: 0.0,
            max_turn_rate: 0.0,
            acceleration: 0.0,
            lift: 0.0,
            braking: BIGNUM,
            min_speed: 0.0,
            min_turn_speed: BIGNUM,
            behavior_z: LocomotorBehaviorZ::ZNoZMotiveForce,
            appearance: LocomotorAppearance::LocoOther,
            move_priority: LocomotorPriority::LocoMovesMiddle,
            preferred_height: 0.0,
            preferred_height_damping: 1.0,
            circling_radius: 0.0,

            max_thrust_angle: 0.0,
            speed_limit_z: 999999.0,
            extra_2d_friction: 0.0,

            accel_pitch_limit: 0.0,
            decel_pitch_limit: 0.0,
            bounce_kick: 0.0,

            // it's highly unlikely you want zero for the defaults for stiffness and damping... (srj)
            pitch_stiffness: 0.1,
            roll_stiffness: 0.1,
            pitch_damping: 0.9,
            roll_damping: 0.9,
            forward_vel_coef: 0.0,
            pitch_by_z_vel_coef: 0.0,
            thrust_roll: 0.0,
            wobble_rate: 0.0,
            min_wobble: 0.0,
            max_wobble: 0.0,
            lateral_vel_coef: 0.0,
            forward_accel_coef: 0.0,
            lateral_accel_coef: 0.0,
            uniform_axial_damping: 1.0,
            turn_pivot_offset: 0.0,
            apply_2d_friction_when_airborne: false,
            downhill_only: false,
            allow_motive_force_while_airborne: false,
            locomotor_works_when_dead: false,
            airborne_targeting_height: i32::MAX,
            stick_to_ground: false,
            can_move_backward: false,
            has_suspension: false,
            wheel_turn_angle: 0.0,
            maximum_wheel_extension: 0.0,
            maximum_wheel_compression: 0.0,
            close_enough_dist: 1.0,
            is_close_enough_dist_3d: false,
            ultra_accurate_slide_into_place_factor: 0.0,

            wander_width_factor: 0.0,
            wander_length_factor: 1.0,
            wander_about_point_radius: 0.0,

            rudder_correction_degree: 0.0,
            rudder_correction_rate: 0.0,
            elevator_correction_degree: 0.0,
            elevator_correction_rate: 0.0,
        }
    }
}

impl LocomotorTemplate {
    /// `LocomotorTemplate()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `validate()`: fill the "damaged" values that were omitted, and heal or
    /// reject what `WINGS` and `THRUST` locomotors cannot have. `Err` is
    /// upstream's `throw INI_INVALID_DATA`.
    pub fn validate(&mut self) -> IniResult<()> {
        // for 'damaged' stuff that was omitted, set 'em to be the same as 'undamaged'...
        if self.max_speed_damaged < 0.0 {
            self.max_speed_damaged = self.max_speed;
        }

        if self.max_turn_rate_damaged < 0.0 {
            self.max_turn_rate_damaged = self.max_turn_rate;
        }

        if self.acceleration_damaged < 0.0 {
            self.acceleration_damaged = self.acceleration;
        }

        if self.lift_damaged < 0.0 {
            self.lift_damaged = self.lift;
        }

        if self.appearance == LocomotorAppearance::LocoWings {
            if self.min_speed <= 0.0 {
                // WINGS should always have positive minSpeeds (otherwise, they hover)
                self.min_speed = 0.01;
            }
            if self.min_turn_speed <= 0.0 {
                // WINGS should always have positive minTurnSpeed
                self.min_turn_speed = 0.01;
            }
        }

        if self.appearance == LocomotorAppearance::LocoThrust {
            if self.behavior_z != LocomotorBehaviorZ::ZNoZMotiveForce
                || self.lift != 0.0
                || self.lift_damaged != 0.0
            {
                // THRUST locos may not use ZAxisBehavior or lift!
                return Err(IniError::new(IniErrorKind::InvalidData));
            }
            if self.max_speed <= 0.0 {
                // if one of these was omitted, it defaults to zero... just quietly heal it here, rather than crashing
                self.max_speed = 0.01;
            }
            if self.max_speed_damaged <= 0.0 {
                self.max_speed_damaged = 0.01;
            }
            if self.min_speed <= 0.0 {
                self.min_speed = 0.01;
            }
        }
        Ok(())
    }
}

/// `parseFrictionPerSec`.
fn parse_friction_per_sec(
    ini: &mut Ini<'_>,
    t: &mut LocomotorTemplate,
    _ud: &UserData,
) -> IniResult<()> {
    let fric_per_sec = Ini::scan_real(&ini.get_next_token(None)?)?;
    let fric_per_frame = fric_per_sec * SECONDS_PER_LOGICFRAME_REAL;
    t.extra_2d_friction = fric_per_frame;
    Ok(())
}

macro_rules! real_field {
    ($name:literal, $parse:ident, $field:ident) => {
        FieldParse::new(
            $name,
            |ini: &mut Ini<'_>, t: &mut LocomotorTemplate, _ud: &UserData| {
                t.$field = ini.$parse()?;
                Ok(())
            },
            UserData::None,
        )
    };
}

macro_rules! bool_field {
    ($name:literal, $field:ident) => {
        FieldParse::new(
            $name,
            |ini: &mut Ini<'_>, t: &mut LocomotorTemplate, _ud: &UserData| {
                t.$field = ini.parse_bool()?;
                Ok(())
            },
            UserData::None,
        )
    };
}

/// `LocomotorTemplate::getFieldParse()`: the field table of a `Locomotor` block.
pub static LOCOMOTOR_TEMPLATE_FIELDS: [FieldParse<LocomotorTemplate>; 61] = [
    FieldParse::new(
        "Surfaces",
        |ini, t, ud| {
            let mut bits = t.surfaces as u32;
            ini.parse_bit_string32(&mut bits, ud.names())?;
            t.surfaces = bits as i32;
            Ok(())
        },
        UserData::Names(&LOCOMOTOR_SURFACE_TYPE_NAMES),
    ),
    real_field!("Speed", parse_velocity_real, max_speed),
    real_field!("SpeedDamaged", parse_velocity_real, max_speed_damaged),
    real_field!("TurnRate", parse_angular_velocity_real, max_turn_rate),
    real_field!(
        "TurnRateDamaged",
        parse_angular_velocity_real,
        max_turn_rate_damaged
    ),
    real_field!("Acceleration", parse_acceleration_real, acceleration),
    real_field!(
        "AccelerationDamaged",
        parse_acceleration_real,
        acceleration_damaged
    ),
    real_field!("Lift", parse_acceleration_real, lift),
    real_field!("LiftDamaged", parse_acceleration_real, lift_damaged),
    real_field!("Braking", parse_acceleration_real, braking),
    real_field!("MinSpeed", parse_velocity_real, min_speed),
    real_field!("MinTurnSpeed", parse_velocity_real, min_turn_speed),
    real_field!("PreferredHeight", parse_real, preferred_height),
    real_field!(
        "PreferredHeightDamping",
        parse_real,
        preferred_height_damping
    ),
    real_field!("CirclingRadius", parse_real, circling_radius),
    FieldParse::new("Extra2DFriction", parse_friction_per_sec, UserData::None),
    real_field!("SpeedLimitZ", parse_velocity_real, speed_limit_z),
    // yes, angle, not angular-vel
    real_field!("MaxThrustAngle", parse_angle_real, max_thrust_angle),
    FieldParse::new(
        "ZAxisBehavior",
        |ini, t, ud| {
            let i = ini.parse_index_list(ud.names())?;
            t.behavior_z = LocomotorBehaviorZ::from_index(i)
                .ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
            Ok(())
        },
        UserData::Names(&LOCO_Z_NAMES),
    ),
    FieldParse::new(
        "Appearance",
        |ini, t, ud| {
            let i = ini.parse_index_list(ud.names())?;
            t.appearance = LocomotorAppearance::from_index(i)
                .ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
            Ok(())
        },
        UserData::Names(&LOCO_APPEARANCE_NAMES),
    ),
    FieldParse::new(
        "GroupMovementPriority",
        |ini, t, ud| {
            let i = ini.parse_index_list(ud.names())?;
            t.move_priority = LocomotorPriority::from_index(i)
                .ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
            Ok(())
        },
        UserData::Names(&LOCO_PRIORITY_NAMES),
    ),
    real_field!(
        "AccelerationPitchLimit",
        parse_angle_real,
        accel_pitch_limit
    ),
    real_field!(
        "DecelerationPitchLimit",
        parse_angle_real,
        decel_pitch_limit
    ),
    real_field!("BounceAmount", parse_angular_velocity_real, bounce_kick),
    real_field!("PitchStiffness", parse_real, pitch_stiffness),
    real_field!("RollStiffness", parse_real, roll_stiffness),
    real_field!("PitchDamping", parse_real, pitch_damping),
    real_field!("RollDamping", parse_real, roll_damping),
    real_field!("ThrustRoll", parse_real, thrust_roll),
    real_field!("ThrustWobbleRate", parse_real, wobble_rate),
    real_field!("ThrustMinWobble", parse_real, min_wobble),
    real_field!("ThrustMaxWobble", parse_real, max_wobble),
    real_field!(
        "PitchInDirectionOfZVelFactor",
        parse_real,
        pitch_by_z_vel_coef
    ),
    real_field!("ForwardVelocityPitchFactor", parse_real, forward_vel_coef),
    real_field!("LateralVelocityRollFactor", parse_real, lateral_vel_coef),
    real_field!(
        "ForwardAccelerationPitchFactor",
        parse_real,
        forward_accel_coef
    ),
    real_field!(
        "LateralAccelerationRollFactor",
        parse_real,
        lateral_accel_coef
    ),
    real_field!("UniformAxialDamping", parse_real, uniform_axial_damping),
    real_field!("TurnPivotOffset", parse_real, turn_pivot_offset),
    bool_field!(
        "Apply2DFrictionWhenAirborne",
        apply_2d_friction_when_airborne
    ),
    bool_field!("DownhillOnly", downhill_only),
    bool_field!(
        "AllowAirborneMotiveForce",
        allow_motive_force_while_airborne
    ),
    bool_field!("LocomotorWorksWhenDead", locomotor_works_when_dead),
    real_field!(
        "AirborneTargetingHeight",
        parse_int,
        airborne_targeting_height
    ),
    bool_field!("StickToGround", stick_to_ground),
    bool_field!("CanMoveBackwards", can_move_backward),
    bool_field!("HasSuspension", has_suspension),
    real_field!("FrontWheelTurnAngle", parse_angle_real, wheel_turn_angle),
    real_field!("MaximumWheelExtension", parse_real, maximum_wheel_extension),
    real_field!(
        "MaximumWheelCompression",
        parse_real,
        maximum_wheel_compression
    ),
    real_field!("CloseEnoughDist", parse_real, close_enough_dist),
    bool_field!("CloseEnoughDist3D", is_close_enough_dist_3d),
    real_field!(
        "SlideIntoPlaceTime",
        parse_duration_real,
        ultra_accurate_slide_into_place_factor
    ),
    real_field!("WanderWidthFactor", parse_real, wander_width_factor),
    real_field!("WanderLengthFactor", parse_real, wander_length_factor),
    real_field!(
        "WanderAboutPointRadius",
        parse_real,
        wander_about_point_radius
    ),
    real_field!(
        "RudderCorrectionDegree",
        parse_real,
        rudder_correction_degree
    ),
    real_field!("RudderCorrectionRate", parse_real, rudder_correction_rate),
    real_field!(
        "ElevatorCorrectionDegree",
        parse_real,
        elevator_correction_degree
    ),
    real_field!(
        "ElevatorCorrectionRate",
        parse_real,
        elevator_correction_rate
    ),
    FieldParse::end(),
];

// ---------------------------------------------------------------------------
// LocomotorStore
// ---------------------------------------------------------------------------

/// `class LocomotorStore` (`TheLocomotorStore`): every `LocomotorTemplate`
/// read from INI, by name key, each with its override chain.
#[derive(Clone, Debug, Default)]
pub struct LocomotorStore {
    /// `m_locomotorTemplates` (a `std::map`, ordered by key).
    locomotor_templates: BTreeMap<NameKeyType, Overridable<LocomotorTemplate>>,
}

impl LocomotorStore {
    /// `LocomotorStore()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of templates.
    pub fn len(&self) -> usize {
        self.locomotor_templates.len()
    }

    /// Whether there are no templates.
    pub fn is_empty(&self) -> bool {
        self.locomotor_templates.is_empty()
    }

    /// Every template chain, by key.
    pub fn chains(&self) -> impl Iterator<Item = (NameKeyType, &Overridable<LocomotorTemplate>)> {
        self.locomotor_templates.iter().map(|(k, t)| (*k, t))
    }

    /// `findLocomotorTemplate(namekey) const`: the template (the head of its
    /// chain), or `None` for `NAMEKEY_INVALID` or an unknown key. Read it
    /// through [`Overridable::get_final_override`] as `OVERRIDE<T>` does.
    pub fn find_locomotor_template(
        &self,
        namekey: NameKeyType,
    ) -> Option<&Overridable<LocomotorTemplate>> {
        if namekey == NameKeyType::INVALID {
            return None;
        }
        self.locomotor_templates.get(&namekey)
    }

    /// `newLocomotor(tmpl)` for the template of `namekey`: a `Locomotor` on the
    /// template's final override. `None` if there is no such template.
    pub fn new_locomotor(
        &self,
        namekey: NameKeyType,
        rng: &mut RandomValues,
        frame: u32,
    ) -> Option<Locomotor> {
        let tmpl = self.find_locomotor_template(namekey)?;
        Some(Locomotor::new(
            Arc::new(tmpl.get_final_override().data.clone()),
            rng,
            frame,
        ))
    }

    /// `reset()`: clean up overrides; a template that was itself made by an
    /// override load is removed.
    pub fn reset(&mut self) {
        let old = std::mem::take(&mut self.locomotor_templates);
        for (key, node) in old {
            if let Some(kept) = Box::new(node).delete_overrides() {
                self.locomotor_templates.insert(key, *kept);
            }
        }
    }

    /// `LocomotorStore::parseLocomotorTemplateDefinition(ini)` (the
    /// `Locomotor` block): the name, then the template (found, or new; for an
    /// override load a new override of the final override), its fields until
    /// `End`, then `validate()`. A new template is stored only if both
    /// succeed; an override is linked before it is filled and stays linked if
    /// either fails.
    pub fn parse_locomotor_template_definition(
        &mut self,
        ini: &mut Ini<'_>,
        names: &mut NameKeyGenerator,
    ) -> IniResult<()> {
        // read the Locomotor name
        let token = ini.get_next_token(None)?;
        let namekey = names
            .try_name_to_key(&token)
            .map_err(|_| IniError::undefined(Undefined::NameKeysExhausted))?;
        let overrides = ini.get_load_type() == IniLoadType::CreateOverrides;

        if let Some(head) = self.locomotor_templates.get_mut(&namekey) {
            // isOverride = true: the map keeps the existing head
            let target: &mut LocomotorTemplate = if overrides {
                // newOverride(final override): a copy, marked, hung off the final override
                let last = head.friend_get_final_override();
                let mut copy = Overridable::new(last.data.clone());
                copy.mark_as_override();
                last.set_next_override(Some(Box::new(copy)));
                &mut last.friend_get_final_override().data
            } else {
                // `loco` is the head itself
                &mut head.data
            };
            target.name = token;
            ini.init_from_ini(target, &LOCOMOTOR_TEMPLATE_FIELDS)?;
            target.validate()?;
            Ok(())
        } else {
            let mut node = Overridable::new(LocomotorTemplate::default());
            if overrides {
                node.mark_as_override();
            }
            node.data.name = token;
            ini.init_from_ini(&mut node.data, &LOCOMOTOR_TEMPLATE_FIELDS)?;
            node.data.validate()?;
            self.locomotor_templates.insert(namekey, node);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Locomotor
// ---------------------------------------------------------------------------

// Note: these values are saved in save files, so you MUST NOT REMOVE OR CHANGE
// existing values!
const IS_BRAKING: u32 = 0;
const ALLOW_INVALID_POSITION: u32 = 1;
const MAINTAIN_POS_IS_VALID: u32 = 2;
const PRECISE_Z_POS: u32 = 3;
const NO_SLOW_DOWN_AS_APPROACHING_DEST: u32 = 4;
/// To allow things to move slower/faster over water and do special effects.
const OVER_WATER: u32 = 5;
const ULTRA_ACCURATE: u32 = 6;
const MOVING_BACKWARDS: u32 = 7;
const DOING_THREE_POINT_TURN: u32 = 8;
const CLIMBING: u32 = 9;
const IS_CLOSE_ENOUGH_DIST_3D: u32 = 10;
const OFFSET_INCREASING: u32 = 11;

/// `class Locomotor`: the movement state of one object's locomotor, on a
/// template.
#[derive(Clone, Debug)]
pub struct Locomotor {
    /// `m_template`.
    template: Arc<LocomotorTemplate>,
    /// `m_maintainPos`.
    maintain_pos: Coord3D,
    /// `m_brakingFactor`.
    braking_factor: f32,
    /// `m_maxLift`.
    max_lift: f32,
    /// `m_maxSpeed`.
    max_speed: f32,
    /// `m_maxAccel`.
    max_accel: f32,
    /// `m_maxBraking`.
    max_braking: f32,
    /// `m_maxTurnRate`.
    max_turn_rate: f32,
    /// `m_closeEnoughDist`.
    close_enough_dist: f32,
    /// `m_flags`.
    flags: u32,
    /// `m_preferredHeight`.
    preferred_height: f32,
    /// `m_preferredHeightDamping`.
    preferred_height_damping: f32,
    /// `m_angleOffset`.
    angle_offset: f32,
    /// `m_offsetIncrement`.
    offset_increment: f32,
    /// `m_donutTimer`: frame time to keep units from doing the donut.
    donut_timer: u32,
    /// How many times an input that is undefined upstream was met (see the module docs); not
    /// part of the saved state.
    undefined_inputs: u32,
}

impl Locomotor {
    /// `Locomotor(tmpl)`: draws three logic random values (the wander angle
    /// offset, its increment and its direction), and starts the donut timer
    /// at `frame` plus 2.5 seconds.
    pub fn new(template: Arc<LocomotorTemplate>, rng: &mut RandomValues, frame: u32) -> Self {
        let mut l = Locomotor {
            maintain_pos: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            braking_factor: 1.0,
            max_lift: BIGNUM,
            max_speed: BIGNUM,
            max_accel: BIGNUM,
            max_braking: BIGNUM,
            max_turn_rate: BIGNUM,
            flags: 0,
            close_enough_dist: template.close_enough_dist,
            preferred_height: template.preferred_height,
            preferred_height_damping: template.preferred_height_damping,
            angle_offset: 0.0,
            offset_increment: 0.0,
            donut_timer: 0,
            undefined_inputs: 0,
            template,
        };
        l.set_flag(IS_CLOSE_ENOUGH_DIST_3D, l.template.is_close_enough_dist_3d);
        l.angle_offset = rng.game_logic_random_value_real(-PI / 6.0, PI / 6.0);
        l.offset_increment = (PI / 40.0)
            * (rng.game_logic_random_value_real(0.8, 1.2) / l.template.wander_length_factor);
        let increasing = rng.game_logic_random_value(0, 1) != 0;
        l.set_flag(OFFSET_INCREASING, increasing);
        l.donut_timer = Self::donut_deadline(frame);
        l
    }

    /// `TheGameLogic->getFrame()+DONUT_TIME_DELAY_SECONDS*LOGICFRAMES_PER_SECOND`
    /// stored in an `UnsignedInt`: the frame is converted to `float` first.
    fn donut_deadline(frame: u32) -> u32 {
        ((frame as f32) + DONUT_TIME_DELAY_SECONDS * (LOGICFRAMES_PER_SECOND as f32)) as u32
    }

    #[inline]
    fn get_flag(&self, f: u32) -> bool {
        (self.flags & (1 << f)) != 0
    }

    #[inline]
    fn set_flag(&mut self, f: u32, b: bool) {
        if b {
            self.flags |= 1 << f;
        } else {
            self.flags &= !(1 << f);
        }
    }

    // ---- accessors ----

    /// `getTemplateName()`.
    pub fn get_template_name(&self) -> &[u8] {
        &self.template.name
    }
    /// The template (`m_template`, its final override at construction).
    pub fn template(&self) -> &LocomotorTemplate {
        &self.template
    }
    /// `m_flags`, for inspection (the bits are `LocoFlag`, saved in save files).
    pub fn flags(&self) -> u32 {
        self.flags
    }
    /// `m_maintainPos`, for inspection.
    pub fn maintain_pos(&self) -> Coord3D {
        self.maintain_pos
    }
    /// `m_brakingFactor`, for inspection.
    pub fn braking_factor(&self) -> f32 {
        self.braking_factor
    }
    /// `m_angleOffset`, for inspection.
    pub fn angle_offset(&self) -> f32 {
        self.angle_offset
    }
    /// `m_offsetIncrement`, for inspection.
    pub fn offset_increment(&self) -> f32 {
        self.offset_increment
    }
    /// `m_donutTimer`, for inspection.
    pub fn donut_timer(&self) -> u32 {
        self.donut_timer
    }
    /// How many times a `moveTowardsPositionTreads` read `relAngle` where upstream leaves it
    /// uninitialised (see the module docs); the Rust uses 0 there.
    pub fn undefined_inputs(&self) -> u32 {
        self.undefined_inputs
    }
    /// `getPreferredHeight()`: no damage consideration.
    pub fn get_preferred_height(&self) -> f32 {
        self.preferred_height
    }
    /// `restorePreferredHeightFromTemplate()`.
    pub fn restore_preferred_height_from_template(&mut self) {
        self.preferred_height = self.template.preferred_height;
    }
    /// `getPreferredHeightDamping()`.
    pub fn get_preferred_height_damping(&self) -> f32 {
        self.preferred_height_damping
    }
    /// `getAppearance()`.
    pub fn get_appearance(&self) -> LocomotorAppearance {
        self.template.appearance
    }
    /// `getMovePriority()`.
    pub fn get_move_priority(&self) -> LocomotorPriority {
        self.template.move_priority
    }
    /// `getLegalSurfaces()`.
    pub fn get_legal_surfaces(&self) -> LocomotorSurfaceTypeMask {
        self.template.surfaces
    }
    /// `getMinSpeed()`.
    pub fn get_min_speed(&self) -> f32 {
        self.template.min_speed
    }
    /// `getAccelPitchLimit()`.
    pub fn get_accel_pitch_limit(&self) -> f32 {
        self.template.accel_pitch_limit
    }
    /// `getDecelPitchLimit()`.
    pub fn get_decel_pitch_limit(&self) -> f32 {
        self.template.decel_pitch_limit
    }
    /// `getBounceKick()`.
    pub fn get_bounce_kick(&self) -> f32 {
        self.template.bounce_kick
    }
    /// `getPitchStiffness()`.
    pub fn get_pitch_stiffness(&self) -> f32 {
        self.template.pitch_stiffness
    }
    /// `getRollStiffness()`.
    pub fn get_roll_stiffness(&self) -> f32 {
        self.template.roll_stiffness
    }
    /// `getPitchDamping()`.
    pub fn get_pitch_damping(&self) -> f32 {
        self.template.pitch_damping
    }
    /// `getRollDamping()`.
    pub fn get_roll_damping(&self) -> f32 {
        self.template.roll_damping
    }
    /// `getPitchByZVelCoef()`.
    pub fn get_pitch_by_z_vel_coef(&self) -> f32 {
        self.template.pitch_by_z_vel_coef
    }
    /// `getThrustRoll()`.
    pub fn get_thrust_roll(&self) -> f32 {
        self.template.thrust_roll
    }
    /// `getWobbleRate()`.
    pub fn get_wobble_rate(&self) -> f32 {
        self.template.wobble_rate
    }
    /// `getMaxWobble()`.
    pub fn get_max_wobble(&self) -> f32 {
        self.template.max_wobble
    }
    /// `getMinWobble()`.
    pub fn get_min_wobble(&self) -> f32 {
        self.template.min_wobble
    }
    /// `getForwardVelCoef()`.
    pub fn get_forward_vel_coef(&self) -> f32 {
        self.template.forward_vel_coef
    }
    /// `getLateralVelCoef()`.
    pub fn get_lateral_vel_coef(&self) -> f32 {
        self.template.lateral_vel_coef
    }
    /// `getForwardAccelCoef()`.
    pub fn get_forward_accel_coef(&self) -> f32 {
        self.template.forward_accel_coef
    }
    /// `getLateralAccelCoef()`.
    pub fn get_lateral_accel_coef(&self) -> f32 {
        self.template.lateral_accel_coef
    }
    /// `getUniformAxialDamping()`.
    pub fn get_uniform_axial_damping(&self) -> f32 {
        self.template.uniform_axial_damping
    }
    /// `getTurnPivotOffset()`.
    pub fn get_turn_pivot_offset(&self) -> f32 {
        self.template.turn_pivot_offset
    }
    /// `getApply2DFrictionWhenAirborne()`.
    pub fn get_apply_2d_friction_when_airborne(&self) -> bool {
        self.template.apply_2d_friction_when_airborne
    }
    /// `getIsDownhillOnly()`.
    pub fn get_is_downhill_only(&self) -> bool {
        self.template.downhill_only
    }
    /// `getAllowMotiveForceWhileAirborne()`.
    pub fn get_allow_motive_force_while_airborne(&self) -> bool {
        self.template.allow_motive_force_while_airborne
    }
    /// `getAirborneTargetingHeight()`.
    pub fn get_airborne_targeting_height(&self) -> i32 {
        self.template.airborne_targeting_height
    }
    /// `getLocomotorWorksWhenDead()`.
    pub fn get_locomotor_works_when_dead(&self) -> bool {
        self.template.locomotor_works_when_dead
    }
    /// `getStickToGround()`.
    pub fn get_stick_to_ground(&self) -> bool {
        self.template.stick_to_ground
    }
    /// `getCloseEnoughDist()`.
    pub fn get_close_enough_dist(&self) -> f32 {
        self.close_enough_dist
    }
    /// `isCloseEnoughDist3D()`.
    pub fn is_close_enough_dist_3d(&self) -> bool {
        self.get_flag(IS_CLOSE_ENOUGH_DIST_3D)
    }
    /// `hasSuspension()`.
    pub fn has_suspension(&self) -> bool {
        self.template.has_suspension
    }
    /// `canMoveBackwards()`.
    pub fn can_move_backwards(&self) -> bool {
        self.template.can_move_backward
    }
    /// `getMaxWheelExtension()`.
    pub fn get_max_wheel_extension(&self) -> f32 {
        self.template.maximum_wheel_extension
    }
    /// `getMaxWheelCompression()`.
    pub fn get_max_wheel_compression(&self) -> f32 {
        self.template.maximum_wheel_compression
    }
    /// `getWheelTurnAngle()`.
    pub fn get_wheel_turn_angle(&self) -> f32 {
        self.template.wheel_turn_angle
    }
    /// `getRudderCorrectionDegree()`.
    pub fn get_rudder_correction_degree(&self) -> f32 {
        self.template.rudder_correction_degree
    }
    /// `getRudderCorrectionRate()`.
    pub fn get_rudder_correction_rate(&self) -> f32 {
        self.template.rudder_correction_rate
    }
    /// `getElevatorCorrectionDegree()`.
    pub fn get_elevator_correction_degree(&self) -> f32 {
        self.template.elevator_correction_degree
    }
    /// `getElevatorCorrectionRate()`.
    pub fn get_elevator_correction_rate(&self) -> f32 {
        self.template.elevator_correction_rate
    }
    /// `getWanderWidthFactor()`.
    pub fn get_wander_width_factor(&self) -> f32 {
        self.template.wander_width_factor
    }
    /// `getWanderAboutPointRadius()`.
    pub fn get_wander_about_point_radius(&self) -> f32 {
        self.template.wander_about_point_radius
    }

    // ---- setters ----

    /// `setMaxLift(lift)`: "handy for forcing helicopters to crash realistically: cut their lift".
    pub fn set_max_lift(&mut self, lift: f32) {
        self.max_lift = lift;
    }
    /// `setMaxSpeed(speed)`.
    pub fn set_max_speed(&mut self, speed: f32) {
        self.max_speed = speed;
    }
    /// `setMaxAcceleration(accel)`.
    pub fn set_max_acceleration(&mut self, accel: f32) {
        self.max_accel = accel;
    }
    /// `setMaxBraking(braking)`.
    pub fn set_max_braking(&mut self, braking: f32) {
        self.max_braking = braking;
    }
    /// `setMaxTurnRate(turn)`.
    pub fn set_max_turn_rate(&mut self, turn: f32) {
        self.max_turn_rate = turn;
    }
    /// `setAllowInvalidPosition(allow)`.
    pub fn set_allow_invalid_position(&mut self, allow: bool) {
        self.set_flag(ALLOW_INVALID_POSITION, allow);
    }
    /// `setCloseEnoughDist(dist)`.
    pub fn set_close_enough_dist(&mut self, dist: f32) {
        self.close_enough_dist = dist;
    }
    /// `setCloseEnoughDist3D(setting)`.
    pub fn set_close_enough_dist_3d(&mut self, setting: bool) {
        self.set_flag(IS_CLOSE_ENOUGH_DIST_3D, setting);
    }
    /// `isInvalidPositionAllowed()`.
    pub fn is_invalid_position_allowed(&self) -> bool {
        self.get_flag(ALLOW_INVALID_POSITION)
    }
    /// `setPreferredHeight(height)`.
    pub fn set_preferred_height(&mut self, height: f32) {
        self.preferred_height = height;
    }
    /// `setUsePreciseZPos(u)`: when on, do the best to reach the specified z
    /// even if it is not the preferred height (force missiles, takeoff, landing).
    pub fn set_use_precise_z_pos(&mut self, u: bool) {
        self.set_flag(PRECISE_Z_POS, u);
    }
    /// `setNoSlowDownAsApproachingDest(u)`: when on, units go full speed till
    /// the end and may overshoot.
    pub fn set_no_slow_down_as_approaching_dest(&mut self, u: bool) {
        self.set_flag(NO_SLOW_DOWN_AS_APPROACHING_DEST, u);
    }
    /// `setUltraAccurate(u)`: when on, cheat for very precise motion.
    pub fn set_ultra_accurate(&mut self, u: bool) {
        self.set_flag(ULTRA_ACCURATE, u);
    }
    /// `isUltraAccurate()`.
    pub fn is_ultra_accurate(&self) -> bool {
        self.get_flag(ULTRA_ACCURATE)
    }
    /// `isMovingBackwards()`.
    pub fn is_moving_backwards(&self) -> bool {
        self.get_flag(MOVING_BACKWARDS)
    }

    /// `startMove()`: "a move is starting", resets the donut timer.
    pub fn start_move(&mut self, frame: u32) {
        self.donut_timer = Self::donut_deadline(frame);
    }

    // ---- condition-dependent limits ----

    /// `getMaxSpeedForCondition(condition)`.
    pub fn get_max_speed_for_condition<G: LocomotorGlobals + ?Sized>(
        &self,
        g: &G,
        condition: BodyDamageType,
    ) -> f32 {
        let mut speed = if is_condition_better(condition, g.movement_penalty_damage_state()) {
            self.template.max_speed
        } else {
            self.template.max_speed_damaged
        };

        if speed > self.max_speed {
            speed = self.max_speed;
        }

        speed
    }

    /// `getMaxTurnRate(condition)`.
    pub fn get_max_turn_rate<G: LocomotorGlobals + ?Sized>(
        &self,
        g: &G,
        condition: BodyDamageType,
    ) -> f32 {
        let mut turn = if is_condition_better(condition, g.movement_penalty_damage_state()) {
            self.template.max_turn_rate
        } else {
            self.template.max_turn_rate_damaged
        };

        if turn > self.max_turn_rate {
            turn = self.max_turn_rate;
        }

        const TURN_FACTOR: f32 = 2.0;
        if self.get_flag(ULTRA_ACCURATE) {
            turn *= TURN_FACTOR; // monster turning ability
        }

        turn
    }

    /// `getMaxAcceleration(condition)`.
    pub fn get_max_acceleration<G: LocomotorGlobals + ?Sized>(
        &self,
        g: &G,
        condition: BodyDamageType,
    ) -> f32 {
        let mut accel = if is_condition_better(condition, g.movement_penalty_damage_state()) {
            self.template.acceleration
        } else {
            self.template.acceleration_damaged
        };

        if accel > self.max_accel {
            accel = self.max_accel;
        }

        accel
    }

    /// `getBraking()`.
    pub fn get_braking(&self) -> f32 {
        let mut braking = self.template.braking;

        if braking > self.max_braking {
            braking = self.max_braking;
        }

        braking
    }

    /// `getMaxLift(condition)`.
    pub fn get_max_lift<G: LocomotorGlobals + ?Sized>(
        &self,
        g: &G,
        condition: BodyDamageType,
    ) -> f32 {
        let mut lift = if is_condition_better(condition, g.movement_penalty_damage_state()) {
            self.template.lift
        } else {
            self.template.lift_damaged
        };

        if lift > self.max_lift {
            lift = self.max_lift;
        }

        lift
    }

    /// `calcMinTurnRadius(condition, &timeToTravelThatDist)`: the minimum turn
    /// radius, and the time to travel it (0 if it cannot move).
    pub fn calc_min_turn_radius<G: LocomotorGlobals + ?Sized>(
        &self,
        g: &G,
        condition: BodyDamageType,
    ) -> (f32, f32) {
        let min_speed = self.get_min_speed(); // in dist/frame
        let max_turn_rate = self.get_max_turn_rate(g, condition); // in rads/frame

        // if we can't turn, return a huge-but-finite radius rather than NAN...
        let min_turn_radius = if max_turn_rate > 0.0 {
            min_speed / max_turn_rate
        } else {
            BIGNUM
        };

        let time_to_travel_that_dist = if min_speed > 0.0 {
            min_turn_radius / min_speed
        } else {
            0.0
        };
        (min_turn_radius, time_to_travel_that_dist)
    }

    // ---- per-frame entry points ----

    /// `setPhysicsOptions(obj)`.
    pub fn set_physics_options<H: LocomotorHost>(&self, obj: &mut H) {
        if !obj.has_physics() {
            return;
        }

        // crank up the friction in ultra-accurate mode to increase movement precision.
        const EXTRA_FRIC: f32 = 0.5;
        let extra_extra_friction = if self.get_flag(ULTRA_ACCURATE) {
            EXTRA_FRIC
        } else {
            0.0
        };
        obj.set_extra_friction(self.template.extra_2d_friction + extra_extra_friction);
        // you'd think we wouldn't want friction in the air, but it's needed for realistic behavior.
        obj.set_allow_airborne_friction(self.get_apply_2d_friction_when_airborne());
        // walking guys aren't allowed to catch huge (or even small) air.
        obj.set_stick_to_ground(self.get_stick_to_ground());
    }

    /// `locoUpdate_moveTowardsAngle(obj, goalAngle)`.
    pub fn loco_update_move_towards_angle<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_angle: f32,
    ) {
        self.set_flag(MAINTAIN_POS_IS_VALID, false);

        if !obj.has_physics() {
            return;
        }

        // Skip moveTowardsAngle if physics say you're stunned
        if obj.is_stunned() {
            return;
        }

        let min_speed = self.get_min_speed();
        if min_speed > 0.0 {
            // can't stay in one place; move in the desired direction at min speed.
            let mut desired_pos = obj.position();
            desired_pos.x += trig::cos(goal_angle) * min_speed * 2.0;
            desired_pos.y += trig::sin(goal_angle) * min_speed * 2.0;
            // pass a huge num for "dist to goal", so that we don't think we're nearing
            // our destination and thus slow down...
            let on_path_dist_to_goal = 99999.0f32;
            let mut blocked = false;
            self.loco_update_move_towards_position(
                obj,
                &desired_pos,
                on_path_dist_to_goal,
                min_speed,
                &mut blocked,
            );

            // don't need to call handleBehaviorZ() here, since locoUpdate_moveTowardsPosition() will do so
        } else {
            let mut desired_pos = obj.position();
            desired_pos.x += trig::cos(goal_angle) * 1000.0;
            desired_pos.y += trig::sin(goal_angle) * 1000.0;
            let rotating = self.rotate_towards_position(obj, &desired_pos, None);
            obj.set_turning(rotating);
            let here = obj.position();
            self.handle_behavior_z(obj, &here);
        }
    }

    /// `rotateTowardsPosition(obj, goalPos, relAngle)`.
    fn rotate_towards_position<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        rel_angle: Option<&mut f32>,
    ) -> PhysicsTurningType {
        let bdt = obj.damage_state();
        let turn_rate = self.get_max_turn_rate(obj, bdt);

        self.rotate_obj_around_loco_pivot(obj, goal_pos, turn_rate, rel_angle)
    }

    /// `locoUpdate_moveTowardsPosition(obj, goalPos, onPathDistToGoal, desiredSpeed, &blocked)`.
    pub fn loco_update_move_towards_position<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        mut on_path_dist_to_goal: f32,
        mut desired_speed: f32,
        blocked: &mut bool,
    ) {
        self.set_flag(MAINTAIN_POS_IS_VALID, false);

        let bdt = obj.damage_state();
        let max_speed = self.get_max_speed_for_condition(obj, bdt);

        // sanity, we cannot use desired speed that is greater than our max speed we are capable of moving at
        if desired_speed > max_speed {
            desired_speed = max_speed;
        }

        let dist_to_stop_at_max_speed = (max_speed / self.get_braking()) * (max_speed) / 2.0;
        if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F
            && on_path_dist_to_goal > dist_to_stop_at_max_speed
        {
            self.set_flag(IS_BRAKING, false);
            self.braking_factor = 1.0;
        }
        if !obj.has_physics() {
            return;
        }

        // Skip moveTowardsPosition if physics say you're stunned
        if obj.is_stunned() {
            return;
        }

        //
        // do not allow for invalid positions that the pathfinder cannot handle ... for airborne
        // objects we don't need the pathfinder so we'll ignore this
        //
        if (self.template.surfaces & LOCOMOTORSURFACE_AIR) == 0 {
            let layer = obj.layer();
            let here = obj.position();
            // the pathfinder is asked first, whether or not invalid positions are allowed
            if !obj.valid_movement_terrain(layer, self, &here)
                && !self.get_flag(ALLOW_INVALID_POSITION)
            {
                // Somehow, we have gotten to an invalid location.
                if self.fix_invalid_position(obj) {
                    // the we adjusted us toward a legal position, so just return.
                    return;
                }
            }
        }

        // If the actual distance is farther, then use the actual distance so we get there.
        let here = obj.position();
        let mut dx = goal_pos.x - here.x;
        let mut dy = goal_pos.y - here.y;
        let mut dz = goal_pos.z - here.z;
        let mut dist = sqrt_d(dx * dx + dy * dy);
        if dist > on_path_dist_to_goal {
            if !obj.is_kind_of(KINDOF_PROJECTILE) && dist > 2.0 * on_path_dist_to_goal {
                self.set_flag(IS_BRAKING, true);
            }
            on_path_dist_to_goal = dist;
        }

        let mut treat_as_airborne = false;
        let pos = obj.position();
        let layer = obj.layer();
        let mut height_above_surface = pos.z - obj.layer_height(pos.x, pos.y, layer);

        if obj.status_test(OBJECT_STATUS_DECK_HEIGHT_OFFSET) {
            height_above_surface -= obj.carrier_deck_height();
        }

        if height_above_surface > -9.0f32 * obj.gravity() {
            // If we get high enough to stay up for 3 frames, then we left the ground.
            treat_as_airborne = true;
        }
        // We apply a zero acceleration to all units, as the call to
        // applyMotiveForce flags an object as being "driven" by a locomotor, rather
        // than being pushed around by objects bumping it.
        let null_accel = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        obj.apply_motive_force(&null_accel);

        if *blocked {
            if desired_speed > obj.velocity_magnitude() {
                *blocked = false;
            }
            if treat_as_airborne && (self.template.surfaces & LOCOMOTORSURFACE_AIR) != 0 {
                // Airborne flying objects don't collide for now.  jba.
                *blocked = false;
            }
        }

        if *blocked {
            obj.scrub_velocity_2d(desired_speed); // stop if we are about to run into the blocking object.
            let bdt = obj.damage_state();
            let turn_rate = self.get_max_turn_rate(obj, bdt);
            if self.template.wander_width_factor == 0.0 {
                *blocked = PhysicsTurningType::TurnNone
                    != self.rotate_obj_around_loco_pivot(obj, goal_pos, turn_rate, None);
            }

            // it is very important to be sure to call this in all situations, even if not moving in 2d space.
            self.handle_behavior_z(obj, goal_pos);
            return;
        }

        // srj sez: I don't know why we didn't want HOVERs to allow to "brake".
        // we actually really want them to, because it allows much more precise destination positioning.
        if self.template.appearance == LocomotorAppearance::LocoWings {
            self.set_flag(IS_BRAKING, false);
        }

        let was_braking = obj.status_test(OBJECT_STATUS_BRAKING);

        obj.set_turning(PhysicsTurningType::TurnNone);
        if self.get_allow_motive_force_while_airborne() || !treat_as_airborne {
            match self.template.appearance {
                LocomotorAppearance::LocoLegsTwo => self.move_towards_position_legs(
                    obj,
                    goal_pos,
                    on_path_dist_to_goal,
                    desired_speed,
                ),
                LocomotorAppearance::LocoClimber => self.move_towards_position_climb(
                    obj,
                    goal_pos,
                    on_path_dist_to_goal,
                    desired_speed,
                ),
                LocomotorAppearance::LocoWheelsFour | LocomotorAppearance::LocoMotorcycle => self
                    .move_towards_position_wheels(
                        obj,
                        goal_pos,
                        on_path_dist_to_goal,
                        desired_speed,
                    ),
                LocomotorAppearance::LocoTreads => self.move_towards_position_treads(
                    obj,
                    goal_pos,
                    on_path_dist_to_goal,
                    desired_speed,
                ),
                LocomotorAppearance::LocoHover => self.move_towards_position_hover(
                    obj,
                    goal_pos,
                    on_path_dist_to_goal,
                    desired_speed,
                ),
                LocomotorAppearance::LocoWings => self.move_towards_position_wings(
                    obj,
                    goal_pos,
                    on_path_dist_to_goal,
                    desired_speed,
                ),
                LocomotorAppearance::LocoThrust => self.move_towards_position_thrust(
                    obj,
                    goal_pos,
                    on_path_dist_to_goal,
                    desired_speed,
                ),
                LocomotorAppearance::LocoOther => self.move_towards_position_other(
                    obj,
                    goal_pos,
                    on_path_dist_to_goal,
                    desired_speed,
                ),
            }
        }

        self.handle_behavior_z(obj, goal_pos);
        // Objects that are braking don't follow the normal physics, so they end up at their destination exactly.
        obj.set_status(OBJECT_STATUS_BRAKING, self.get_flag(IS_BRAKING));

        if was_braking {
            // MIN_VEL = PATHFIND_CELL_SIZE_F/(LOGICFRAMES_PER_SECOND)
            let min_vel = PATHFIND_CELL_SIZE_F / (LOGICFRAMES_PER_SECOND as f32);

            let mut pos = obj.position();
            if obj.is_kind_of(KINDOF_PROJECTILE) {
                // Projectiles never stop braking once they start.  jba.
                obj.set_status(OBJECT_STATUS_BRAKING, true);
                // Projectiles cheat in 3 dimensions.
                dist = sqrt_d(dx * dx + dy * dy + dz * dz);
                let mut vel = obj.velocity_magnitude();
                if vel < min_vel {
                    vel = min_vel;
                }
                if vel > dist {
                    vel = dist; // do not overcompensate!
                }
                // Normalize.
                if dist > 0.001 {
                    dist = 1.0 / dist;
                    dx *= dist;
                    dy *= dist;
                    dz *= dist;
                    pos.x += dx * vel;
                    pos.y += dy * vel;
                    pos.z += dz * vel;
                }
            } else {
                // not projectiles only cheat in x & y.
                // Normalize.
                if dist > 0.001 {
                    let mut vel = obj.forward_speed_2d().abs();
                    if vel < min_vel {
                        vel = min_vel;
                    }
                    if vel > dist {
                        vel = dist; // do not overcompensate!
                    }
                    dist = 1.0 / dist;
                    dx *= dist;
                    dy *= dist;
                    pos.x += dx * vel;
                    pos.y += dy * vel;
                }
            }
            obj.set_position(&pos);
        }
    }

    /// `moveTowardsPositionTreads`.
    fn move_towards_position_treads<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        on_path_dist_to_goal: f32,
        mut desired_speed: f32,
    ) {
        // sanity, we cannot use desired speed that is greater than our max speed we are capable of moving at
        let bdt = obj.damage_state();
        let max_speed = self.get_max_speed_for_condition(obj, bdt);
        if desired_speed > max_speed {
            desired_speed = max_speed;
        }

        let max_acceleration = self.get_max_acceleration(obj, bdt);

        // Locomotion for treaded vehicles, ie tanks.

        //
        // Orient toward goal position
        //
        let mut rel_angle = f32::from_bits(UNWRITTEN_REL_ANGLE);
        let rotating = self.rotate_towards_position(obj, goal_pos, Some(&mut rel_angle));
        if rel_angle.to_bits() == UNWRITTEN_REL_ANGLE {
            // rotateObjAroundLocoPivot returned early (a pivoting tread within 0.1 of its goal)
            // without writing relAngle, which upstream then reads uninitialised
            self.undefined_inputs += 1;
            rel_angle = 0.0;
        }
        obj.set_turning(rotating);

        //
        // Modulate speed according to turning. The more we have to turn, the slower we go
        //
        const QUAETERPI: f32 = PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUAETERPI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        let here = obj.position();
        let dx = here.x - goal_pos.x;
        let dy = here.y - goal_pos.y;

        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;

        let actual_speed = obj.forward_speed_2d();
        let slow_down_time = actual_speed / self.get_braking();
        let slow_down_dist = (actual_speed / 1.50) * slow_down_time;

        // double: `angleCoeff > 0.05`
        if sqr(dx) + sqr(dy) < sqr(2.0 * PATHFIND_CELL_SIZE_F) && f64::from(angle_coeff) > 0.05f64 {
            goal_speed = actual_speed * 0.6;
        }

        if on_path_dist_to_goal < slow_down_dist
            && !self.get_flag(IS_BRAKING)
            && !self.get_flag(NO_SLOW_DOWN_AS_APPROACHING_DEST)
        {
            self.set_flag(IS_BRAKING, true);
            self.braking_factor = 1.1;
        }

        // double: `2.0*slowDownDist`
        if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F
            && f64::from(on_path_dist_to_goal) > 2.0f64 * f64::from(slow_down_dist)
        {
            self.set_flag(IS_BRAKING, false);
        }

        if self.get_flag(IS_BRAKING) {
            self.braking_factor = slow_down_dist / on_path_dist_to_goal;
            self.braking_factor *= self.braking_factor;
            if self.braking_factor > MAX_BRAKING_FACTOR {
                self.braking_factor = MAX_BRAKING_FACTOR;
            }
            if slow_down_dist > on_path_dist_to_goal {
                goal_speed = actual_speed - self.get_braking();
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else if slow_down_dist > on_path_dist_to_goal * 0.75 {
                goal_speed = actual_speed - self.get_braking() / 2.0;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else {
                goal_speed = actual_speed;
            }
        }

        //
        // Maintain goal speed
        //
        let speed_delta = goal_speed - actual_speed;
        if speed_delta != 0.0 {
            let mass = obj.mass();
            let acceleration = if speed_delta > 0.0 {
                max_acceleration
            } else {
                -self.braking_factor * self.get_braking()
            };
            let mut accel_force = mass * acceleration;

            /*
                don't accelerate/brake more than necessary. do a quick calc to
                see how much force we really need to achieve our goal speed...
            */
            let max_force_needed = mass * speed_delta;
            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            let dir = obj.unit_direction_vector_2d();

            let force = Coord3D {
                x: accel_force * dir.x,
                y: accel_force * dir.y,
                z: 0.0,
            };

            // apply forces to object
            obj.apply_motive_force(&force);
        }
    }

    /// `moveTowardsPositionWheels`.
    fn move_towards_position_wheels<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        on_path_dist_to_goal: f32,
        mut desired_speed: f32,
    ) {
        let bdt = obj.damage_state();
        let max_speed = self.get_max_speed_for_condition(obj, bdt);
        let max_turn_rate = self.get_max_turn_rate(obj, bdt);
        let max_acceleration = self.get_max_acceleration(obj, bdt);

        // sanity, we cannot use desired speed that is greater than our max speed we are capable of moving at
        if desired_speed > max_speed {
            desired_speed = max_speed;
        }

        // Locomotion for wheeled vehicles, ie trucks.
        //
        // See if we are turning.  If so, use the min turn speed.
        //
        let mut turn_speed = self.template.min_turn_speed;
        let angle = obj.orientation();
        let here = obj.position();
        let mut desired_angle = atan2_d(goal_pos.y - here.y, goal_pos.x - here.x);
        let mut rel_angle = std_angle_diff(desired_angle, angle);

        let mut move_backwards = false;

        // Wheeled vehicles can only turn while moving, so make sure the turn speed is reasonable.
        if turn_speed < max_speed / 4.0 {
            turn_speed = max_speed / 4.0;
        }

        let mut actual_speed = obj.forward_speed_2d();
        let mut do_3_point_turn = false;
        if actual_speed == 0.0 {
            self.set_flag(MOVING_BACKWARDS, false);
            if self.template.can_move_backward && rel_angle.abs() > PI / 2.0 {
                self.set_flag(MOVING_BACKWARDS, true);
                self.set_flag(
                    DOING_THREE_POINT_TURN,
                    on_path_dist_to_goal > 5.0 * obj.major_radius(),
                );
            }
        }
        if self.get_flag(MOVING_BACKWARDS) {
            if rel_angle.abs() < PI / 2.0 {
                move_backwards = false;
                self.set_flag(MOVING_BACKWARDS, false);
            } else {
                move_backwards = true;
                self.set_flag(
                    DOING_THREE_POINT_TURN,
                    on_path_dist_to_goal > 5.0 * obj.major_radius(),
                );
                do_3_point_turn = self.get_flag(DOING_THREE_POINT_TURN);
                if !do_3_point_turn {
                    desired_angle = std_angle_diff(desired_angle, PI);
                    rel_angle = std_angle_diff(desired_angle, angle);
                }
            }
        }

        const SMALL_TURN: f32 = PI / 20.0;
        if rel_angle.abs() > SMALL_TURN && desired_speed > turn_speed {
            desired_speed = turn_speed;
        }

        let mut goal_speed = desired_speed;
        if move_backwards {
            actual_speed = -actual_speed;
        }

        let slow_down_time = actual_speed / self.get_braking() + 1.0;
        let slow_down_dist = (actual_speed / 1.5) * slow_down_time + actual_speed;
        let mut effective_slow_down_dist = slow_down_dist;
        if effective_slow_down_dist < 1.0 * PATHFIND_CELL_SIZE {
            effective_slow_down_dist = 1.0 * PATHFIND_CELL_SIZE;
        }

        const FIFTEEN_DEGREES: f32 = PI / 12.0;
        // Project out 1/2 second.
        const PROJECT_FRAMES: f32 = (LOGICFRAMES_PER_SECOND / 2) as f32;
        if rel_angle.abs() > FIFTEEN_DEGREES {
            // If we're turning more than 10 degrees, check & see if we're moving into "impassable territory"
            let distance = PROJECT_FRAMES * (goal_speed + actual_speed) / 2.0;
            let mut target_angle = obj.orientation();
            let mut turn_factor = ((goal_speed + actual_speed) / 2.0) / turn_speed;
            if turn_factor > 1.0 {
                turn_factor = 1.0;
            }
            let turn_amount = PROJECT_FRAMES * turn_factor * max_turn_rate / 4.0;
            if rel_angle < 0.0 {
                target_angle -= turn_amount;
            } else {
                target_angle += turn_amount;
            }
            let offset = Coord3D {
                x: trig::cos(target_angle) * distance,
                y: trig::sin(target_angle) * distance,
                z: 0.0,
            };

            let pos = obj.position();

            let next_pos = Coord3D {
                x: pos.x + offset.x,
                y: pos.y + offset.y,
                z: pos.z,
            };

            let pos = obj.position();

            let half_pos = Coord3D {
                x: pos.x + offset.x / 2.0,
                y: pos.y + offset.y / 2.0,
                z: pos.z,
            };

            let layer = obj.layer();
            if !obj.valid_movement_terrain(layer, self, &half_pos) || {
                let layer = obj.layer();
                !obj.valid_movement_terrain(layer, self, &next_pos)
            } {
                let rotating = self.rotate_towards_position(obj, goal_pos, None);
                obj.set_turning(rotating);

                // apply a zero force to object so that it acts "driven"
                let force = Coord3D {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                };
                obj.apply_motive_force(&force);
                return;
            }
        }

        if on_path_dist_to_goal < effective_slow_down_dist
            && !self.get_flag(IS_BRAKING)
            && !self.get_flag(NO_SLOW_DOWN_AS_APPROACHING_DEST)
        {
            self.set_flag(IS_BRAKING, true);
            self.braking_factor = 1.1;
        }

        // double: `2.0*slowDownDist`
        if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F
            && f64::from(on_path_dist_to_goal) > 2.0f64 * f64::from(slow_down_dist)
        {
            self.set_flag(IS_BRAKING, false);
        }

        if on_path_dist_to_goal > DONUT_DISTANCE {
            self.donut_timer = Self::donut_deadline(obj.frame());
        } else if self.donut_timer < obj.frame() {
            self.set_flag(IS_BRAKING, true);
        }

        if self.get_flag(IS_BRAKING) {
            self.braking_factor = slow_down_dist / on_path_dist_to_goal;
            self.braking_factor *= self.braking_factor;
            if self.braking_factor > MAX_BRAKING_FACTOR {
                self.braking_factor = MAX_BRAKING_FACTOR;
            }
            self.braking_factor = 1.0;
            if slow_down_dist > on_path_dist_to_goal {
                goal_speed = actual_speed - self.get_braking();
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else if slow_down_dist > on_path_dist_to_goal * 0.75 {
                goal_speed = actual_speed - self.get_braking() / 2.0;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else {
                goal_speed = actual_speed;
            }
        }

        // Wheeled can only turn while moving.
        let mut turn_factor = actual_speed / turn_speed;
        if turn_factor < 0.0 {
            turn_factor = -turn_factor; // in case we're sliding backwards in a 3 pt turn.
        }
        if turn_factor > 1.0 {
            turn_factor = 1.0;
        }
        let turn_amount = turn_factor * max_turn_rate;

        let rotating = if move_backwards && !do_3_point_turn {
            let here = obj.position();
            let mut backward_pos = here;
            backward_pos.x += -(goal_pos.x - here.x);
            backward_pos.y += -(goal_pos.y - here.y);
            self.rotate_obj_around_loco_pivot(obj, &backward_pos, turn_amount, None)
        } else {
            self.rotate_obj_around_loco_pivot(obj, goal_pos, turn_amount, None)
        };

        obj.set_turning(rotating);

        //
        // Maintain goal speed
        //
        let mut speed_delta = goal_speed - actual_speed;
        if move_backwards {
            speed_delta = -goal_speed + actual_speed;
        }
        if speed_delta != 0.0 {
            let mass = obj.mass();
            let acceleration = if move_backwards {
                if speed_delta < 0.0 {
                    -max_acceleration
                } else {
                    self.braking_factor * self.get_braking()
                }
            } else if speed_delta > 0.0 {
                max_acceleration
            } else {
                -self.braking_factor * self.get_braking()
            };
            let mut accel_force = mass * acceleration;

            /*
                don't accelerate/brake more than necessary. do a quick calc to
                see how much force we really need to achieve our goal speed...
            */
            let max_force_needed = mass * speed_delta;
            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            let dir = obj.unit_direction_vector_2d();

            let force = Coord3D {
                x: accel_force * dir.x,
                y: accel_force * dir.y,
                z: 0.0,
            };

            // apply forces to object
            obj.apply_motive_force(&force);
        }
    }

    /// `fixInvalidPosition(obj, physics)`.
    fn fix_invalid_position<H: LocomotorHost>(&mut self, obj: &mut H) -> bool {
        if obj.is_kind_of(KINDOF_DOZER) {
            // don't fix him.
            return false;
        }
        let mut dx = 0i32;
        let mut dy = 0i32;
        for j in -1i32..2 {
            for i in -1i32..2 {
                let mut the_pos = obj.position();
                the_pos.x += (i as f32) * PATHFIND_CELL_SIZE_F;
                the_pos.y += (j as f32) * PATHFIND_CELL_SIZE_F;
                let layer = obj.layer();
                if !obj.valid_movement_terrain(layer, self, &the_pos) {
                    if i < 0 {
                        dx += 1;
                    }
                    if i > 0 {
                        dx -= 1;
                    }
                    if j < 0 {
                        dy += 1;
                    }
                    if j > 0 {
                        dy -= 1;
                    }
                }
            }
        }
        if dx != 0 || dy != 0 {
            let mass = obj.mass();
            // `dx*physics->getMass()/5`: int * float / int
            let correction = Coord3D {
                x: (dx as f32) * mass / 5.0,
                y: (dy as f32) * mass / 5.0,
                z: 0.0,
            };

            let mut correction_normalized = correction;
            correction_normalized.normalize();

            // Kill current velocity in the direction of the correction.
            let velocity = obj.velocity();
            let mut dot =
                (velocity.x * correction_normalized.x) + (velocity.y * correction_normalized.y);
            if dot > 0.25 {
                // It was already leaving.
                return false;
            }

            // Kill current accel
            if dot < 0.0 {
                dot = sqrt_d(-dot);
                correction_normalized.x *= dot * mass;
                correction_normalized.y *= dot * mass;
                obj.apply_motive_force(&correction_normalized);
            }

            // apply correction.
            obj.apply_motive_force(&correction);
            return true;
        }
        false
    }

    /// `moveTowardsPositionLegs`.
    fn move_towards_position_legs<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        on_path_dist_to_goal: f32,
        mut desired_speed: f32,
    ) {
        if self.get_is_downhill_only() && obj.position().z < goal_pos.z {
            return;
        }

        let bdt = obj.damage_state();
        let max_acceleration = self.get_max_acceleration(obj, bdt);

        // sanity, we cannot use desired speed that is greater than our max speed we are capable of moving at
        let bdt = obj.damage_state();
        let max_speed = self.get_max_speed_for_condition(obj, bdt);
        if desired_speed > max_speed {
            desired_speed = max_speed;
        }

        // Locomotion for infantry.
        //
        // Orient toward goal position
        //
        let actual_speed = obj.forward_speed_2d();
        let angle = obj.orientation();
        let here = obj.position();
        let mut desired_angle = atan2_d(goal_pos.y - here.y, goal_pos.x - here.x);

        if self.template.wander_width_factor != 0.0 {
            let angle_limit = PI / 8.0 * self.template.wander_width_factor;
            // This is the wander offline code - it forces the desired angle away from the goal, so we wander back & forth.  jba.
            if self.get_flag(OFFSET_INCREASING) {
                self.angle_offset += self.offset_increment * actual_speed;
                if self.angle_offset > angle_limit {
                    self.set_flag(OFFSET_INCREASING, false);
                }
            } else {
                self.angle_offset -= self.offset_increment * actual_speed;
                if self.angle_offset < -angle_limit {
                    self.set_flag(OFFSET_INCREASING, true);
                }
            }
            desired_angle = normalize_angle(desired_angle + self.angle_offset);
        }

        let rel_angle = std_angle_diff(desired_angle, angle);
        self.loco_update_move_towards_angle(obj, desired_angle);

        //
        // Modulate speed according to turning. The more we have to turn, the slower we go
        //
        const QUARTERPI: f32 = PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTERPI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;

        let slow_down_dist =
            calc_slow_down_dist(actual_speed, self.template.min_speed, self.get_braking());
        if on_path_dist_to_goal < slow_down_dist && !self.get_flag(NO_SLOW_DOWN_AS_APPROACHING_DEST)
        {
            goal_speed = self.template.min_speed;
        }

        //
        // Maintain goal speed
        //
        let speed_delta = goal_speed - actual_speed;
        if speed_delta != 0.0 {
            let mass = obj.mass();
            let acceleration = if speed_delta > 0.0 {
                max_acceleration
            } else {
                -self.get_braking()
            };
            let mut accel_force = mass * acceleration;

            /*
                don't accelerate/brake more than necessary. do a quick calc to
                see how much force we really need to achieve our goal speed...
            */
            let max_force_needed = mass * speed_delta;
            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            let dir = obj.unit_direction_vector_2d();

            let force = Coord3D {
                x: accel_force * dir.x,
                y: accel_force * dir.y,
                z: 0.0,
            };

            // apply forces to object
            obj.apply_motive_force(&force);
        }
    }

    /// `moveTowardsPositionClimb`.
    fn move_towards_position_climb<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        on_path_dist_to_goal: f32,
        mut desired_speed: f32,
    ) {
        let bdt = obj.damage_state();
        let max_acceleration = self.get_max_acceleration(obj, bdt);

        // sanity, we cannot use desired speed that is greater than our max speed we are capable of moving at
        let bdt = obj.damage_state();
        let max_speed = self.get_max_speed_for_condition(obj, bdt);
        if desired_speed > max_speed {
            desired_speed = max_speed;
        }

        // Locomotion for climbing infantry.

        let mut move_backwards = false;

        let pos = obj.position();

        let _dx = pos.x - goal_pos.x;
        let _dy = pos.y - goal_pos.y;
        let dz = pos.z - goal_pos.z;
        if dz * dz > sqr(PATHFIND_CELL_SIZE_F) {
            self.set_flag(CLIMBING, true);
        }
        if dz.abs() < 1.0 {
            self.set_flag(CLIMBING, false);
        }

        if self.get_flag(CLIMBING) {
            let mut delta = *goal_pos;
            delta.x -= pos.x;
            delta.y -= pos.y;
            delta.z = 0.0;
            delta.normalize();
            delta.x += pos.x;
            delta.y += pos.y;
            delta.z = obj.ground_height(delta.x, delta.y);
            // double: `pos.z-0.1`
            if f64::from(delta.z) < f64::from(pos.z) - 0.1f64 {
                move_backwards = true;
            }

            let mut ground_slope = (delta.z - pos.z).abs();
            if ground_slope < 1.0 {
                ground_slope = 1.0;
            }

            if ground_slope > 1.0 {
                desired_speed /= ground_slope * 4.0;
            }
        }
        self.set_flag(MOVING_BACKWARDS, move_backwards);

        //
        // Orient toward goal position
        //
        let angle = obj.orientation();
        let here = obj.position();
        let mut desired_angle = atan2_d(goal_pos.y - here.y, goal_pos.x - here.x);
        let mut rel_angle = std_angle_diff(desired_angle, angle);

        if move_backwards {
            desired_angle = std_angle_diff(desired_angle, PI);
            rel_angle = std_angle_diff(desired_angle, angle);
        }

        self.loco_update_move_towards_angle(obj, desired_angle);

        //
        // Modulate speed according to turning. The more we have to turn, the slower we go
        //
        const QUARTERPI: f32 = PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTERPI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;

        let mut actual_speed = obj.forward_speed_2d();

        if move_backwards {
            actual_speed = -actual_speed;
        }

        let slow_down_dist =
            calc_slow_down_dist(actual_speed, self.template.min_speed, self.get_braking());
        if on_path_dist_to_goal < slow_down_dist && !self.get_flag(NO_SLOW_DOWN_AS_APPROACHING_DEST)
        {
            goal_speed = self.template.min_speed;
        }

        //
        // Maintain goal speed
        //
        let mut speed_delta = goal_speed - actual_speed;
        if move_backwards {
            speed_delta = -goal_speed + actual_speed;
        }
        if speed_delta != 0.0 {
            let mass = obj.mass();
            let acceleration = if move_backwards {
                if speed_delta < 0.0 {
                    -max_acceleration
                } else {
                    self.get_braking()
                }
            } else if speed_delta > 0.0 {
                max_acceleration
            } else {
                -self.get_braking()
            };
            let mut accel_force = mass * acceleration;

            /*
                don't accelerate/brake more than necessary. do a quick calc to
                see how much force we really need to achieve our goal speed...
            */
            let max_force_needed = mass * speed_delta;
            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            let dir = obj.unit_direction_vector_2d();

            let force = Coord3D {
                x: accel_force * dir.x,
                y: accel_force * dir.y,
                z: 0.0,
            };

            // apply forces to object
            obj.apply_motive_force(&force);
        }
    }

    /// `moveTowardsPositionWings` (the `CIRCLE_FOR_LANDING` part is not compiled upstream).
    fn move_towards_position_wings<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        on_path_dist_to_goal: f32,
        desired_speed: f32,
    ) {
        // handle the 2D component.
        self.move_towards_position_other(obj, goal_pos, on_path_dist_to_goal, desired_speed);
    }

    /// `moveTowardsPositionHover`.
    fn move_towards_position_hover<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        on_path_dist_to_goal: f32,
        desired_speed: f32,
    ) {
        // handle the 2D component.
        self.move_towards_position_other(obj, goal_pos, on_path_dist_to_goal, desired_speed);

        // Only hover locomotors care about their OverWater special effects.  (OverWater also affects speed, so this is not a client thing)
        let new_position = obj.position();
        if obj.underwater(new_position.x, new_position.y).underwater {
            if !self.get_flag(OVER_WATER) {
                // Change my model condition because I used to not be over water, but now I am
                self.set_flag(OVER_WATER, true);
                obj.set_model_condition_state(MODELCONDITION_OVER_WATER);
            }
        } else if self.get_flag(OVER_WATER) {
            // Here, I was, but now I'm not
            self.set_flag(OVER_WATER, false);
            obj.clear_model_condition_state(MODELCONDITION_OVER_WATER);
        }
    }

    /// `moveTowardsPositionThrust`.
    fn move_towards_position_thrust<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        on_path_dist_to_goal: f32,
        desired_speed: f32,
    ) {
        let bdt = obj.damage_state();

        let mut max_forward_speed = self.get_max_speed_for_condition(obj, bdt);
        let mut desired_speed = clamp(self.template.min_speed, desired_speed, max_forward_speed);
        let actual_forward_speed = obj.forward_speed_3d();

        if self.get_braking() > 0.0 {
            let slow_down_dist = calc_slow_down_dist(
                actual_forward_speed,
                self.template.min_speed,
                self.get_braking(),
            );
            if on_path_dist_to_goal < slow_down_dist
                && !self.get_flag(NO_SLOW_DOWN_AS_APPROACHING_DEST)
            {
                desired_speed = self.template.min_speed;
            }
        }

        let mut local_goal_pos = *goal_pos;

        //out of the handleBehaviorZ() function
        let pos = obj.position();
        if self.preferred_height != 0.0 && !self.get_flag(PRECISE_Z_POS) {
            // If we have a preferred flight height, and we haven't been told explicitly to ignore it...
            let surface_ht = self.get_surface_ht_at_pt(obj, pos.x, pos.y);
            local_goal_pos.z = self.preferred_height + surface_ht;
            let mut delta = local_goal_pos.z - pos.z;
            delta *= self.get_preferred_height_damping();
            local_goal_pos.z = pos.z + delta;
        }

        let forward_dir = obj.transform_matrix().get_x_vector();

        // Maintain goal speed
        let forward_speed_delta = desired_speed - actual_forward_speed;
        let max_accel = if forward_speed_delta > 0.0 || self.get_braking() == 0.0 {
            self.get_max_acceleration(obj, bdt)
        } else {
            -self.get_braking()
        };
        let mut max_turn_rate = self.get_max_turn_rate(obj, bdt);

        // what direction do we need to thrust in, in order to reach the goalpos?
        let mut desired_thrust_dir = Vector3::default();
        calc_direction_to_apply_thrust(obj, &local_goal_pos, max_accel, &mut desired_thrust_dir);

        // we might not be able to thrust in that dir, so thrust as closely as we can
        let max_thrust_angle = if max_turn_rate > 0.0 {
            self.template.max_thrust_angle
        } else {
            0.0
        };
        let mut thrust_dir = Vector3::default();
        let thrust_angle = try_to_rotate_vector_3d(
            max_thrust_angle,
            &forward_dir,
            &desired_thrust_dir,
            &mut thrust_dir,
        );

        // note that we are trying to orient in the direction of our vel, not the dir of our thrust.
        if !is_nearly_zero(obj.velocity_magnitude()) {
            let veltmp = obj.velocity();
            let mut vel = Vector3::new(veltmp.x, veltmp.y, veltmp.z);
            let mut adjust = true;
            if obj.status_test(OBJECT_STATUS_BRAKING) {
                // align to target, cause that's where we're going anyway.

                vel.set(goal_pos.x - pos.x, goal_pos.y - pos.y, goal_pos.z - pos.z);
                if is_nearly_zero(sqr(vel.x) + sqr(vel.y) + sqr(vel.z)) {
                    // we are at target.
                    adjust = false;
                }
                max_turn_rate *= 3.0;
            }
            if adjust {
                try_to_orient_in_this_direction_3d(obj, max_turn_rate, &vel);
            }
        }

        if forward_speed_delta != 0.0 || thrust_angle != 0.0 {
            if max_forward_speed <= 0.0 {
                max_forward_speed = 0.01; // In some cases, this is 0, hack for now.  jba.
            }
            let damping = clamp(0.0f32, max_accel / max_forward_speed, 1.0f32);
            let v = obj.velocity();
            let cur_vel = Vector3::new(v.x, v.y, v.z);

            let accel_vec = thrust_dir * max_accel - cur_vel * damping;

            let mass = obj.mass();

            let force = Coord3D {
                x: mass * accel_vec.x,
                y: mass * accel_vec.y,
                z: mass * accel_vec.z,
            };

            // apply forces to object
            obj.apply_motive_force(&force);
        }
    }

    /// `getSurfaceHtAtPt(x, y)`.
    fn get_surface_ht_at_pt<H: LocomotorHost>(&self, obj: &mut H, x: f32, y: f32) -> f32 {
        let mut ht = 0.0f32;

        let w = obj.underwater(x, y);
        if w.underwater {
            ht += w.water_z.unwrap_or(0.0);
        } else {
            ht += w.terrain_z;
        }

        ht
    }

    /// `calcLiftToUseAtPt(obj, physics, curZ, surfaceAtPt, preferredHeight)`.
    fn calc_lift_to_use_at_pt<H: LocomotorHost>(
        &self,
        obj: &H,
        cur_z: f32,
        _surface_at_pt: f32,
        preferred_height: f32,
    ) -> f32 {
        /*
            take the classic equation:

                x = x0 + v*t + 0.5*a*t^2

            and solve for acceleration.
        */
        let bdt = obj.damage_state();
        let max_gross_lift = self.get_max_lift(obj, bdt);
        let gravity = obj.gravity();
        let mut max_net_lift = max_gross_lift + gravity; // note that gravity is always negative.
        if max_net_lift < 0.0 {
            max_net_lift = 0.0;
        }
        let cur_vel_z = obj.velocity().z;
        // going down, braking is limited by net lift; going up, braking is limited by gravity
        let max_accel = if self.get_flag(ULTRA_ACCURATE) {
            if cur_vel_z < 0.0 {
                2.0 * max_net_lift
            } else {
                -2.0 * max_net_lift
            }
        } else if cur_vel_z < 0.0 {
            max_net_lift
        } else {
            gravity
        };
        // see how far we need to slow to dead stop, given max braking
        let desired_accel;
        const TINY_ACCEL: f32 = 0.001;
        if max_accel.abs() > TINY_ACCEL {
            let delta_z = preferred_height - cur_z;
            // calc how far it will take for us to go from cur speed to zero speed, at max accel.
            // in theory, calcSlowDownDist is the correct calculation, but in practice,
            // doesn't work in some situations (eg, opening of USA01 map). Why, I dunno.
            // But for now I have gone back to the old, looks-incorrect-to-me-but-works calc. (srj)
            // double: `sqr(curVelZ) / fabs(maxAccel)` (`fabs` returns a double)
            let brake_dist = (f64::from(sqr(cur_vel_z)) / f64::from(max_accel.abs())) as f32;
            if brake_dist.abs() > delta_z.abs() {
                // if the dist-to-accel (or dist-to-brake) is further than the dist-to-go,
                // use the max accel.
                desired_accel = max_accel;
            } else if cur_vel_z.abs() > self.template.speed_limit_z {
                // or, if we're going too fast, limit it here.
                desired_accel = self.template.speed_limit_z - cur_vel_z;
            } else {
                // ok, figure out the correct accel to use to get us there at zero.
                //
                // dz = v t + 0.5 a t^2
                //  thus
                // a = 2(dz - v t)/t^2
                //  and
                // t = (-v +- sqrt(v*v + 2*a*dz))/a
                //
                // but if we assume t=1, then
                //  a=2(dz-v)
                // then, plug it back in and see if t is really 1...
                desired_accel = 2.0 * (delta_z - cur_vel_z);
            }
        } else {
            desired_accel = 0.0;
        }
        let mut lift_to_use = desired_accel - gravity;
        if self.get_flag(ULTRA_ACCURATE) {
            // in ultra-accurate mode, we allow cheating.
            const UP_FACTOR: f32 = 3.0;
            if lift_to_use > UP_FACTOR * max_gross_lift {
                lift_to_use = UP_FACTOR * max_gross_lift;
            }
            // srj sez: we used to clip lift to zero here (not allowing neg lift).
            // however, I now think that allowing neg lift in ultra-accurate mode is
            // a good and desirable thing; in particular, it enables jets to complete
            // "short" landings more accurately (previously they sometimes would "float"
            // down, which sucked.) if you need to bump this back to zero, check it carefully...
            else if lift_to_use < -max_gross_lift {
                lift_to_use = -max_gross_lift;
            }
        } else if lift_to_use > max_gross_lift {
            lift_to_use = max_gross_lift;
        } else if lift_to_use < 0.0 {
            lift_to_use = 0.0;
        }

        lift_to_use
    }

    /// `rotateObjAroundLocoPivot(obj, goalPos, maxTurnRate, relAngle)`.
    fn rotate_obj_around_loco_pivot<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        max_turn_rate: f32,
        rel_angle: Option<&mut f32>,
    ) -> PhysicsTurningType {
        let angle = obj.orientation();
        let mut offset = self.get_turn_pivot_offset();

        let turn;

        if self.get_flag(IS_BRAKING) {
            offset = 0.0; // When braking we do exact movement towards goal, instead of physics.
        }
        // Rotating about pivot moves the object, and can make us miss our goal, so it is disabled.  jba.
        if offset != 0.0 {
            let radius = obj.bounding_circle_radius();
            let turn_point_offset = offset * radius;

            let mut turn_pos = obj.position();
            let dir = obj.unit_direction_vector_2d();
            turn_pos.x += dir.x * turn_point_offset;
            turn_pos.y += dir.y * turn_point_offset;
            let dx = goal_pos.x - turn_pos.x;
            let dy = goal_pos.y - turn_pos.y;
            // If we are very close to the goal, we twitch due to rounding error.  So just return. jba.
            if dx.abs() < 0.1 && dy.abs() < 0.1 {
                return PhysicsTurningType::TurnNone;
            }
            let desired_angle = atan2_d(dy, dx);
            let mut amount = std_angle_diff(desired_angle, angle);
            if let Some(r) = rel_angle {
                *r = amount;
            }
            if amount > max_turn_rate {
                amount = max_turn_rate;
                turn = PhysicsTurningType::TurnPositive;
            } else if amount < -max_turn_rate {
                amount = -max_turn_rate;
                turn = PhysicsTurningType::TurnNegative;
            } else {
                turn = PhysicsTurningType::TurnNone;
            }

            let mut tmp = Matrix3D::identity();
            tmp.translate(turn_pos.x, turn_pos.y, 0.0);
            tmp.in_place_pre_rotate_z(amount);
            tmp.translate(-turn_pos.x, -turn_pos.y, 0.0);

            let mut mtx = Matrix3D::default();
            let cur = obj.transform_matrix();
            mtx.mul(&tmp, &cur);

            obj.set_transform_matrix(&mtx);
        } else {
            let here = obj.position();
            let desired_angle = atan2_d(goal_pos.y - here.y, goal_pos.x - here.x);
            let mut amount = std_angle_diff(desired_angle, angle);
            if let Some(r) = rel_angle {
                *r = amount;
            }
            if amount > max_turn_rate {
                amount = max_turn_rate;
                turn = PhysicsTurningType::TurnPositive;
            } else if amount < -max_turn_rate {
                amount = -max_turn_rate;
                turn = PhysicsTurningType::TurnNegative;
            } else {
                turn = PhysicsTurningType::TurnNone;
            }
            obj.set_orientation(normalize_angle(angle + amount));
        }
        turn
    }

    /// `handleBehaviorZ(obj, physics, goalPos)`: return true if the position
    /// cannot be maintained without being called every frame (hovering,
    /// circling); false if it can (eg, resting on the ground).
    fn handle_behavior_z<H: LocomotorHost>(&mut self, obj: &mut H, goal_pos: &Coord3D) -> bool {
        // keep the agent aligned on the terrain
        match self.template.behavior_z {
            LocomotorBehaviorZ::ZNoZMotiveForce => {
                // nothing to do.
                false
            }

            LocomotorBehaviorZ::ZSeaLevel => {
                if !obj.is_disabled_held() {
                    let mut pos = obj.position();
                    let w = obj.underwater(pos.x, pos.y);
                    if w.underwater {
                        pos.z = w.water_z.unwrap_or(0.0);
                    } else {
                        let layer = obj.layer();
                        pos.z = obj.layer_height(pos.x, pos.y, layer);
                    }
                    obj.set_position(&pos);
                }
                true
            }

            LocomotorBehaviorZ::ZFixedSurfaceRelativeHeight
            | LocomotorBehaviorZ::ZFixedAbsoluteHeight => {
                let mut pos = obj.position();
                let surface_rel =
                    self.template.behavior_z == LocomotorBehaviorZ::ZFixedSurfaceRelativeHeight;
                let surface_ht = if surface_rel {
                    self.get_surface_ht_at_pt(obj, pos.x, pos.y)
                } else {
                    0.0
                };
                pos.z = self.preferred_height + (if surface_rel { surface_ht } else { 0.0 });
                obj.set_position(&pos);
                true
            }

            LocomotorBehaviorZ::ZRelativeToGroundAndBuildings => {
                // srj sez: use getGroundOrStructureHeight(), because someday it will cache building heights...
                let mut pos = obj.position();
                let surface_ht = obj.ground_or_structure_height(pos.x, pos.y);

                pos.z = self.preferred_height + surface_ht;

                obj.set_position(&pos);
                true
            }

            LocomotorBehaviorZ::ZSmoothRelativeToHighestLayer => {
                if self.preferred_height != 0.0 || self.get_flag(PRECISE_Z_POS) {
                    let pos = obj.position();

                    // srj sez: if we aren't on the ground, never find the ground layer
                    let mut layer_at_dest = obj.layer();
                    if layer_at_dest == LAYER_GROUND {
                        layer_at_dest = obj.highest_layer_for_destination(&pos);
                    }

                    // return the height, even if off the edge of the bridge proper.
                    let surface_ht = obj.layer_height_unclipped(pos.x, pos.y, layer_at_dest);

                    let mut preferred_height = self.preferred_height + surface_ht;
                    if self.get_flag(PRECISE_Z_POS) {
                        preferred_height = goal_pos.z;
                    }

                    let mut delta = preferred_height - pos.z;
                    delta *= self.get_preferred_height_damping();
                    preferred_height = pos.z + delta;

                    let lift_to_use =
                        self.calc_lift_to_use_at_pt(obj, pos.z, surface_ht, preferred_height);

                    if lift_to_use != 0.0 {
                        let force = Coord3D {
                            x: 0.0,
                            y: 0.0,
                            z: lift_to_use * obj.mass(),
                        };
                        obj.apply_motive_force(&force);
                    }
                }
                true
            }

            LocomotorBehaviorZ::ZSurfaceRelativeHeight | LocomotorBehaviorZ::ZAbsoluteHeight => {
                if self.preferred_height != 0.0 || self.get_flag(PRECISE_Z_POS) {
                    let pos = obj.position();

                    let surface_rel =
                        self.template.behavior_z == LocomotorBehaviorZ::ZSurfaceRelativeHeight;
                    let surface_ht = if surface_rel {
                        self.get_surface_ht_at_pt(obj, pos.x, pos.y)
                    } else {
                        0.0
                    };
                    let mut preferred_height =
                        self.preferred_height + (if surface_rel { surface_ht } else { 0.0 });
                    if self.get_flag(PRECISE_Z_POS) {
                        preferred_height = goal_pos.z;
                    }

                    let mut delta = preferred_height - pos.z;
                    delta *= self.get_preferred_height_damping();
                    preferred_height = pos.z + delta;

                    let lift_to_use =
                        self.calc_lift_to_use_at_pt(obj, pos.z, surface_ht, preferred_height);

                    if lift_to_use != 0.0 {
                        let force = Coord3D {
                            x: 0.0,
                            y: 0.0,
                            z: lift_to_use * obj.mass(),
                        };
                        obj.apply_motive_force(&force);
                    }
                }
                true
            }
        }
    }

    /// `moveTowardsPositionOther`.
    fn move_towards_position_other<H: LocomotorHost>(
        &mut self,
        obj: &mut H,
        goal_pos: &Coord3D,
        on_path_dist_to_goal: f32,
        mut desired_speed: f32,
    ) {
        let bdt = obj.damage_state();
        let max_acceleration = self.get_max_acceleration(obj, bdt);

        // sanity, we cannot use desired speed that is greater than our max speed we are capable of moving at
        let max_speed = self.get_max_speed_for_condition(obj, bdt);
        if desired_speed > max_speed {
            desired_speed = max_speed;
        }

        let mut goal_speed = desired_speed;
        let actual_speed = obj.forward_speed_2d();

        // Locomotion for other things, ie don't know what it is jba :)
        //
        // Orient toward goal position
        // exception: if very close (ie, we could get there in 2 frames or less),
        // and ULTRA_ACCURATE, just slide into place
        //
        let pos = obj.position();
        let mut dir_to_apply_force = obj.unit_direction_vector_2d();

        if self.get_flag(ULTRA_ACCURATE)
            && (goal_pos.y - pos.y).abs()
                <= goal_speed * self.template.ultra_accurate_slide_into_place_factor
            && (goal_pos.x - pos.x).abs()
                <= goal_speed * self.template.ultra_accurate_slide_into_place_factor
        {
            // don't turn, just slide in the right direction
            obj.set_turning(PhysicsTurningType::TurnNone);
            dir_to_apply_force.x = goal_pos.x - pos.x;
            dir_to_apply_force.y = goal_pos.y - pos.y;
            dir_to_apply_force.z = 0.0;
            dir_to_apply_force.normalize();
        } else {
            let rotating = self.rotate_towards_position(obj, goal_pos, None);
            obj.set_turning(rotating);
        }

        if !self.get_flag(NO_SLOW_DOWN_AS_APPROACHING_DEST) {
            let slow_down_dist =
                calc_slow_down_dist(actual_speed, self.template.min_speed, self.get_braking());
            if on_path_dist_to_goal < slow_down_dist {
                goal_speed = self.template.min_speed;
            }
        }

        //
        // Maintain goal speed
        //
        let speed_delta = goal_speed - actual_speed;
        if speed_delta != 0.0 {
            let mass = obj.mass();
            let acceleration = if speed_delta > 0.0 {
                max_acceleration
            } else {
                -self.get_braking()
            };
            let mut accel_force = mass * acceleration;

            /*
                don't accelerate/brake more than necessary. do a quick calc to
                see how much force we really need to achieve our goal speed...
            */
            let max_force_needed = mass * speed_delta;
            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            let force = Coord3D {
                x: accel_force * dir_to_apply_force.x,
                y: accel_force * dir_to_apply_force.y,
                z: 0.0,
            };

            // apply forces to object
            obj.apply_motive_force(&force);
        }
    }

    /// `locoUpdate_maintainCurrentPosition(obj)`: return true if we can
    /// maintain the position without being called every frame (eg, we are
    /// resting on the ground), false if not (eg, we are hovering or circling).
    ///
    /// Note the name is upstream's and its meaning inverted from the return
    /// value's variable: the value is `requiresConstantCalling`.
    pub fn loco_update_maintain_current_position<H: LocomotorHost>(&mut self, obj: &mut H) -> bool {
        if !self.get_flag(MAINTAIN_POS_IS_VALID) {
            self.maintain_pos = obj.position();
            self.set_flag(MAINTAIN_POS_IS_VALID, true);
        }

        self.donut_timer = Self::donut_deadline(obj.frame());
        self.set_flag(IS_BRAKING, false);
        if !obj.has_physics() {
            return true;
        }

        let mut requires_constant_calling; // assume the worst.
        match self.template.appearance {
            LocomotorAppearance::LocoThrust => {
                self.maintain_current_position_thrust(obj);
                requires_constant_calling = true;
            }
            LocomotorAppearance::LocoLegsTwo | LocomotorAppearance::LocoClimber => {
                self.maintain_current_position_other(obj);
                requires_constant_calling = false;
            }
            LocomotorAppearance::LocoWheelsFour
            | LocomotorAppearance::LocoMotorcycle
            | LocomotorAppearance::LocoTreads => {
                self.maintain_current_position_other(obj);
                requires_constant_calling = false;
            }
            LocomotorAppearance::LocoHover => {
                self.maintain_current_position_hover(obj);
                requires_constant_calling = true;
            }
            LocomotorAppearance::LocoWings => {
                self.maintain_current_position_wings(obj);
                requires_constant_calling = true;
            }
            LocomotorAppearance::LocoOther => {
                self.maintain_current_position_other(obj);
                requires_constant_calling = true;
            }
        }

        // but we do need to do this even if not moving, for hovering/Thrusting things.
        let maintain_pos = self.maintain_pos;
        if self.handle_behavior_z(obj, &maintain_pos) {
            requires_constant_calling = true;
        }

        requires_constant_calling
    }

    /// `maintainCurrentPositionThrust`.
    fn maintain_current_position_thrust<H: LocomotorHost>(&mut self, obj: &mut H) {
        // should these also use the "circling radius" stuff, like wings?
        let maintain_pos = self.maintain_pos;
        let min_speed = self.get_min_speed();
        self.move_towards_position_thrust(obj, &maintain_pos, 0.0, min_speed);
    }

    /// `maintainCurrentPositionWings`.
    fn maintain_current_position_wings<H: LocomotorHost>(&mut self, obj: &mut H) {
        obj.set_turning(PhysicsTurningType::TurnNone);
        // no need to stop something that isn't moving (or is just sitting on the ground)
        if obj.is_motive() && obj.is_above_terrain() {
            // aim for the spot on the opposite side of the circle.
            let bdt = obj.damage_state();
            let mut turn_radius = self.template.circling_radius;
            if turn_radius == 0.0 {
                turn_radius = self.calc_min_turn_radius(obj, bdt).0;
            }

            // find the direction towards our "maintain pos"
            let pos = obj.position();
            let dx = self.maintain_pos.x - pos.x;
            let dy = self.maintain_pos.y - pos.y;
            let mut angle_toward_maintain_pos = if is_nearly_zero(dx) && is_nearly_zero(dy) {
                obj.orientation()
            } else {
                atan2_d(dy, dx)
            };

            let mut aim_dir = PI - PI / 8.0;
            if turn_radius < 0.0 {
                turn_radius = -turn_radius;
                aim_dir = -aim_dir;
            }
            angle_toward_maintain_pos += aim_dir;

            // project a spot "radius" dist away from it, in that dir
            let mut desired_pos = self.maintain_pos;
            desired_pos.x += trig::cos(angle_toward_maintain_pos) * turn_radius;
            desired_pos.y += trig::sin(angle_toward_maintain_pos) * turn_radius;
            let min_speed = self.template.min_speed;
            self.move_towards_position_wings(obj, &desired_pos, 0.0, min_speed);
        }
    }

    /// `maintainCurrentPositionHover`.
    fn maintain_current_position_hover<H: LocomotorHost>(&mut self, obj: &mut H) {
        obj.set_turning(PhysicsTurningType::TurnNone);
        if obj.is_motive() {
            // no need to stop something that isn't moving.
            let bdt = obj.damage_state();
            let max_acceleration = self.get_max_acceleration(obj, bdt);
            let actual_speed = obj.forward_speed_2d();
            //
            // Stop
            //
            let min_speed = crate::common::base_type::max(1.0E-10f32, self.template.min_speed);
            let speed_delta = min_speed - actual_speed;
            if speed_delta.abs() > min_speed {
                let mass = obj.mass();
                let acceleration = if speed_delta > 0.0 {
                    max_acceleration
                } else {
                    -self.get_braking()
                };
                let mut accel_force = mass * acceleration;

                /*
                    don't accelerate/brake more than necessary. do a quick calc to
                    see how much force we really need to achieve our goal speed...
                */
                let max_force_needed = mass * speed_delta;
                if accel_force.abs() > max_force_needed.abs() {
                    accel_force = max_force_needed;
                }

                let dir = obj.unit_direction_vector_2d();

                let force = Coord3D {
                    x: accel_force * dir.x,
                    y: accel_force * dir.y,
                    z: 0.0,
                };

                // apply forces to object
                obj.apply_motive_force(&force);
            }
        }
    }

    /// `maintainCurrentPositionOther` (also legs, wheels and treads).
    fn maintain_current_position_other<H: LocomotorHost>(&mut self, obj: &mut H) {
        obj.set_turning(PhysicsTurningType::TurnNone);
        if obj.is_motive() {
            // no need to stop something that isn't moving.
            obj.scrub_velocity_2d(0.0); // stop.
        }
    }

    // ---- snapshot ----

    /// `Locomotor::xfer`, version 2 (version 1 lacks the donut timer).
    pub fn xfer_state(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        const CURRENT_VERSION: XferVersion = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)?;

        if version >= 2 {
            xfer.xfer_unsigned_int(&mut self.donut_timer)?;
        }

        xfer.xfer_coord3d(&mut self.maintain_pos)?;
        xfer.xfer_real(&mut self.braking_factor)?;
        xfer.xfer_real(&mut self.max_lift)?;
        xfer.xfer_real(&mut self.max_speed)?;
        xfer.xfer_real(&mut self.max_accel)?;
        xfer.xfer_real(&mut self.max_braking)?;
        xfer.xfer_real(&mut self.max_turn_rate)?;
        xfer.xfer_real(&mut self.close_enough_dist)?;
        xfer.xfer_unsigned_int(&mut self.flags)?;
        xfer.xfer_real(&mut self.preferred_height)?;
        xfer.xfer_real(&mut self.preferred_height_damping)?;
        xfer.xfer_real(&mut self.angle_offset)?;
        xfer.xfer_real(&mut self.offset_increment)?;
        Ok(())
    }
}

impl Snapshot for Locomotor {
    /// `crc`: empty upstream.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.xfer_state(xfer)
    }

    /// `loadPostProcess`: empty upstream.
    fn load_post_process(&mut self) {}
}

// ---------------------------------------------------------------------------
// LocomotorSet
// ---------------------------------------------------------------------------

/// Why a [`LocomotorSet`] operation could not be done (inputs that crash or
/// throw upstream).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocomotorSetError {
    /// `newLocomotor(NULL)` (a template the store does not have).
    UnknownTemplate,
    /// `xfer` on load with a non-empty set (`XFER_LIST_NOT_EMPTY`).
    NotEmpty,
    /// A saved template name the store does not have (`XFER_UNKNOWN_STRING`).
    UnknownString(Vec<u8>),
    /// The xfer itself failed.
    Xfer(XferStatus),
}

impl From<XferStatus> for LocomotorSetError {
    fn from(s: XferStatus) -> Self {
        LocomotorSetError::Xfer(s)
    }
}

/// `class LocomotorSet`: the locomotors an object can use, one per kind of
/// surface at most worth choosing between.
#[derive(Clone, Debug, Default)]
pub struct LocomotorSet {
    /// `m_locomotors`.
    locomotors: Vec<Locomotor>,
    /// `m_validLocomotorSurfaces`.
    valid_locomotor_surfaces: LocomotorSurfaceTypeMask,
    /// `m_downhillOnly`.
    downhill_only: bool,
}

impl LocomotorSet {
    /// `LocomotorSet()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.locomotors.clear();
        self.valid_locomotor_surfaces = 0;
        self.downhill_only = false;
    }

    /// `addLocomotor(lt)`: a new `Locomotor` on the template of `namekey`
    /// (`TheLocomotorStore->newLocomotor(lt)`), appended; its surfaces join
    /// the valid ones, and a downhill-only one marks the set.
    pub fn add_locomotor(
        &mut self,
        store: &LocomotorStore,
        namekey: NameKeyType,
        rng: &mut RandomValues,
        frame: u32,
    ) -> Result<(), LocomotorSetError> {
        let loco = store
            .new_locomotor(namekey, rng, frame)
            .ok_or(LocomotorSetError::UnknownTemplate)?;
        self.valid_locomotor_surfaces |= loco.get_legal_surfaces();
        if loco.get_is_downhill_only() {
            self.downhill_only = true;
        }
        // else: Previous locos were gravity only, but this one isn't! (DEBUG_ASSERTCRASH only)
        self.locomotors.push(loco);
        Ok(())
    }

    /// `findLocomotor(t)`: the index of the first locomotor with a legal surface in `t`.
    pub fn find_locomotor(&self, t: LocomotorSurfaceTypeMask) -> Option<usize> {
        self.locomotors
            .iter()
            .position(|l| (l.get_legal_surfaces() & t) != 0)
    }

    /// The locomotor at `index` (what `findLocomotor` returned).
    pub fn get(&self, index: usize) -> Option<&Locomotor> {
        self.locomotors.get(index)
    }

    /// The locomotor at `index`, mutably.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Locomotor> {
        self.locomotors.get_mut(index)
    }

    /// `m_locomotors.size()`.
    pub fn len(&self) -> usize {
        self.locomotors.len()
    }

    /// Whether the set has no locomotors.
    pub fn is_empty(&self) -> bool {
        self.locomotors.is_empty()
    }

    /// `getValidSurfaces()`.
    pub fn get_valid_surfaces(&self) -> LocomotorSurfaceTypeMask {
        self.valid_locomotor_surfaces
    }

    /// `isDownhillOnly()`.
    pub fn is_downhill_only(&self) -> bool {
        self.downhill_only
    }

    /// `LocomotorSet::xfer`, version 1: the count, each locomotor as its
    /// template name and snapshot, the valid surfaces, the downhill flag. On
    /// load the set must be empty and the templates are looked up in `store`
    /// (`rng` and `frame` are the `Locomotor` constructor's, whose values the
    /// snapshot then overwrites).
    pub fn xfer_with_store(
        &mut self,
        xfer: &mut dyn Xfer,
        store: &LocomotorStore,
        names: &mut NameKeyGenerator,
        rng: &mut RandomValues,
        frame: u32,
    ) -> Result<(), LocomotorSetError> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)?;

        // count of vector
        let mut count = self.locomotors.len() as u16;
        xfer.xfer_unsigned_short(&mut count)?;

        // data
        if xfer.get_xfer_mode() == XferMode::Save {
            for loco in self.locomotors.iter_mut() {
                let mut name = loco.get_template_name().to_vec();
                xfer.xfer_ascii_string(&mut name)?;
                xfer.xfer_snapshot(loco)?;
            }
        } else if xfer.get_xfer_mode() == XferMode::Load {
            // vector should be empty at this point
            if !self.locomotors.is_empty() {
                return Err(LocomotorSetError::NotEmpty);
            }

            for _ in 0..count {
                let mut name = Vec::new();
                xfer.xfer_ascii_string(&mut name)?;

                let key = names
                    .try_name_to_key(&name)
                    .map_err(|_| LocomotorSetError::UnknownString(name.clone()))?;
                let mut loco = store
                    .new_locomotor(key, rng, frame)
                    .ok_or_else(|| LocomotorSetError::UnknownString(name.clone()))?;
                xfer.xfer_snapshot(&mut loco)?;
                self.locomotors.push(loco);
            }
        }

        xfer.xfer_int(&mut self.valid_locomotor_surfaces)?;
        xfer.xfer_bool(&mut self.downhill_only)?;
        Ok(())
    }

    /// `xferSelfAndCurLocoPtr(xfer, &loco)`: the set, then the template name of
    /// the current locomotor (given and returned as an index; empty name = none).
    pub fn xfer_self_and_cur_loco_ptr(
        &mut self,
        xfer: &mut dyn Xfer,
        store: &LocomotorStore,
        names: &mut NameKeyGenerator,
        rng: &mut RandomValues,
        frame: u32,
        loco: &mut Option<usize>,
    ) -> Result<(), LocomotorSetError> {
        self.xfer_with_store(xfer, store, names, rng, frame)?;

        if xfer.get_xfer_mode() == XferMode::Save {
            let mut name = Vec::new();
            if let Some(l) = loco.and_then(|i| self.locomotors.get(i)) {
                name = l.get_template_name().to_vec();
            }
            xfer.xfer_ascii_string(&mut name)?;
        } else if xfer.get_xfer_mode() == XferMode::Load {
            let mut name = Vec::new();
            xfer.xfer_ascii_string(&mut name)?;

            if name.is_empty() {
                *loco = None;
            } else {
                match self
                    .locomotors
                    .iter()
                    .position(|l| l.get_template_name() == name.as_slice())
                {
                    Some(i) => {
                        *loco = Some(i);
                        return Ok(());
                    }
                    None => return Err(LocomotorSetError::UnknownString(name)),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;
    use crate::logic::module::body_module::BODY_PRISTINE;

    /// What the host was asked to do.
    #[derive(Clone, Debug, PartialEq)]
    enum Call {
        Force(Coord3D),
        Scrub(f32),
        Turning(PhysicsTurningType),
        SetPos(Coord3D),
        SetStatus(ObjectStatusTypes, bool),
    }

    /// A flat world with one object that records what the locomotor does.
    struct Host {
        pos: Coord3D,
        angle: f32,
        vel: Coord3D,
        mass: f32,
        motive: bool,
        calls: Vec<Call>,
        frame: u32,
    }

    impl Host {
        fn new() -> Self {
            Host {
                pos: Coord3D {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                angle: 0.0,
                vel: Coord3D {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                mass: 2.0,
                motive: false,
                calls: Vec::new(),
                frame: 100,
            }
        }
        fn forces(&self) -> Vec<Coord3D> {
            self.calls
                .iter()
                .filter_map(|c| {
                    if let Call::Force(f) = c {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .collect()
        }
    }

    impl LocomotorGlobals for Host {
        fn gravity(&self) -> f32 {
            -1.0
        }
        fn movement_penalty_damage_state(&self) -> BodyDamageType {
            crate::logic::module::body_module::BODY_REALLYDAMAGED
        }
    }

    impl LocomotorHost for Host {
        fn has_physics(&self) -> bool {
            true
        }
        fn position(&self) -> Coord3D {
            self.pos
        }
        fn set_position(&mut self, pos: &Coord3D) {
            self.pos = *pos;
            self.calls.push(Call::SetPos(*pos));
        }
        fn orientation(&self) -> f32 {
            self.angle
        }
        fn set_orientation(&mut self, angle: f32) {
            self.angle = angle;
        }
        fn unit_direction_vector_2d(&mut self) -> Coord3D {
            Coord3D {
                x: trig::cos(self.angle),
                y: trig::sin(self.angle),
                z: 0.0,
            }
        }
        fn transform_matrix(&self) -> Matrix3D {
            let mut m = Matrix3D::identity();
            m.translate(self.pos.x, self.pos.y, self.pos.z);
            m.rotate_z(self.angle);
            m
        }
        fn set_transform_matrix(&mut self, mtx: &Matrix3D) {
            self.pos = Coord3D {
                x: mtx.get_x_translation(),
                y: mtx.get_y_translation(),
                z: mtx.get_z_translation(),
            };
            self.angle = mtx.get_z_rotation();
        }
        fn layer(&self) -> PathfindLayer {
            LAYER_GROUND
        }
        fn is_kind_of(&self, _kind: KindOfType) -> bool {
            false
        }
        fn status_test(&self, _status: ObjectStatusTypes) -> bool {
            false
        }
        fn set_status(&mut self, status: ObjectStatusTypes, set: bool) {
            self.calls.push(Call::SetStatus(status, set));
        }
        fn carrier_deck_height(&self) -> f32 {
            0.0
        }
        fn major_radius(&self) -> f32 {
            5.0
        }
        fn bounding_circle_radius(&self) -> f32 {
            5.0
        }
        fn is_disabled_held(&self) -> bool {
            false
        }
        fn set_model_condition_state(&mut self, _c: ModelConditionFlagType) {}
        fn clear_model_condition_state(&mut self, _c: ModelConditionFlagType) {}
        fn is_above_terrain(&mut self) -> bool {
            self.pos.z > 0.0
        }
        fn damage_state(&self) -> BodyDamageType {
            BODY_PRISTINE
        }
        fn is_stunned(&self) -> bool {
            false
        }
        fn apply_motive_force(&mut self, force: &Coord3D) {
            self.calls.push(Call::Force(*force));
        }
        fn scrub_velocity_2d(&mut self, desired_speed: f32) {
            self.calls.push(Call::Scrub(desired_speed));
        }
        fn set_turning(&mut self, turning: PhysicsTurningType) {
            self.calls.push(Call::Turning(turning));
        }
        fn velocity(&self) -> Coord3D {
            self.vel
        }
        fn velocity_magnitude(&self) -> f32 {
            self.vel.length()
        }
        fn forward_speed_2d(&self) -> f32 {
            let d = Coord3D {
                x: trig::cos(self.angle),
                y: trig::sin(self.angle),
                z: 0.0,
            };
            self.vel.x * d.x + self.vel.y * d.y
        }
        fn forward_speed_3d(&self) -> f32 {
            self.vel.length()
        }
        fn mass(&self) -> f32 {
            self.mass
        }
        fn is_motive(&self) -> bool {
            self.motive
        }
        fn set_extra_friction(&mut self, _f: f32) {}
        fn set_allow_airborne_friction(&mut self, _a: bool) {}
        fn set_stick_to_ground(&mut self, _s: bool) {}
        fn frame(&self) -> u32 {
            self.frame
        }
        fn ground_height(&mut self, _x: f32, _y: f32) -> f32 {
            0.0
        }
        fn layer_height(&mut self, _x: f32, _y: f32, _layer: PathfindLayer) -> f32 {
            0.0
        }
        fn layer_height_unclipped(&mut self, _x: f32, _y: f32, _layer: PathfindLayer) -> f32 {
            0.0
        }
        fn underwater(&mut self, _x: f32, _y: f32) -> Underwater {
            Underwater {
                underwater: false,
                water_z: None,
                terrain_z: 0.0,
            }
        }
        fn highest_layer_for_destination(&mut self, _pos: &Coord3D) -> PathfindLayer {
            LAYER_GROUND
        }
        fn ground_or_structure_height(&mut self, _x: f32, _y: f32) -> f32 {
            0.0
        }
        fn valid_movement_terrain(
            &mut self,
            _layer: PathfindLayer,
            _loco: &Locomotor,
            _pos: &Coord3D,
        ) -> bool {
            true
        }
    }

    struct Ctx {
        store: LocomotorStore,
        names: NameKeyGenerator,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store
            .parse_locomotor_template_definition(ini, &mut c.names)
    }

    static BLOCKS: [BlockParse<Ctx>; 1] = [BlockParse {
        token: "Locomotor",
        parse: block,
    }];

    fn load_as(text: &[u8], ty: IniLoadType, c: &mut Ctx) -> IniResult<()> {
        Ini::load(b"l.ini", text, ty, None, &BLOCKS, c)
    }

    fn ctx() -> Ctx {
        let mut c = Ctx {
            store: LocomotorStore::new(),
            names: NameKeyGenerator::new(),
        };
        c.names.init();
        c
    }

    fn rng() -> RandomValues {
        let mut r = RandomValues::default();
        r.init_game_logic_random(7);
        r
    }

    const TANK: &[u8] = b"Locomotor Tank\n Surfaces = GROUND RUBBLE\n Speed = 30\n SpeedDamaged = 15\n TurnRate = 90\n Acceleration = 60\n Braking = 120\n Appearance = TREADS\n MinSpeed = 0\n Extra2DFriction = 3\nEnd\n";

    #[test]
    fn template_ini_units_and_damaged_defaults() {
        let mut c = ctx();
        load_as(TANK, IniLoadType::Overwrite, &mut c).unwrap();
        let key = c.names.name_to_key(b"Tank");
        let t = &c.store.find_locomotor_template(key).unwrap().data;
        assert_eq!(t.name, b"Tank");
        assert_eq!(
            t.surfaces,
            LOCOMOTORSURFACE_GROUND | LOCOMOTORSURFACE_RUBBLE
        );
        assert_eq!(t.appearance, LocomotorAppearance::LocoTreads);
        // per second to per frame, as the INI converters do
        assert_eq!(
            t.max_speed,
            crate::common::game_common::convert_velocity_in_secs_to_frames(30.0)
        );
        assert_eq!(
            t.max_speed_damaged,
            crate::common::game_common::convert_velocity_in_secs_to_frames(15.0)
        );
        // not given: "damaged" copies the undamaged value
        assert_eq!(t.max_turn_rate_damaged, t.max_turn_rate);
        assert_eq!(t.acceleration_damaged, t.acceleration);
        assert_eq!(t.lift_damaged, t.lift);
        assert_eq!(t.extra_2d_friction, 3.0 * SECONDS_PER_LOGICFRAME_REAL);
        // untouched defaults
        assert_eq!(t.min_turn_speed, BIGNUM);
        assert_eq!(t.pitch_stiffness, 0.1);
        assert_eq!(t.airborne_targeting_height, i32::MAX);
    }

    #[test]
    fn validate_heals_wings_and_thrust_and_rejects_thrust_with_lift() {
        let mut t = LocomotorTemplate::new();
        t.appearance = LocomotorAppearance::LocoWings;
        t.validate().unwrap();
        assert_eq!((t.min_speed, t.min_turn_speed), (0.01, BIGNUM));
        t.min_turn_speed = 0.0;
        t.min_speed = -1.0;
        t.validate().unwrap();
        assert_eq!((t.min_speed, t.min_turn_speed), (0.01, 0.01));

        let mut t = LocomotorTemplate::new();
        t.appearance = LocomotorAppearance::LocoThrust;
        t.validate().unwrap();
        assert_eq!(
            (t.max_speed, t.max_speed_damaged, t.min_speed),
            (0.01, 0.01, 0.01)
        );
        t.lift = 1.0;
        assert_eq!(t.validate().unwrap_err().kind, IniErrorKind::InvalidData);
        let mut t = LocomotorTemplate::new();
        t.appearance = LocomotorAppearance::LocoThrust;
        t.behavior_z = LocomotorBehaviorZ::ZSeaLevel;
        assert_eq!(t.validate().unwrap_err().kind, IniErrorKind::InvalidData);
    }

    #[test]
    fn ini_errors_and_overrides() {
        let mut c = ctx();
        // a THRUST block with lift is rejected, and the template is not stored
        let r = load_as(
            b"Locomotor T\n Appearance = THRUST\n Lift = 5\nEnd\n",
            IniLoadType::Overwrite,
            &mut c,
        );
        assert_eq!(r.unwrap_err().kind, IniErrorKind::InvalidData);
        assert!(c.store.is_empty());
        // an unknown field
        let r = load_as(
            b"Locomotor T\n Wibble = 5\nEnd\n",
            IniLoadType::Overwrite,
            &mut c,
        );
        assert_eq!(r.unwrap_err().kind, IniErrorKind::UnknownToken);
        assert!(c.store.is_empty());

        load_as(TANK, IniLoadType::Overwrite, &mut c).unwrap();
        // an override load hangs a copy off the final override and fills it
        load_as(
            b"Locomotor Tank\n Speed = 60\nEnd\n",
            IniLoadType::CreateOverrides,
            &mut c,
        )
        .unwrap();
        let key = c.names.name_to_key(b"Tank");
        let head = c.store.find_locomotor_template(key).unwrap();
        assert_eq!(
            head.data.max_speed,
            crate::common::game_common::convert_velocity_in_secs_to_frames(30.0)
        );
        let fin = &head.get_final_override().data;
        assert_eq!(
            fin.max_speed,
            crate::common::game_common::convert_velocity_in_secs_to_frames(60.0)
        );
        // everything not mentioned is the copied value
        assert_eq!(
            fin.max_speed_damaged,
            crate::common::game_common::convert_velocity_in_secs_to_frames(15.0)
        );
        let loco = c.store.new_locomotor(key, &mut rng(), 0).unwrap();
        assert_eq!(loco.template().max_speed, fin.max_speed);
        // reset removes the override
        c.store.reset();
        let head = c.store.find_locomotor_template(key).unwrap();
        assert!(head.get_next_override().is_none());
        // a template created by an override load disappears with it
        load_as(
            b"Locomotor Other\nEnd\n",
            IniLoadType::CreateOverrides,
            &mut c,
        )
        .unwrap();
        assert_eq!(c.store.len(), 2);
        c.store.reset();
        assert_eq!(c.store.len(), 1);
    }

    #[test]
    fn constructor_draws_three_logic_randoms_and_sets_the_donut_timer() {
        let mut c = ctx();
        load_as(b"Locomotor W\n WanderLengthFactor = 2\n CloseEnoughDist = 4\n CloseEnoughDist3D = Yes\n PreferredHeight = 9\nEnd\n", IniLoadType::Overwrite, &mut c).unwrap();
        let key = c.names.name_to_key(b"W");
        let mut r = rng();
        let loco = c.store.new_locomotor(key, &mut r, 1000).unwrap();
        let mut expect = rng();
        let a = expect.game_logic_random_value_real(-PI / 6.0, PI / 6.0);
        let inc = (PI / 40.0) * (expect.game_logic_random_value_real(0.8, 1.2) / 2.0);
        let up = expect.game_logic_random_value(0, 1) != 0;
        assert_eq!(loco.angle_offset().to_bits(), a.to_bits());
        assert_eq!(loco.offset_increment().to_bits(), inc.to_bits());
        assert_eq!(loco.get_flag(OFFSET_INCREASING), up);
        assert_eq!(r, expect);
        assert_eq!(loco.donut_timer(), 1075);
        assert_eq!(loco.get_close_enough_dist(), 4.0);
        assert!(loco.is_close_enough_dist_3d());
        assert_eq!(loco.get_preferred_height(), 9.0);
        // a fresh locomotor has no limits
        assert_eq!(loco.get_braking(), BIGNUM);
        // a frame beyond 2^24 loses bits in the float sum, as upstream's does
        assert_eq!(Locomotor::donut_deadline(16_777_217), 16_777_292);
    }

    fn make(c: &mut Ctx, text: &[u8], name: &[u8]) -> Locomotor {
        load_as(text, IniLoadType::Overwrite, c).unwrap();
        let key = c.names.name_to_key(name);
        c.store.new_locomotor(key, &mut rng(), 100).unwrap()
    }

    #[test]
    fn treads_accelerate_toward_the_goal_and_turn_first() {
        let mut c = ctx();
        let mut loco = make(&mut c, TANK, b"Tank");
        let mut h = Host::new();
        let mut blocked = false;
        // goal straight ahead: full acceleration along +x, no turning
        loco.loco_update_move_towards_position(
            &mut h,
            &Coord3D {
                x: 500.0,
                y: 0.0,
                z: 0.0,
            },
            500.0,
            1000.0,
            &mut blocked,
        );
        let forces = h.forces();
        // a null force flags the object as driven, then the real one
        assert_eq!(
            forces[0],
            Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0
            }
        );
        let accel = crate::common::game_common::convert_acceleration_in_secs_to_frames(60.0);
        assert_eq!(
            forces[1],
            Coord3D {
                x: h.mass * accel,
                y: 0.0,
                z: 0.0
            }
        );
        assert!(h
            .calls
            .contains(&Call::Turning(PhysicsTurningType::TurnNone)));
        assert!(!blocked);
        // goal behind: the tank turns by its turn rate and, being > 45 degrees off, does not accelerate
        let mut h = Host::new();
        loco.loco_update_move_towards_position(
            &mut h,
            &Coord3D {
                x: -500.0,
                y: 1.0,
                z: 0.0,
            },
            500.0,
            1000.0,
            &mut blocked,
        );
        assert!(h
            .calls
            .contains(&Call::Turning(PhysicsTurningType::TurnPositive)));
        assert_eq!(h.forces().len(), 1);
        assert_eq!(h.angle, crate::common::game_common::convert_angular_velocity_in_degrees_per_sec_to_rads_per_frame(90.0));
    }

    #[test]
    fn maintain_position_on_the_ground_scrubs_and_needs_no_calling() {
        let mut c = ctx();
        let mut loco = make(&mut c, TANK, b"Tank");
        let mut h = Host::new();
        h.motive = true;
        h.pos = Coord3D {
            x: 3.0,
            y: 4.0,
            z: 0.0,
        };
        assert!(!loco.loco_update_maintain_current_position(&mut h));
        assert!(h.calls.contains(&Call::Scrub(0.0)));
        assert_eq!(
            loco.maintain_pos(),
            Coord3D {
                x: 3.0,
                y: 4.0,
                z: 0.0
            }
        );
        // moving clears the validity of the maintain position
        let mut blocked = false;
        loco.loco_update_move_towards_position(
            &mut h,
            &Coord3D {
                x: 9.0,
                y: 4.0,
                z: 0.0,
            },
            6.0,
            1.0,
            &mut blocked,
        );
        assert!(!loco.get_flag(MAINTAIN_POS_IS_VALID));
        // hover and other things need constant calling
        let mut c = ctx();
        let mut hover = make(&mut c, b"Locomotor H\n Appearance = HOVER\n Speed = 30\n Acceleration = 30\n Braking = 30\nEnd\n", b"H");
        assert!(hover.loco_update_maintain_current_position(&mut h));
    }

    #[test]
    fn helicopter_lift_follows_the_preferred_height() {
        let mut c = ctx();
        let mut loco = make(
            &mut c,
            b"Locomotor Heli\n Appearance = OTHER\n ZAxisBehavior = SURFACE_RELATIVE_HEIGHT\n PreferredHeight = 20\n Lift = 300000\n Speed = 60\n Acceleration = 30\n Braking = 30\nEnd\n",
            b"Heli",
        );
        let mut h = Host::new();
        h.pos.z = 5.0;
        assert!(loco.loco_update_maintain_current_position(&mut h));
        let lift = h
            .forces()
            .into_iter()
            .find(|f| f.z != 0.0)
            .expect("a lift force");
        assert!(lift.z > 0.0 && lift.x == 0.0 && lift.y == 0.0);
        // fixed height: the position is written, no force
        let mut c = ctx();
        let mut fixed = make(
            &mut c,
            b"Locomotor F\n ZAxisBehavior = FIXED_ABSOLUTE_HEIGHT\n PreferredHeight = 7\nEnd\n",
            b"F",
        );
        let mut h = Host::new();
        assert!(fixed.loco_update_maintain_current_position(&mut h));
        assert_eq!(h.pos.z, 7.0);
    }

    #[test]
    fn locomotor_set_finds_by_surface_and_round_trips_through_xfer() {
        let mut c = ctx();
        load_as(TANK, IniLoadType::Overwrite, &mut c).unwrap();
        load_as(
            b"Locomotor Boat\n Surfaces = WATER\n Appearance = HOVER\n DownhillOnly = No\nEnd\n",
            IniLoadType::Overwrite,
            &mut c,
        )
        .unwrap();
        let tank = c.names.name_to_key(b"Tank");
        let boat = c.names.name_to_key(b"Boat");
        let mut r = rng();
        let mut set = LocomotorSet::new();
        set.add_locomotor(&c.store, tank, &mut r, 10).unwrap();
        set.add_locomotor(&c.store, boat, &mut r, 10).unwrap();
        assert_eq!(
            set.add_locomotor(&c.store, NameKeyType(999_999), &mut r, 10),
            Err(LocomotorSetError::UnknownTemplate)
        );
        assert_eq!(
            set.get_valid_surfaces(),
            LOCOMOTORSURFACE_GROUND | LOCOMOTORSURFACE_RUBBLE | LOCOMOTORSURFACE_WATER
        );
        assert_eq!(set.find_locomotor(LOCOMOTORSURFACE_WATER), Some(1));
        assert_eq!(
            set.find_locomotor(LOCOMOTORSURFACE_GROUND | LOCOMOTORSURFACE_WATER),
            Some(0)
        );
        assert_eq!(set.find_locomotor(LOCOMOTORSURFACE_AIR), None);
        assert!(!set.is_downhill_only());
        set.get_mut(0).unwrap().set_max_speed(1.5);

        let mut save = XferSave::new();
        save.open(b"t").unwrap();
        let mut cur = Some(1usize);
        let mut r2 = rng();
        set.xfer_self_and_cur_loco_ptr(&mut save, &c.store, &mut c.names, &mut r2, 0, &mut cur)
            .unwrap();
        save.close().unwrap();
        let bytes = save.bytes().to_vec();

        let mut load = XferLoad::new(bytes.clone());
        load.open(b"t").unwrap();
        let mut back = LocomotorSet::new();
        let mut cur2 = None;
        back.xfer_self_and_cur_loco_ptr(&mut load, &c.store, &mut c.names, &mut r2, 0, &mut cur2)
            .unwrap();
        load.close().unwrap();
        assert_eq!(cur2, Some(1));
        assert_eq!(back.len(), 2);
        assert_eq!(back.get_valid_surfaces(), set.get_valid_surfaces());
        for i in 0..2 {
            let (a, b) = (set.get(i).unwrap(), back.get(i).unwrap());
            assert_eq!(a.get_template_name(), b.get_template_name());
            assert_eq!(a.donut_timer(), b.donut_timer());
            assert_eq!(a.angle_offset().to_bits(), b.angle_offset().to_bits());
            assert_eq!(a.flags(), b.flags());
        }
        assert_eq!(back.get(0).unwrap().max_speed, 1.5);

        // loading into a non-empty set is refused, as is a name the store lacks
        let mut load = XferLoad::new(bytes);
        load.open(b"t").unwrap();
        let mut cur3 = None;
        assert_eq!(
            set.xfer_self_and_cur_loco_ptr(
                &mut load,
                &c.store,
                &mut c.names,
                &mut r2,
                0,
                &mut cur3
            ),
            Err(LocomotorSetError::NotEmpty)
        );
    }
}
