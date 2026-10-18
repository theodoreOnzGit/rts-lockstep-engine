// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Thing.h
//   GeneralsMD/Code/GameEngine/Source/Common/Thing/Thing.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Thing.h / Thing.cpp: Colin Day, May 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `class Thing`: the position, orientation and world transform that objects
//! and drawables share, with the cached values upstream derives from the
//! transform (the 2D direction vector and the heights above terrain and
//! above water).
//!
//! # Cut points
//!
//! Each of these is something upstream reaches through a global or a
//! virtual method that belongs to a later layer. Here it is an argument:
//!
//! 1. **The ThingTemplate pointer** (`m_template`, `getTemplate()`). The
//!    template is shared and never changes once loaded, so [`Thing`] holds
//!    an `Arc<T>` where `T: `[`ThingTemplateView`], the three `isKindOf`
//!    queries the thing forwards. `ThingTemplate` itself is layer 5 work
//!    not yet translated. Its debug-only name (`m_templateName`, `_DEBUG` and
//!    `_INTERNAL` builds) is not translated.
//! 2. **TheTerrainLogic and TheGlobalData** (`getGroundHeight`,
//!    `isUnderwater`, `alignOnTerrain`, `m_gravity`). The methods that touch
//!    them take a [`ThingContext`]. The terrain (layer 6) implements it.
//! 3. **reactToTransformChange** (pure virtual, implemented by `Object` and
//!    `Drawable`). Every setter returns a [`TransformChange`] holding the old
//!    transform, position and angle, which is exactly what upstream passes to
//!    the reaction. The owner must run its reaction on the returned value
//!    (`#[must_use]`).
//! 4. **calculateHeightAboveTerrain** (virtual, overridden by `Object` on
//!    bridges). The cached altitude getters use the base calculation, and
//!    the `_by` variants take the owner's calculation as a closure, so the
//!    bridge override is supplied by the caller (layer 8).
//! 5. **Object handles** and the `asObject`/`asDrawable` downcasts. Those
//!    are the owner's concern. `Thing` is never used without its owner.
//!
//! # Translation choices
//!
//! - **Caches and `&mut self`.** Upstream's getters are `const` but fill
//!   `mutable` caches. Here the getters that fill a cache take `&mut self`,
//!   so `Thing` stays `Sync` for the `Arc<RwLock<_>>` that holds simulation
//!   state (a `Cell` would not be). Getters that read no cache keep `&self`.
//! - **Several upstream overloads become one method.** `getUnitDirectionVector2D`
//!   returns a pointer or fills a reference; here it returns the value.
//!   `convertBonePosToWorldPos` has two optional outputs; here the two
//!   computations are separate methods and the combined one returns both.
//! - **NULL pointers.** A `NULL` template, position or bone position is
//!   undefined upstream (it is dereferenced). Here it is a
//!   [`ThingError`], never a crash. [`Thing::transform_point`] takes
//!   `Option`s, because upstream defines a `NULL` argument as a no-op.
//! - **Hanging angles.** `setOrientation` calls `normalizeAngle`, which never
//!   returns for an angle whose subtraction of `2*PI` changes nothing
//!   (`try_normalize_angle`). Upstream has already written the transform by
//!   then. Here the angle is checked first, and the error leaves the thing
//!   untouched.
//! - **Upstream quirk kept.** `setOrientation` on a
//!   `KINDOF_STICK_TO_TERRAIN_SLOPE` thing aligns the transform on the
//!   terrain but caches the position from *before* the alignment, so
//!   [`Thing::position`] can disagree with the transform's translation. The
//!   translation reproduces that (see the differential test), rather than
//!   silently "fixing" the cache.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_common::try_normalize_angle;
use crate::common::kind_of::{KindOfMaskType, KindOfType, KINDOF_STICK_TO_TERRAIN_SLOPE};
use crate::common::trig;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// Cut point 1: the queries `Thing` forwards to its `ThingTemplate`
/// (`isKindOf`, `isKindOfMulti`, `isAnyKindOf`).
pub trait ThingTemplateView {
    /// `ThingTemplate::isKindOf(t)`.
    fn is_kind_of(&self, t: KindOfType) -> bool;
    /// `ThingTemplate::isKindOfMulti(mustBeSet, mustBeClear)`.
    fn is_kind_of_multi(&self, must_be_set: &KindOfMaskType, must_be_clear: &KindOfMaskType) -> bool;
    /// `ThingTemplate::isAnyKindOf(anyKindOf)`.
    fn is_any_kind_of(&self, any_kind_of: &KindOfMaskType) -> bool;
}

/// Cut point 2: the terrain and game-data values `Thing` reads
/// (`TheTerrainLogic` and `TheGlobalData->m_gravity`).
pub trait ThingContext {
    /// `TheTerrainLogic->getGroundHeight(x, y)` (the normal is not asked for).
    fn ground_height(&self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->isUnderwater(x, y, &waterZ)`: the water level when
    /// the point is under water, `None` otherwise.
    fn underwater_level(&self, x: f32, y: f32) -> Option<f32>;
    /// `TheTerrainLogic->alignOnTerrain(orientation, pos, stickToGround, mtx)`.
    /// The result is the matrix upstream writes into `mtx`.
    fn align_on_terrain(&self, orientation: f32, pos: &Coord3D, stick_to_ground: bool) -> Matrix3D;
    /// `TheGlobalData->m_gravity`: acceleration per frame squared.
    fn gravity(&self) -> f32;
}

/// What the setters pass to upstream's `reactToTransformChange`: the
/// transform, position and angle as they were before the change.
#[must_use = "the owner must run its reaction to the transform change"]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformChange {
    pub old_transform: Matrix3D,
    pub old_position: Coord3D,
    pub old_angle: f32,
}

/// The errors of [`Thing`]: each is an input that is undefined upstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThingError {
    /// `Thing(NULL)`: upstream asserts and leaves `m_template` unset.
    NoTemplate,
    /// `setPosition(NULL)` dereferences its argument.
    NullPosition,
    /// `setOrientation(angle)` with an angle `normalizeAngle` never reduces
    /// (an infinity, or a magnitude at which `2*PI` is lost in `float`).
    AngleNeverNormalizes,
}

// m_cacheFlags bits (upstream's private enum).
const VALID_DIRVECTOR: u32 = 0x01;
const VALID_ALTITUDE_TERRAIN: u32 = 0x02;
const VALID_ALTITUDE_SEALEVEL: u32 = 0x04;

/// `class Thing`. Upstream's `m_transform` is the true description of where
/// the thing is; the other fields are caches of it. Nothing outside this
/// module writes them.
pub struct Thing<T> {
    template: Arc<T>,
    transform: Matrix3D,
    cached_pos: Coord3D,
    cached_angle: f32,
    cached_dir_vector: Coord3D,
    cached_altitude_above_terrain: f32,
    cached_altitude_above_terrain_or_water: f32,
    cache_flags: u32,
}

impl<T: ThingTemplateView> Thing<T> {
    /// `Thing(const ThingTemplate *)`. Identity transform at the origin, all
    /// caches empty. `None` is undefined upstream (`DEBUG_CRASH`).
    pub fn new(thing_template: Option<Arc<T>>) -> Result<Self, ThingError> {
        let template = thing_template.ok_or(ThingError::NoTemplate)?;
        Ok(Thing {
            template,
            transform: Matrix3D::IDENTITY,
            cached_pos: Coord3D::default(),
            cached_angle: 0.0,
            cached_dir_vector: Coord3D::default(),
            cached_altitude_above_terrain: 0.0,
            cached_altitude_above_terrain_or_water: 0.0,
            cache_flags: 0,
        })
    }

    /// `getTemplate()`.
    pub fn template(&self) -> &Arc<T> {
        &self.template
    }

    /// `isKindOf(t)`: forwarded to the template.
    pub fn is_kind_of(&self, t: KindOfType) -> bool {
        self.template.is_kind_of(t)
    }

    /// `isKindOfMulti(mustBeSet, mustBeClear)`: forwarded to the template.
    pub fn is_kind_of_multi(&self, must_be_set: &KindOfMaskType, must_be_clear: &KindOfMaskType) -> bool {
        self.template.is_kind_of_multi(must_be_set, must_be_clear)
    }

    /// `isAnyKindOf(anyKindOf)`: forwarded to the template.
    pub fn is_any_kind_of(&self, any_kind_of: &KindOfMaskType) -> bool {
        self.template.is_any_kind_of(any_kind_of)
    }

    /// `setPosition(pos)`. A thing that sticks to the terrain slope is
    /// aligned on the terrain at its current orientation instead.
    pub fn set_position<C: ThingContext>(&mut self, ctx: &C, pos: Option<&Coord3D>) -> Result<TransformChange, ThingError> {
        let pos = pos.ok_or(ThingError::NullPosition)?;
        if !self.is_kind_of(KINDOF_STICK_TO_TERRAIN_SLOPE) {
            let change = self.change_record();
            self.transform.set_x_translation(pos.x);
            self.transform.set_y_translation(pos.y);
            self.transform.set_z_translation(pos.z);
            self.cached_pos = *pos;
            // but don't clear the dir flags.
            self.cache_flags &= !(VALID_ALTITUDE_TERRAIN | VALID_ALTITUDE_SEALEVEL);
            Ok(change)
        } else {
            let mtx = ctx.align_on_terrain(self.cached_angle, pos, true);
            Ok(self.set_transform_matrix(&mtx))
        }
    }

    /// `setPositionZ(z)`. Upstream says it is cheap because the cached
    /// altitudes are moved by the change in height instead of recomputed.
    pub fn set_position_z<C: ThingContext>(&mut self, ctx: &C, z: f32) -> TransformChange {
        if !self.is_kind_of(KINDOF_STICK_TO_TERRAIN_SLOPE) {
            let change = self.change_record();
            let old_z = self.cached_pos.z;
            self.transform.set_z_translation(z);
            self.cached_pos.z = z;
            if self.cache_flags & VALID_ALTITUDE_TERRAIN != 0 {
                self.cached_altitude_above_terrain += z - old_z;
            }
            if self.cache_flags & VALID_ALTITUDE_SEALEVEL != 0 {
                self.cached_altitude_above_terrain_or_water += z - old_z;
            }
            change
        } else {
            let mut pos = self.cached_pos;
            pos.z = z;
            let mtx = ctx.align_on_terrain(self.cached_angle, &pos, true);
            self.set_transform_matrix(&mtx)
        }
    }

    /// `setOrientation(angle)`. Upstream forces the thing straight up on the
    /// Z axis, or aligns it on the terrain for a sticking thing. Use
    /// [`Thing::set_transform_matrix`] to keep some other orientation.
    pub fn set_orientation<C: ThingContext>(&mut self, ctx: &C, angle: f32) -> Result<TransformChange, ThingError> {
        // Upstream normalises after writing the transform, and never returns
        // for an angle that does not reduce. Checking first leaves the thing
        // untouched on that undefined input.
        let normalized = try_normalize_angle(angle).map_err(|_| ThingError::AngleNeverNormalizes)?;
        let change = self.change_record();
        let pos = Coord3D {
            x: self.transform.get_x_translation(),
            y: self.transform.get_y_translation(),
            z: self.transform.get_z_translation(),
        };
        if self.is_kind_of(KINDOF_STICK_TO_TERRAIN_SLOPE) {
            self.transform = ctx.align_on_terrain(angle, &pos, true);
        } else {
            let z = Coord3D { x: 0.0, y: 0.0, z: 1.0 };
            let u = Coord3D { x: trig::cos(angle), y: trig::sin(angle), z: 0.0 };
            let y = Coord3D::cross_product(&z, &u);
            let x = Coord3D::cross_product(&y, &z);
            self.transform = Matrix3D::new(
                x.x, y.x, z.x, pos.x, //
                x.y, y.y, z.y, pos.y, //
                x.z, y.z, z.z, pos.z,
            );
        }
        self.cached_angle = normalized;
        // Upstream caches the position from before the transform was
        // rewritten (see the module notes); kept as it is.
        self.cached_pos = pos;
        // but don't clear the altitude flags.
        self.cache_flags &= !VALID_DIRVECTOR;
        Ok(change)
    }

    /// `getPosition()`: the cached position, which may differ from the
    /// transform's translation after a sticking thing's `setOrientation`.
    pub fn position(&self) -> &Coord3D {
        &self.cached_pos
    }

    /// `getOrientation()`: the cached angle (normalised after `setOrientation`,
    /// the raw Z rotation after `setTransformMatrix`).
    pub fn orientation(&self) -> f32 {
        self.cached_angle
    }

    /// `getUnitDirectionVector2D()`: `(cos a, sin a, 0)` for the orientation,
    /// cached until the orientation changes.
    pub fn unit_direction_vector_2d(&mut self) -> Coord3D {
        if self.cache_flags & VALID_DIRVECTOR == 0 {
            let angle = self.orientation();
            self.cached_dir_vector = Coord3D { x: trig::cos(angle), y: trig::sin(angle), z: 0.0 };
            self.cache_flags |= VALID_DIRVECTOR;
        }
        self.cached_dir_vector
    }

    /// `getUnitDirectionVector3D(dir)`: the normalised X axis of the transform.
    pub fn unit_direction_vector_3d(&self) -> Coord3D {
        let mut v = self.transform.get_x_vector();
        v.normalize();
        Coord3D { x: v.x, y: v.y, z: v.z }
    }

    /// `getHeightAboveTerrain()`, cached, with the base terrain calculation.
    pub fn height_above_terrain<C: ThingContext>(&mut self, ctx: &C) -> f32 {
        self.height_above_terrain_by(|t| t.calculate_height_above_terrain(ctx))
    }

    /// `getHeightAboveTerrain()` with the owner's own calculation (cut point 4).
    pub fn height_above_terrain_by(&mut self, calc: impl FnOnce(&Self) -> f32) -> f32 {
        if self.cache_flags & VALID_ALTITUDE_TERRAIN == 0 {
            self.cached_altitude_above_terrain = calc(self);
            self.cache_flags |= VALID_ALTITUDE_TERRAIN;
        }
        self.cached_altitude_above_terrain
    }

    /// `getHeightAboveTerrainOrWater()`, cached: the height above the water
    /// surface when the point is under water, else the height above terrain.
    pub fn height_above_terrain_or_water<C: ThingContext>(&mut self, ctx: &C) -> f32 {
        self.height_above_terrain_or_water_by(ctx, |t| t.calculate_height_above_terrain(ctx))
    }

    /// `getHeightAboveTerrainOrWater()` with the owner's terrain calculation (cut point 4).
    pub fn height_above_terrain_or_water_by<C: ThingContext>(
        &mut self,
        ctx: &C,
        calc: impl FnOnce(&Self) -> f32,
    ) -> f32 {
        if self.cache_flags & VALID_ALTITUDE_SEALEVEL == 0 {
            let pos = self.cached_pos;
            self.cached_altitude_above_terrain_or_water = match ctx.underwater_level(pos.x, pos.y) {
                Some(water_z) => pos.z - water_z,
                None => self.height_above_terrain_by(calc),
            };
            self.cache_flags |= VALID_ALTITUDE_SEALEVEL;
        }
        self.cached_altitude_above_terrain_or_water
    }

    /// `isAboveTerrain()`: `getHeightAboveTerrain() > 0`.
    pub fn is_above_terrain<C: ThingContext>(&mut self, ctx: &C) -> bool {
        self.height_above_terrain(ctx) > 0.0
    }

    /// `isAboveTerrainOrWater()`: `getHeightAboveTerrainOrWater() > 0`.
    pub fn is_above_terrain_or_water<C: ThingContext>(&mut self, ctx: &C) -> bool {
        self.height_above_terrain_or_water(ctx) > 0.0
    }

    /// `isSignificantlyAboveTerrain()`: high enough that it would take more
    /// than three frames to fall back, so the thing flies rather than slides.
    pub fn is_significantly_above_terrain<C: ThingContext>(&mut self, ctx: &C) -> bool {
        // `-(3*3)*TheGlobalData->m_gravity`: the int -9, converted to float, times gravity.
        let threshold = (-(3 * 3)) as f32 * ctx.gravity();
        self.height_above_terrain(ctx) > threshold
    }

    /// The base `calculateHeightAboveTerrain()`: uncached, ground height
    /// under the cached position.
    pub fn calculate_height_above_terrain<C: ThingContext>(&self, ctx: &C) -> f32 {
        let pos = &self.cached_pos;
        let terrain_z = ctx.ground_height(pos.x, pos.y);
        pos.z - terrain_z
    }

    /// `convertBonePosToWorldPos`, the transform part: the bone's transform
    /// in the world (`m_transform * boneTransform`).
    pub fn bone_transform_to_world(&self, bone_transform: &Matrix3D) -> Matrix3D {
        Matrix3D::multiply(&self.transform, bone_transform)
    }

    /// `convertBonePosToWorldPos`, the position part: the bone position
    /// pushed through the transform.
    pub fn bone_position_to_world(&self, bone_pos: &Coord3D) -> Coord3D {
        let v = Matrix3D::transform_vector(&self.transform, &Vector3::new(bone_pos.x, bone_pos.y, bone_pos.z));
        Coord3D { x: v.x, y: v.y, z: v.z }
    }

    /// `convertBonePosToWorldPos(bonePos, boneTransform, &worldPos, &worldTransform)`
    /// with both outputs wanted.
    pub fn convert_bone_pos_to_world_pos(&self, bone_pos: &Coord3D, bone_transform: &Matrix3D) -> (Coord3D, Matrix3D) {
        (self.bone_position_to_world(bone_pos), self.bone_transform_to_world(bone_transform))
    }

    /// `setTransformMatrix(mx)`: the world transform; every cache is
    /// refreshed from it (the Z rotation is not normalised, as upstream).
    pub fn set_transform_matrix(&mut self, mx: &Matrix3D) -> TransformChange {
        let change = self.change_record();
        self.transform = *mx;
        self.cached_pos = Coord3D {
            x: self.transform.get_x_translation(),
            y: self.transform.get_y_translation(),
            z: self.transform.get_z_translation(),
        };
        self.cached_angle = self.transform.get_z_rotation();
        self.cache_flags = 0;
        change
    }

    /// `getTransformMatrix()`.
    pub fn transform_matrix(&self) -> &Matrix3D {
        &self.transform
    }

    /// `transformPoint(in, out)`: `in` pushed through the transform into
    /// `out`. A `None` on either side does nothing, as upstream returns early.
    pub fn transform_point(&self, input: Option<&Coord3D>, output: Option<&mut Coord3D>) {
        if let (Some(i), Some(o)) = (input, output) {
            let v = Matrix3D::transform_vector(&self.transform, &Vector3::new(i.x, i.y, i.z));
            o.x = v.x;
            o.y = v.y;
            o.z = v.z;
        }
    }

    /// The old state a setter hands to the owner's reaction.
    fn change_record(&self) -> TransformChange {
        TransformChange {
            old_transform: self.transform,
            old_position: self.cached_pos,
            old_angle: self.cached_angle,
        }
    }
}
