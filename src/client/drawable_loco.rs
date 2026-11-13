// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp
//   (DrawableLocoInfo::DrawableLocoInfo, Drawable::applyPhysicsXform, calcPhysicsXform,
//   calcPhysicsXformThrust, calcPhysicsXformHoverOrWings, calcPhysicsXformTreads,
//   calcPhysicsXformWheels; GitHub #41)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Drawable.h
//   (TWheelInfo, DrawableLocoInfo, PhysicsXformInfo, getWheelInfo)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! A drawable's client-side physics (GitHub #41): the suspension, pitch, roll and bounce a drawable fakes from its
//! object's locomotor and physics (`m_locoInfo`), and the transform it puts on the drawn model
//! ([`Drawable::apply_physics_xform`]). The truck draw modules read the wheel part of it
//! ([`Drawable::get_wheel_info`]).
//!
//! # As upstream reads it
//!
//! - **`Drawable::draw`** calls `applyPhysicsXform(&transformMtx)` before the draw modules' `doDrawModule`: nothing
//!   without an object, when it is `DISABLED_HELD`, or without `TheGlobalData->m_showClientPhysics`; otherwise
//!   `calcPhysicsXform` by the current locomotor's appearance, and, when one ran, the drawable's transform translated
//!   by the total Z, then turned about Y by the pitch, about X by minus the roll and about Z by the yaw
//!   ([`PhysicsXformInfo::apply`]). A hidden drawable is not drawn at all (`m_hidden`), so none of this runs.
//! - **`calcPhysicsXform`**: without an AI or a current locomotor, nothing; `LOCO_WHEELS_FOUR`, `LOCO_TREADS`,
//!   `LOCO_HOVER`/`LOCO_WINGS` and `LOCO_THRUST` each have their own spring and damper model, kept in the drawable's
//!   `DrawableLocoInfo` (made on the first call); a total under 1e-20 in size is flushed to 0 (the "denormal hotfix").
//!   Each model returns early, its totals 0, without physics; the transform is still applied (upstream's
//!   `hasPhysicsXform` is set by the appearance alone).
//! - **Wheels** ([`DrawableLocoInfo::calc_wheels`]): pitch and roll spring back towards the ground's (the terrain
//!   normal's dot products with the direction and its perpendicular, times π/2); above a tenth of the locomotor's
//!   speed, when the rates are small, a random bounce kick (`GameClientRandomValue(0, 3)`, the client's generator); the
//!   acceleration's forward and lateral parts kick the acceleration pitch and roll while motive; each wheel's height
//!   offset follows the pitch and roll heights (damped by half when it drops, never below the maximum extension), and
//!   the wheel angle moves a tenth of the way to the turn angle each draw (reversed when moving backwards). In the air,
//!   the rear wheels drop and only the height changes.
//!
//! # Where this differs from upstream
//!
//! - **What is read of the logic** is gathered first, read only with `&World` ([`physics_xform_facts`]): the
//!   locomotor's constants, the physics module's fields through `PhysicsBehavior::peek_*`, the terrain's height and
//!   normal (`World::peek_layer_height_and_normal`), the height above the terrain
//!   (`World::peek_is_significantly_above_terrain`), and for the treads the overlapped object and its body's crushed
//!   flags (`World::peek_crushed`). Nothing is written and no cache is filled.
//! - **Time frozen** (`isTimeFrozen`, the script engine's freezes): the client has no frozen time yet; not checked.
//! - `calcPhysicsXformMotorcycle` (`LOCO_MOTORCYCLE`) is not translated: no transform for it yet.
//! - The hover model's `atan2`, `sqrt`, `sin` and `cos` take float arguments, so they are the C++ `float` overloads
//!   of MSVC's `math.h` (`atan2f`, `sqrtf`, `sinf`, `cosf`): petir's `f32` functions here. (Not checked against a
//!   build of the shipped binary; the double functions would differ in the last bit at most.)

use crate::common::base_type::{sqr, Coord3D, PI};
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{KINDOF_INFANTRY, KINDOF_LOW_OVERLAPPABLE, KINDOF_SHRUBBERY};
use crate::common::random_value::RandomValues;
use crate::common::trig;
use crate::logic::locomotor::{Locomotor, LocomotorAppearance};
use crate::logic::world::game_logic::World;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::wwmath::fabs;

use super::drawable::Drawable;

/// `struct TWheelInfo`: the wheels' suspension and steering, for the truck draw modules.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TWheelInfo {
    /// `m_frontLeftHeightOffset` ... `m_rearRightHeightOffset`: height offsets for tires due to suspension sway.
    pub front_left_height_offset: f32,
    pub front_right_height_offset: f32,
    pub rear_left_height_offset: f32,
    pub rear_right_height_offset: f32,
    /// `m_wheelAngle`: 0 straight, > 0 left, < 0 right.
    pub wheel_angle: f32,
    /// `m_framesAirborneCounter`.
    pub frames_airborne_counter: i32,
    /// `m_framesAirborne`: how many frames it was in the air.
    pub frames_airborne: i32,
}

/// `class DrawableLocoInfo`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawableLocoInfo {
    pub pitch: f32,
    pub pitch_rate: f32,
    pub roll: f32,
    pub roll_rate: f32,
    pub yaw: f32,
    pub acceleration_pitch: f32,
    pub acceleration_pitch_rate: f32,
    pub acceleration_roll: f32,
    pub acceleration_roll_rate: f32,
    pub overlap_z_vel: f32,
    pub overlap_z: f32,
    pub wobble: f32,
    pub yaw_modulator: f32,
    pub pitch_modulator: f32,
    pub wheel_info: TWheelInfo,
}

impl Default for DrawableLocoInfo {
    /// `DrawableLocoInfo()`: all 0 but `m_wobble`, 1.
    fn default() -> Self {
        DrawableLocoInfo {
            pitch: 0.0,
            pitch_rate: 0.0,
            roll: 0.0,
            roll_rate: 0.0,
            yaw: 0.0,
            acceleration_pitch: 0.0,
            acceleration_pitch_rate: 0.0,
            acceleration_roll: 0.0,
            acceleration_roll_rate: 0.0,
            overlap_z_vel: 0.0,
            overlap_z: 0.0,
            wobble: 1.0,
            yaw_modulator: 0.0,
            pitch_modulator: 0.0,
            wheel_info: TWheelInfo::default(),
        }
    }
}

/// `Drawable::PhysicsXformInfo`: this draw's totals.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicsXformInfo {
    pub total_pitch: f32,
    pub total_roll: f32,
    pub total_yaw: f32,
    pub total_z: f32,
}

impl PhysicsXformInfo {
    /// The transform part of `applyPhysicsXform(mtx)`: `Translate(0, 0, totalZ)`, `Rotate_Y(totalPitch)`,
    /// `Rotate_X(-totalRoll)`, `Rotate_Z(totalYaw)`, each a post-multiplication (in the model's own frame).
    pub fn apply(&self, mtx: &mut Matrix3D) {
        use crate::logic::modules::plant;
        if !plant(9852) {
            mtx.translate(0.0, 0.0, self.total_z);
        }
        if plant(9851) {
            mtx.rotate_x(-self.total_roll);
            mtx.rotate_y(self.total_pitch);
        } else {
            mtx.rotate_y(self.total_pitch);
            mtx.rotate_x(if plant(9850) { self.total_roll } else { -self.total_roll });
        }
        mtx.rotate_z(self.total_yaw);
        if plant(9852) {
            mtx.translate(0.0, 0.0, self.total_z);
        }
    }

    /// [`PhysicsXformInfo::apply`] to the identity: the transform in the model's frame. ~~for a caller that has the
    /// object's own transform apart (the client's Bevy entity).~~ **Corrected 2026-11-13 (#179 item 3):** the client's
    /// renderer now takes [`crate::client::drawable::Drawable::draw_local_matrix`] (the instance matrix, then this);
    /// the draw modules take the whole root ([`crate::client::drawable::draw_transform_mtx`]).
    pub fn local_matrix(&self) -> Matrix3D {
        let mut m = Matrix3D::IDENTITY;
        self.apply(&mut m);
        m
    }

    /// `calcPhysicsXform`'s "HOTFIX": a total between -1e-20 and 1e-20 becomes 0.
    fn flush_denormals(&mut self) {
        for v in [&mut self.total_pitch, &mut self.total_roll, &mut self.total_yaw, &mut self.total_z] {
            if *v > -1e-20 && *v < 1e-20 {
                *v = 0.0;
            }
        }
    }
}

/// What `calcPhysicsXform*` read of the current locomotor (its template's constants and its moving-backwards flag).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocoFacts {
    pub appearance: LocomotorAppearance,
    pub accel_pitch_limit: f32,
    pub decel_pitch_limit: f32,
    pub bounce_kick: f32,
    pub pitch_stiffness: f32,
    pub roll_stiffness: f32,
    pub pitch_damping: f32,
    pub roll_damping: f32,
    pub pitch_by_z_vel_coef: f32,
    pub thrust_roll: f32,
    pub wobble_rate: f32,
    pub max_wobble: f32,
    pub min_wobble: f32,
    pub forward_vel_coef: f32,
    pub lateral_vel_coef: f32,
    pub forward_accel_coef: f32,
    pub lateral_accel_coef: f32,
    pub uniform_axial_damping: f32,
    pub has_suspension: bool,
    pub max_wheel_extension: f32,
    pub wheel_turn_angle: f32,
    pub rudder_correction_degree: f32,
    pub rudder_correction_rate: f32,
    pub elevator_correction_degree: f32,
    pub elevator_correction_rate: f32,
    /// `isMovingBackwards()` (for the truck draw modules).
    pub is_moving_backwards: bool,
}

impl LocoFacts {
    /// The locomotor's getters (all `&self` field reads).
    pub fn of(l: &Locomotor) -> LocoFacts {
        LocoFacts {
            appearance: l.get_appearance(),
            accel_pitch_limit: l.get_accel_pitch_limit(),
            decel_pitch_limit: l.get_decel_pitch_limit(),
            bounce_kick: l.get_bounce_kick(),
            pitch_stiffness: l.get_pitch_stiffness(),
            roll_stiffness: l.get_roll_stiffness(),
            pitch_damping: l.get_pitch_damping(),
            roll_damping: l.get_roll_damping(),
            pitch_by_z_vel_coef: l.get_pitch_by_z_vel_coef(),
            thrust_roll: l.get_thrust_roll(),
            wobble_rate: l.get_wobble_rate(),
            max_wobble: l.get_max_wobble(),
            min_wobble: l.get_min_wobble(),
            forward_vel_coef: l.get_forward_vel_coef(),
            lateral_vel_coef: l.get_lateral_vel_coef(),
            forward_accel_coef: l.get_forward_accel_coef(),
            lateral_accel_coef: l.get_lateral_accel_coef(),
            uniform_axial_damping: l.get_uniform_axial_damping(),
            has_suspension: l.has_suspension(),
            max_wheel_extension: l.get_max_wheel_extension(),
            wheel_turn_angle: l.get_wheel_turn_angle(),
            rudder_correction_degree: l.get_rudder_correction_degree(),
            rudder_correction_rate: l.get_rudder_correction_rate(),
            elevator_correction_degree: l.get_elevator_correction_degree(),
            elevator_correction_rate: l.get_elevator_correction_rate(),
            is_moving_backwards: l.is_moving_backwards(),
        }
    }
}

/// The object the treads model is overlapping (`physics->getCurrentOverlap()`, not shrubbery), as it reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlapFacts {
    /// `getPosition()`.
    pub position: Coord3D,
    /// `getGeometryInfo().getBoundingCircleRadius()`.
    pub bounding_circle_radius: f32,
    /// `getGeometryInfo().getMaxHeightAbovePosition()`.
    pub max_height_above_position: f32,
    /// `KINDOF_LOW_OVERLAPPABLE`, or `KINDOF_INFANTRY`, or its body front and back crushed.
    pub flat: bool,
}

/// What `calcPhysicsXform*` read of the object and its physics; see [`physics_xform_facts`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XformObjectFacts {
    /// `getPosition()`.
    pub position: Coord3D,
    /// `getUnitDirectionVector2D()`.
    pub dir: Coord3D,
    /// `physics->getAcceleration()`, `getVelocity()`, `getVelocityMagnitude()`, `isMotive()`, `getTurning()`,
    /// `getForwardSpeed2D()`.
    pub accel: Coord3D,
    pub vel: Coord3D,
    pub velocity_magnitude: f32,
    pub is_motive: bool,
    pub turning: i32,
    pub forward_speed_2d: f32,
    /// `TheTerrainLogic->getLayerHeight(pos->x, pos->y, obj->getLayer(), &normal)`: the height and the normal.
    pub layer_height: f32,
    pub normal: Coord3D,
    /// `obj->isSignificantlyAboveTerrain()`.
    pub airborne: bool,
    /// `obj->getGeometryInfo()`'s major and minor radius, and the drawable's bounding circle radius
    /// (`getDrawableGeometryInfo()`: the object's).
    pub major_radius: f32,
    pub minor_radius: f32,
    pub bounding_circle_radius: f32,
    /// `ai->getCurLocomotorSpeed()`.
    pub cur_locomotor_speed: f32,
    /// The overlapped object, `None` for none (or shrubbery); and whether `getPreviousOverlap()` is an object.
    pub overlap: Option<OverlapFacts>,
    pub had_previous_overlap: bool,
}

/// What `applyPhysicsXform` and `calcPhysicsXform` read of the logic for one drawable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsXformFacts {
    /// There is an object, it is not `DISABLED_HELD`, and `TheGlobalData->m_showClientPhysics`.
    pub apply: bool,
    /// `ai->getCurLocomotor()`'s; `None` without an AI or a current locomotor.
    pub locomotor: Option<LocoFacts>,
    /// `None` without physics (each model returns before it changes anything).
    pub object: Option<XformObjectFacts>,
}

/// What [`Drawable::apply_physics_xform`] reads of object `id`, all with `&World`: the object's fields and its
/// thing's position and direction (`Thing::peek_unit_direction_vector_2d`, the cache not filled), the AI's current
/// locomotor ([`World::ai_peek`]), the physics module's fields ([`World::physics_peek`] and `PhysicsBehavior::peek_*`),
/// the terrain ([`World::peek_layer_height_and_normal`]), the height above it
/// ([`World::peek_is_significantly_above_terrain`]), `getCurLocomotorSpeed` ([`World::peek_cur_locomotor_speed`]),
/// and the overlapped object with its body's crushed flags ([`World::peek_crushed`]). Nothing is written.
pub fn physics_xform_facts(w: &World, id: ObjectId) -> PhysicsXformFacts {
    let Some(o) = w.find_object(id) else {
        return PhysicsXformFacts { apply: false, locomotor: None, object: None };
    };
    let show = w.statics.global_data.the_global_data().is_none_or(|g| g.show_client_physics);
    let apply = show && !o.is_disabled_by_type(DISABLED_HELD);
    let locomotor = w.ai_peek(id).and_then(|ai| ai.get_cur_locomotor()).map(LocoFacts::of);
    let object = w.physics_peek(id).map(|p| {
        let pos = *o.thing.position();
        let dir = o.thing.peek_unit_direction_vector_2d();
        let (layer_height, normal) = w.peek_layer_height_and_normal(pos.x, pos.y, o.layer);
        let (current, previous) = p.peek_overlaps();
        let overlap = w.find_object(current).filter(|x| !x.is_kind_of(KINDOF_SHRUBBERY)).map(|x| {
            let (front, back) = w.peek_crushed(current);
            OverlapFacts {
                position: x.get_position(),
                bounding_circle_radius: x.geometry_info.get_bounding_circle_radius(),
                max_height_above_position: x.geometry_info.get_max_height_above_position(),
                flat: x.is_kind_of(KINDOF_LOW_OVERLAPPABLE) || x.is_kind_of(KINDOF_INFANTRY) || (front && back),
            }
        });
        XformObjectFacts {
            position: pos,
            dir,
            accel: p.peek_acceleration(),
            vel: p.peek_velocity(),
            velocity_magnitude: p.peek_velocity_magnitude(),
            is_motive: p.peek_is_motive(w.frame()),
            turning: p.peek_turning(),
            forward_speed_2d: p.peek_forward_speed_2d(&dir),
            layer_height,
            normal,
            airborne: w.peek_is_significantly_above_terrain(id),
            major_radius: o.geometry_info.get_major_radius(),
            minor_radius: o.geometry_info.get_minor_radius(),
            bounding_circle_radius: o.geometry_info.get_bounding_circle_radius(),
            cur_locomotor_speed: w.peek_cur_locomotor_speed(id).unwrap_or(0.0),
            overlap,
            had_previous_overlap: previous != INVALID_ID,
        }
    });
    PhysicsXformFacts { apply, locomotor, object }
}

/// `PhysicsTurningType::TURN_NEGATIVE`, `TURN_POSITIVE`.
const TURN_NEGATIVE: i32 = -1;
const TURN_POSITIVE: i32 = 1;

/// `WHEEL_SMOOTHNESS`: "higher numbers add smaller angles, make it more 'smooth'".
const WHEEL_SMOOTHNESS: f32 = 10.0;

impl DrawableLocoInfo {
    /// The spring and damper of the acceleration pitch and roll, damped back towards zero (every model's).
    fn damp_acceleration(&mut self, l: &LocoFacts) {
        self.acceleration_pitch_rate += (-l.pitch_stiffness * self.acceleration_pitch) + (-l.pitch_damping * self.acceleration_pitch_rate); // spring/damper
        self.acceleration_pitch += self.acceleration_pitch_rate;

        self.acceleration_roll_rate += (-l.roll_stiffness * self.acceleration_roll) + (-l.roll_damping * self.acceleration_roll_rate); // spring/damper
        self.acceleration_roll += self.acceleration_roll_rate;
    }

    /// The chassis pitching and rolling in reaction to acceleration (while motive).
    fn kick_acceleration(&mut self, l: &LocoFacts, dir: &Coord3D, accel: &Coord3D) {
        let forward_accel = dir.x * accel.x + dir.y * accel.y;
        self.acceleration_pitch_rate += -(l.forward_accel_coef * forward_accel);

        let lateral_accel = -dir.y * accel.x + dir.x * accel.y;
        self.acceleration_roll_rate += -(l.lateral_accel_coef * lateral_accel);
    }

    /// The acceleration pitch and roll limited (`DECEL_PITCH_LIMIT` above, `-ACCEL_PITCH_LIMIT` below; the roll by the
    /// pitch limits too).
    fn limit_acceleration(&mut self, l: &LocoFacts) {
        if self.acceleration_pitch > l.decel_pitch_limit {
            self.acceleration_pitch = l.decel_pitch_limit;
        } else if self.acceleration_pitch < -l.accel_pitch_limit {
            self.acceleration_pitch = -l.accel_pitch_limit;
        }

        if self.acceleration_roll > l.decel_pitch_limit {
            self.acceleration_roll = l.decel_pitch_limit;
        } else if self.acceleration_roll < -l.accel_pitch_limit {
            self.acceleration_roll = -l.accel_pitch_limit;
        }
    }

    /// The ground's pitch and roll under the chassis: the normal's dot products with the direction and with its
    /// perpendicular `(-dir.y, dir.x)`, times π/2.
    fn ground_pitch_and_roll(o: &XformObjectFacts, normal: &Coord3D) -> (f32, f32) {
        let dir = &o.dir;
        let perp = Coord3D { x: -dir.y, y: dir.x, z: 0.0 };
        let dot = normal.x * dir.x + normal.y * dir.y;
        let ground_pitch = dot * (PI / 2.0);
        let dot = normal.x * perp.x + normal.y * perp.y;
        let ground_roll = dot * (PI / 2.0);
        (ground_pitch, ground_roll)
    }

    /// `calcPhysicsXformThrust(locomotor, info)`: the wobble ("a kind of quick thrust implementation cause we need
    /// scud missiles to wobble *now*") and the thrust roll. Reads no physics.
    pub fn calc_thrust(&mut self, l: &LocoFacts, info: &mut PhysicsXformInfo) {
        let (thrust_roll, wobble_rate, max_wobble, min_wobble) = (l.thrust_roll, l.wobble_rate, l.max_wobble, l.min_wobble);
        if wobble_rate != 0.0 {
            if self.wobble >= 1.0 {
                if self.pitch < max_wobble - wobble_rate * 2.0 {
                    self.pitch += wobble_rate;
                    self.yaw += wobble_rate;
                } else {
                    self.pitch += wobble_rate / 2.0;
                    self.yaw += wobble_rate / 2.0;
                }
                if self.pitch >= max_wobble {
                    self.wobble = -1.0;
                }
            } else {
                if self.pitch >= min_wobble + wobble_rate * 2.0 {
                    self.pitch -= wobble_rate;
                    self.yaw -= wobble_rate;
                } else {
                    self.pitch -= wobble_rate / 2.0;
                    self.yaw -= wobble_rate / 2.0;
                }
                if self.pitch <= min_wobble {
                    self.wobble = 1.0;
                }
            }
            info.total_pitch = self.pitch;
            info.total_yaw = self.yaw;
        }
        if thrust_roll != 0.0 {
            self.roll += thrust_roll;
            info.total_roll = self.roll;
        }
    }

    /// `calcPhysicsXformHoverOrWings(locomotor, info)`: pitch and roll springs towards level, kicked by the speed (and
    /// the climb, with `PitchByZVelCoef`) and the acceleration while motive; the rudder and elevator's swimmy
    /// correction (`sin`/`cos` of their modulators, advanced each draw). `None` physics: returns, totals 0.
    pub fn calc_hover_or_wings(&mut self, l: &LocoFacts, o: Option<&XformObjectFacts>, info: &mut PhysicsXformInfo) {
        let Some(o) = o else { return };
        let (dir, accel, vel) = (&o.dir, &o.accel, &o.vel);

        self.pitch_rate += (-l.pitch_stiffness * self.pitch) + (-l.pitch_damping * self.pitch_rate); // spring/damper
        self.roll_rate += (-l.roll_stiffness * self.roll) + (-l.roll_damping * self.roll_rate); // spring/damper

        self.pitch += self.pitch_rate * l.uniform_axial_damping;
        self.roll += self.roll_rate * l.uniform_axial_damping;

        // process chassis acceleration dynamics - damp back towards zero
        self.damp_acceleration(l);

        // compute total pitch and roll of tank
        info.total_pitch = self.pitch + self.acceleration_pitch;
        info.total_roll = self.roll + self.acceleration_roll;

        if o.is_motive {
            if l.pitch_by_z_vel_coef != 0.0 {
                const TINY_DZ: f32 = 0.001;
                if fabs(vel.z) > TINY_DZ {
                    // `atan2(vel->z, sqrt(sqr(vel->x)+sqr(vel->y)))`, the float overloads
                    let horizontal: f32 = petir::Real::sqrt(sqr(vel.x) + sqr(vel.y));
                    let pitch: f32 = petir::Real::atan2(vel.z, horizontal);
                    self.pitch -= l.pitch_by_z_vel_coef * pitch;
                }
            }

            // cause the chassis to pitch & roll in reaction to current speed
            let forward_vel = dir.x * vel.x + dir.y * vel.y;
            self.pitch += -(l.forward_vel_coef * forward_vel);

            let lateral_vel = -dir.y * vel.x + dir.x * vel.y;
            self.roll += -(l.lateral_vel_coef * lateral_vel);

            // cause the chassis to pitch & roll in reaction to acceleration/deceleration
            self.kick_acceleration(l, dir, accel);
        }

        // limit acceleration pitch and roll
        self.limit_acceleration(l);

        self.yaw_modulator += l.rudder_correction_rate;
        info.total_yaw = l.rudder_correction_degree * petir::Real::sin(self.yaw_modulator);
        self.pitch_modulator += l.elevator_correction_rate;
        info.total_pitch += l.elevator_correction_degree * petir::Real::cos(self.pitch_modulator);

        info.total_z = 0.0;
    }

    /// `calcPhysicsXformTreads(locomotor, info)`: pitch and roll springs towards the ground's (or, overlapping an
    /// object, towards a surface faked over it, roughened by the client's random in proportion to the speed), a pitch
    /// kick on leaving an overlap in the air, the acceleration kicks while motive, and the fake Z of riding over the
    /// overlapped object. `None` physics: returns, totals 0.
    #[allow(clippy::manual_clamp)] // upstream's if/else, which keeps a NaN where `clamp` would too, but written as upstream wrote it
    pub fn calc_treads(&mut self, l: &LocoFacts, o: Option<&XformObjectFacts>, random: &mut RandomValues, info: &mut PhysicsXformInfo) {
        const OVERLAP_SHRINK_FACTOR: f32 = 0.8;
        const FLATTENED_OBJECT_HEIGHT: f32 = 0.5;
        const LEAVE_OVERLAP_PITCH_KICK: f32 = PI / 128.0;
        const OVERLAP_ROUGH_VIBRATION_FACTOR: f32 = 5.0;
        const MAX_ROUGH_VIBRATION: f32 = 0.5;
        let Some(o) = o else { return };
        let (pos, dir, accel, vel) = (&o.position, &o.dir, &o.accel, &o.vel);

        // find pitch and roll of terrain under chassis
        let mut normal = o.normal;

        // override surface normal if we are overlapping another object - crushing it
        let mut overlap_z = 0.0f32;

        if let Some(over) = &o.overlap {
            let over_pos = &over.position;
            let dx = over_pos.x - pos.x;
            let dy = over_pos.y - pos.y;
            let center_dist_sqr = sqr(dx) + sqr(dy);

            // compute maximum distance between objects, if their edges just touched
            let our_size = o.bounding_circle_radius;
            let other_size = over.bounding_circle_radius;
            let mut max_center_dist = other_size + our_size;

            // shrink the overlap distance a bit to avoid floating
            max_center_dist *= OVERLAP_SHRINK_FACTOR;
            if center_dist_sqr < sqr(max_center_dist) {
                let center_dist: f32 = petir::Real::sqrt(center_dist_sqr);
                let mut amount = 1.0 - center_dist / max_center_dist;
                if amount < 0.0 {
                    amount = 0.0;
                } else if amount > 1.0 {
                    amount = 1.0;
                }

                // rough vibrations proportional to speed when we drive over something
                let mut rough = (vel.x * vel.x + vel.y * vel.y) * OVERLAP_ROUGH_VIBRATION_FACTOR;
                if rough > MAX_ROUGH_VIBRATION {
                    rough = MAX_ROUGH_VIBRATION;
                }

                let mut height = over.max_height_above_position;

                // do not "go up" flattened crushed things
                let flat = over.flat;
                if flat {
                    height = FLATTENED_OBJECT_HEIGHT;
                }

                if amount < FLATTENED_OBJECT_HEIGHT && !flat {
                    overlap_z = height * 2.0 * amount;

                    // compute vector along "surface"
                    let v = Coord3D { x: dx / center_dist, y: dy / center_dist, z: 0.2 };

                    let mut up = Coord3D { x: random.game_client_random_value_real(-rough, rough), y: 0.0, z: 1.0 };
                    up.y = random.game_client_random_value_real(-rough, rough);
                    up.normalize();

                    let prp = Coord3D::cross_product(&v, &up);
                    normal = Coord3D::cross_product(&prp, &v);

                    // compute unit normal
                    normal.normalize();
                } else {
                    // sitting on top of object
                    overlap_z = height;

                    normal.x = random.game_client_random_value_real(-rough, rough);
                    normal.y = random.game_client_random_value_real(-rough, rough);
                    normal.z = 1.0;
                    normal.normalize();
                }
            }
        } else {
            // no overlap this frame: if we had an overlap last frame, and we're now in the air, give a kick to the
            // pitch for effect
            if o.had_previous_overlap && self.overlap_z > 0.0 {
                self.pitch_rate += LEAVE_OVERLAP_PITCH_KICK;
            }
        }

        let (ground_pitch, ground_roll) = Self::ground_pitch_and_roll(o, &normal);

        // process chassis suspension dynamics - damp back towards groundPitch

        // the ground can only push back if we're touching it
        if o.overlap.is_some() || self.overlap_z <= 0.0 {
            self.pitch_rate += (-l.pitch_stiffness * (self.pitch - ground_pitch)) + (-l.pitch_damping * self.pitch_rate); // spring/damper
            if self.pitch_rate > 0.0 {
                self.pitch_rate *= 0.5;
            }

            self.roll_rate += (-l.roll_stiffness * (self.roll - ground_roll)) + (-l.roll_damping * self.roll_rate); // spring/damper
        }

        self.pitch += self.pitch_rate * l.uniform_axial_damping;
        self.roll += self.roll_rate * l.uniform_axial_damping;

        // process chassis recoil dynamics - damp back towards zero
        self.damp_acceleration(l);

        // compute total pitch and roll of tank
        info.total_pitch = self.pitch + self.acceleration_pitch;
        info.total_roll = self.roll + self.acceleration_roll;

        if o.is_motive {
            // cause the chassis to pitch & roll in reaction to acceleration/deceleration
            self.kick_acceleration(l, dir, accel);
        }

        // limit recoil pitch and roll
        self.limit_acceleration(l);

        // adjust z
        if overlap_z > self.overlap_z {
            self.overlap_z = overlap_z;
            self.overlap_z_vel = 0.0;
        }

        let ztmp = self.overlap_z / 2.0;

        // do fake Z physics
        if self.overlap_z > 0.0 {
            self.overlap_z_vel -= 0.2;
            self.overlap_z += self.overlap_z_vel;
        }

        if self.overlap_z <= 0.0 {
            self.overlap_z = 0.0;
            self.overlap_z_vel = 0.0;
        }
        info.total_z = ztmp;
    }

    /// `calcPhysicsXformWheels(locomotor, info)` (see the module docs). `None` physics: returns, totals 0.
    pub fn calc_wheels(&mut self, l: &LocoFacts, o: Option<&XformObjectFacts>, random: &mut RandomValues, info: &mut PhysicsXformInfo) {
        let bounce_angle_kick = l.bounce_kick;
        let max_suspension_extension = l.max_wheel_extension; //-2.3f;
        let wheel_angle = l.wheel_turn_angle; //PI/8;
        let do_wheels = l.has_suspension;

        let Some(o) = o else { return };
        let (pos, dir, accel) = (&o.position, &o.dir, &o.accel);

        // find pitch and roll of terrain under chassis
        let hheight = o.layer_height;
        let (ground_pitch, ground_roll) = Self::ground_pitch_and_roll(o, &o.normal);

        let airborne = o.airborne;

        if airborne {
            if do_wheels {
                // Wheels extend when airborne.
                let w = &mut self.wheel_info;
                w.frames_airborne = 0;
                w.frames_airborne_counter += 1;
                if pos.z - hheight > -max_suspension_extension {
                    w.rear_left_height_offset += (max_suspension_extension - w.rear_left_height_offset) / 2.0;
                    w.rear_right_height_offset += (max_suspension_extension - w.rear_right_height_offset) / 2.0;
                } else {
                    w.rear_left_height_offset += (0.0 - w.rear_left_height_offset) / 2.0;
                    w.rear_right_height_offset += (0.0 - w.rear_right_height_offset) / 2.0;
                }
            }
            // Calculate suspension info.
            let length = o.major_radius;
            let width = o.minor_radius;
            let pitch_height = length * trig::sin(self.pitch + self.acceleration_pitch - ground_pitch);
            let roll_height = width * trig::sin(self.roll + self.acceleration_roll - ground_roll);
            info.total_z = fabs(pitch_height) / 4.0 + fabs(roll_height) / 4.0;
            return; // maintain the same orientation while we fly through the air.
        }

        // Bouncy.
        let cur_speed = o.velocity_magnitude;
        let max_speed = o.cur_locomotor_speed;
        if !airborne && cur_speed > max_speed / 10.0 {
            let factor = cur_speed / max_speed;
            if fabs(self.pitch_rate) < factor * bounce_angle_kick / 4.0 && fabs(self.roll_rate) < factor * bounce_angle_kick / 8.0 {
                // do the bouncy.
                match random.game_client_random_value(0, 3) {
                    0 => {
                        self.pitch_rate -= bounce_angle_kick * factor;
                        self.roll_rate -= bounce_angle_kick * factor / 2.0;
                    }
                    1 => {
                        self.pitch_rate += bounce_angle_kick * factor;
                        self.roll_rate -= bounce_angle_kick * factor / 2.0;
                    }
                    2 => {
                        self.pitch_rate -= bounce_angle_kick * factor;
                        self.roll_rate += bounce_angle_kick * factor / 2.0;
                    }
                    3 => {
                        self.pitch_rate += bounce_angle_kick * factor;
                        self.roll_rate += bounce_angle_kick * factor / 2.0;
                    }
                    _ => {}
                }
            }
        }

        // process chassis suspension dynamics - damp back towards groundPitch

        // the ground can only push back if we're touching it
        if !airborne {
            self.pitch_rate += (-l.pitch_stiffness * (self.pitch - ground_pitch)) + (-l.pitch_damping * self.pitch_rate); // spring/damper
            if self.pitch_rate > 0.0 {
                self.pitch_rate *= 0.5;
            }

            self.roll_rate += (-l.roll_stiffness * (self.roll - ground_roll)) + (-l.roll_damping * self.roll_rate); // spring/damper
        }

        self.pitch += self.pitch_rate * l.uniform_axial_damping;
        self.roll += self.roll_rate * l.uniform_axial_damping;

        // process chassis acceleration dynamics - damp back towards zero
        self.damp_acceleration(l);

        // compute total pitch and roll of tank
        info.total_pitch = self.pitch + self.acceleration_pitch;
        info.total_roll = self.roll + self.acceleration_roll;

        if o.is_motive {
            // cause the chassis to pitch & roll in reaction to acceleration/deceleration
            self.kick_acceleration(l, dir, accel);
        }

        // limit acceleration pitch and roll
        self.limit_acceleration(l);

        info.total_z = 0.0;

        // Calculate suspension info.
        let length = o.major_radius;
        let width = o.minor_radius;
        let pitch_height = length * trig::sin(info.total_pitch - ground_pitch);
        let roll_height = width * trig::sin(info.total_roll - ground_roll);
        if do_wheels {
            // calculate each wheel position
            self.wheel_info.frames_airborne = self.wheel_info.frames_airborne_counter;
            self.wheel_info.frames_airborne_counter = 0;
            let mut new_info = self.wheel_info;
            let rotation = o.turning;
            if rotation == TURN_NEGATIVE {
                new_info.wheel_angle = -wheel_angle;
            } else if rotation == TURN_POSITIVE {
                new_info.wheel_angle = wheel_angle;
            } else {
                new_info.wheel_angle = 0.0;
            }
            if o.forward_speed_2d < 0.0 {
                // if we're moving backwards, the wheels rotate in the opposite direction.
                new_info.wheel_angle = -new_info.wheel_angle;
            }

            // smaller angles each draw, for "smoother" wheel turning transitions
            let w = &mut self.wheel_info;
            w.wheel_angle += (new_info.wheel_angle - w.wheel_angle) / WHEEL_SMOOTHNESS;

            const SPRING_FACTOR: f32 = 0.9;
            if pitch_height < 0.0 {
                // Front raising up
                new_info.front_left_height_offset = SPRING_FACTOR * (pitch_height / 3.0 + pitch_height / 2.0);
                new_info.front_right_height_offset = SPRING_FACTOR * (pitch_height / 3.0 + pitch_height / 2.0);
                new_info.rear_left_height_offset = -pitch_height / 2.0 + pitch_height / 4.0;
                new_info.rear_right_height_offset = -pitch_height / 2.0 + pitch_height / 4.0;
            } else {
                // Back rasing up.
                new_info.front_left_height_offset = -pitch_height / 4.0 + pitch_height / 2.0;
                new_info.front_right_height_offset = -pitch_height / 4.0 + pitch_height / 2.0;
                new_info.rear_left_height_offset = SPRING_FACTOR * (-pitch_height / 2.0 + -pitch_height / 3.0);
                new_info.rear_right_height_offset = SPRING_FACTOR * (-pitch_height / 2.0 + -pitch_height / 3.0);
            }
            if roll_height > 0.0 {
                // Right raising up
                new_info.front_right_height_offset += -SPRING_FACTOR * (roll_height / 3.0 + roll_height / 2.0);
                new_info.rear_right_height_offset += -SPRING_FACTOR * (roll_height / 3.0 + roll_height / 2.0);
                new_info.rear_left_height_offset += roll_height / 2.0 - roll_height / 4.0;
                new_info.front_left_height_offset += roll_height / 2.0 - roll_height / 4.0;
            } else {
                // Left rasing up.
                new_info.front_right_height_offset += -roll_height / 2.0 + roll_height / 4.0;
                new_info.rear_right_height_offset += -roll_height / 2.0 + roll_height / 4.0;
                new_info.rear_left_height_offset += SPRING_FACTOR * (roll_height / 3.0 + roll_height / 2.0);
                new_info.front_left_height_offset += SPRING_FACTOR * (roll_height / 3.0 + roll_height / 2.0);
            }
            // If it's going down, dampen the movement a bit
            let follow = |cur: &mut f32, new: f32| {
                if new < *cur {
                    *cur += (new - *cur) / 2.0;
                } else {
                    *cur = new;
                }
            };
            follow(&mut w.front_left_height_offset, new_info.front_left_height_offset);
            follow(&mut w.front_right_height_offset, new_info.front_right_height_offset);
            follow(&mut w.rear_left_height_offset, new_info.rear_left_height_offset);
            follow(&mut w.rear_right_height_offset, new_info.rear_right_height_offset);
            for h in [&mut w.front_left_height_offset, &mut w.front_right_height_offset, &mut w.rear_left_height_offset, &mut w.rear_right_height_offset] {
                if *h < max_suspension_extension {
                    *h = max_suspension_extension;
                }
            }
        }
        // If we are > 22 degrees, need to raise height;
        let mut divisor = 4.0f32;
        let pitch = fabs(info.total_pitch - ground_pitch);

        if pitch > PI / 8.0 {
            divisor = ((4.0 * PI / 8.0) + (1.0 * (pitch - PI / 8.0))) / pitch;
        }
        info.total_z += fabs(pitch_height) / divisor;
        info.total_z += fabs(roll_height) / divisor;
    }
}

impl Drawable {
    /// `calcPhysicsXform(info)`: the current locomotor's model by its appearance (making `m_locoInfo` on the first),
    /// the totals' denormals flushed; `None` when no model ran (no locomotor, or an appearance without one:
    /// `LOCO_LEGS_TWO`, `LOCO_CLIMBER`, `LOCO_OTHER`, and `LOCO_MOTORCYCLE`, not translated yet).
    pub fn calc_physics_xform(&mut self, facts: &PhysicsXformFacts, random: &mut RandomValues) -> Option<PhysicsXformInfo> {
        let l = facts.locomotor.as_ref()?;
        let mut info = PhysicsXformInfo::default();
        let o = facts.object.as_ref();
        match l.appearance {
            LocomotorAppearance::LocoWheelsFour => self.loco_info.get_or_insert_with(Default::default).calc_wheels(l, o, random, &mut info),
            LocomotorAppearance::LocoTreads => self.loco_info.get_or_insert_with(Default::default).calc_treads(l, o, random, &mut info),
            LocomotorAppearance::LocoHover | LocomotorAppearance::LocoWings => {
                self.loco_info.get_or_insert_with(Default::default).calc_hover_or_wings(l, o, &mut info)
            }
            LocomotorAppearance::LocoThrust => self.loco_info.get_or_insert_with(Default::default).calc_thrust(l, &mut info),
            _ => return None,
        }
        info.flush_denormals();
        Some(info)
    }

    /// `applyPhysicsXform(&transformMtx)` as `Drawable::draw` calls it (see the module docs): this draw's totals kept in
    /// [`Drawable::physics_xform`] (for the client to put on the drawn model, [`PhysicsXformInfo::apply`]; **#179 item
    /// 3:** and put on the draw's root by [`Drawable::draw_transform_mtx`], as upstream puts them on `transformMtx`),
    /// `None` when none applies (no object, held, `ShowClientPhysics` off, the drawable hidden, or no model ran).
    pub fn apply_physics_xform(&mut self, facts: &PhysicsXformFacts, random: &mut RandomValues) {
        self.physics_xform = if !facts.apply || self.hidden { None } else { self.calc_physics_xform(facts, random) };
    }

    /// `getWheelInfo()`: `m_locoInfo`'s wheel info, `None` until a physics model made it.
    pub fn get_wheel_info(&self) -> Option<TWheelInfo> {
        self.loco_info.as_ref().map(|l| l.wheel_info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loco(appearance: LocomotorAppearance) -> LocoFacts {
        LocoFacts {
            appearance,
            accel_pitch_limit: 0.5,
            decel_pitch_limit: 0.5,
            bounce_kick: 0.0,
            pitch_stiffness: 0.0,
            roll_stiffness: 0.0,
            pitch_damping: 0.0,
            roll_damping: 0.0,
            pitch_by_z_vel_coef: 0.0,
            thrust_roll: 0.0,
            wobble_rate: 0.0,
            max_wobble: 0.0,
            min_wobble: 0.0,
            forward_vel_coef: 0.0,
            lateral_vel_coef: 0.0,
            forward_accel_coef: 0.0,
            lateral_accel_coef: 0.0,
            uniform_axial_damping: 1.0,
            has_suspension: false,
            max_wheel_extension: 0.0,
            wheel_turn_angle: 0.0,
            rudder_correction_degree: 0.0,
            rudder_correction_rate: 0.0,
            elevator_correction_degree: 0.0,
            elevator_correction_rate: 0.0,
            is_moving_backwards: false,
        }
    }

    fn object(vx: f32, motive: bool) -> XformObjectFacts {
        XformObjectFacts {
            position: Coord3D::default(),
            dir: Coord3D { x: 1.0, y: 0.0, z: 0.0 },
            accel: Coord3D::default(),
            vel: Coord3D { x: vx, y: 0.0, z: 0.0 },
            velocity_magnitude: fabs(vx),
            is_motive: motive,
            turning: 0,
            forward_speed_2d: vx,
            layer_height: 0.0,
            normal: Coord3D { x: 0.0, y: 0.0, z: 1.0 },
            airborne: false,
            major_radius: 5.0,
            minor_radius: 5.0,
            bounding_circle_radius: 5.0,
            cur_locomotor_speed: 4.0,
            overlap: None,
            had_previous_overlap: false,
        }
    }

    fn drawable() -> Drawable {
        Drawable::for_tests()
    }

    /// *Predictions, written before the first run* (`WobbleRate` 0.1, `MaxWobble` 0.25, `MinWobble` -0.25, `ThrustRoll`
    /// 0.05): the pitch and yaw climb 0.1, then 0.05 a draw once within two rates of the maximum (0.1, 0.15, 0.2, 0.25),
    /// the wobble turning to -1 on the fourth draw; the fifth goes down by 0.1; the roll grows by 0.05 a draw; the
    /// drawable's `m_locoInfo` is made on the first call; no physics is needed.
    ///
    /// *Results (2026-11-06), first run:* all held (to within 1e-6 for the sums).
    #[test]
    fn thrust_wobbles_and_rolls() {
        let l = LocoFacts { wobble_rate: 0.1, max_wobble: 0.25, min_wobble: -0.25, thrust_roll: 0.05, ..loco(LocomotorAppearance::LocoThrust) };
        let facts = PhysicsXformFacts { apply: true, locomotor: Some(l), object: None };
        let mut d = drawable();
        let mut random = RandomValues::new(0);
        assert!(d.loco_info.is_none());
        let mut pitches = Vec::new();
        for _ in 0..5 {
            let info = d.calc_physics_xform(&facts, &mut random).expect("a thrust model");
            pitches.push(info.total_pitch);
            assert_eq!(info.total_pitch, info.total_yaw);
        }
        let want = [0.1, 0.15, 0.2, 0.25, 0.15];
        assert!(pitches.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-6), "{pitches:?}");
        let li = d.loco_info.unwrap();
        assert_eq!(li.wobble, -1.0);
        assert!((li.roll - 0.25).abs() < 1e-6);
    }

    /// *Predictions, written before the first run* (hover: no springs, axial damping 1, `ForwardVelocityPitchFactor`
    /// 0.1, rudder correction 0.1 at 0.5 a draw): moving at 2 forward under a motive force, the pitch goes down by 0.2 a
    /// draw after the totals are taken (totals 0 then -0.2); the yaw is 0.1 sin 0.5, then 0.1 sin 1; not motive, the
    /// pitch stays; without physics the model returns, totals 0, but the transform still applies (`Some`).
    ///
    /// *Results (2026-11-06), first run:* all held.
    #[test]
    fn hover_pitches_with_its_speed() {
        let l = LocoFacts { forward_vel_coef: 0.1, rudder_correction_degree: 0.1, rudder_correction_rate: 0.5, ..loco(LocomotorAppearance::LocoHover) };
        let mut d = drawable();
        let mut random = RandomValues::new(0);
        let f = PhysicsXformFacts { apply: true, locomotor: Some(l), object: Some(object(2.0, true)) };
        let a = d.calc_physics_xform(&f, &mut random).unwrap();
        let b = d.calc_physics_xform(&f, &mut random).unwrap();
        assert_eq!(a.total_pitch, 0.0);
        assert!((b.total_pitch + 0.2).abs() < 1e-7);
        assert_eq!(a.total_yaw, 0.1 * trig::sin(0.5));
        assert_eq!(b.total_yaw, 0.1 * trig::sin(1.0));
        let still = PhysicsXformFacts { object: Some(object(2.0, false)), ..f };
        let c = d.calc_physics_xform(&still, &mut random).unwrap();
        assert!((c.total_pitch + 0.4).abs() < 1e-6, "{}", c.total_pitch);
        let none = PhysicsXformFacts { object: None, ..f };
        let mut e = drawable();
        assert_eq!(e.calc_physics_xform(&none, &mut random), Some(PhysicsXformInfo::default()));
        assert_eq!(e.calc_physics_xform(&PhysicsXformFacts { locomotor: None, ..f }, &mut random), None);
        e.apply_physics_xform(&PhysicsXformFacts { apply: false, ..f }, &mut random);
        assert_eq!(e.physics_xform, None);
        e.hidden = true;
        e.apply_physics_xform(&f, &mut random);
        assert_eq!(e.physics_xform, None);
    }

    /// *Predictions, written before the first run* (treads, no springs, axial damping 1): sitting on an object 1 away
    /// (both bounding radii 5: 0.8 × 10 = 8 apart at most, amount 0.875, so "sitting on top"), 3 high, at rest: the
    /// normal is drawn from the client's generator (two reals in [-0, 0], so (0, 0, 1)), the overlap Z becomes 3, the
    /// total Z half of it, 1.5, then the fake Z falls to 2.8; the next draw without an overlap (one the frame before)
    /// kicks the pitch rate by π/128 (in the air, so no spring) and the total Z is 1.4.
    ///
    /// *Results (2026-11-06), first run:* all held.
    #[test]
    fn treads_ride_over_what_they_overlap() {
        let l = loco(LocomotorAppearance::LocoTreads);
        let mut o = object(0.0, false);
        o.overlap = Some(OverlapFacts { position: Coord3D { x: 1.0, y: 0.0, z: 0.0 }, bounding_circle_radius: 5.0, max_height_above_position: 3.0, flat: false });
        let mut d = drawable();
        let mut random = RandomValues::new(5);
        let mut expect = random.clone();
        expect.game_client_random_value_real(0.0, 0.0);
        expect.game_client_random_value_real(0.0, 0.0);
        let a = d.calc_physics_xform(&PhysicsXformFacts { apply: true, locomotor: Some(l), object: Some(o) }, &mut random).unwrap();
        assert_eq!(format!("{random:?}"), format!("{expect:?}"));
        assert_eq!(a.total_z, 1.5);
        assert!((d.loco_info.unwrap().overlap_z - 2.8).abs() < 1e-6);
        let mut o2 = object(0.0, false);
        o2.had_previous_overlap = true;
        let b = d.calc_physics_xform(&PhysicsXformFacts { apply: true, locomotor: Some(l), object: Some(o2) }, &mut random).unwrap();
        assert!((b.total_z - 1.4).abs() < 1e-6);
        assert_eq!(d.loco_info.unwrap().pitch_rate, PI / 128.0);
        assert_eq!(b.total_pitch, PI / 128.0);
    }
}
