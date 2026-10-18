// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Geometry.h
//   GeneralsMD/Code/GameEngine/Source/Common/System/Geometry.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Geometry.cpp: Steven Johnson, Aug 2002 (Geometry.h: Colin Day, Nov 2001)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `GeometryInfo`: the footprint and bounding volume of a template's shape
//! (`GEOMETRY_SPHERE`, `GEOMETRY_CYLINDER`, `GEOMETRY_BOX`), read from the
//! `Geometry*` fields of an `Object` block.
//!
//! The INI fields are parsed by [`parse_geometry_type`] and friends, and
//! [`GEOMETRY_FIELDS`] is the parse table `ThingTemplate.cpp` uses for them
//! (its other fields come with the thing-template layer). Every parse calls
//! `calcBoundingStuff`, as upstream does, so the bounding radii are always
//! consistent with the fields read so far.
//!
//! # Math
//!
//! Upstream calls the C library's `sin`, `cos`, `atan2` and `sqrt` on
//! `float` arguments. Under the MSVC headers the shipped game was built with
//! those are the `double` functions, rounded to `float` once (the convention
//! of [`crate::ww::wwmath`]), so they are evaluated here in `f64` the same
//! way. `sqrt` is exact in either precision.
//!
//! # Random offsets
//!
//! Upstream draws from the global `GameLogicRandomValueReal`; here the caller
//! passes the game's [`RandomValues`], and the draws happen in upstream's
//! order (a point's x before its y, and so on).
//!
//! # Not translated
//!
//! - `tweakExtents` and `getDescriptiveString` exist only in `_DEBUG` and
//!   `_INTERNAL` builds, which the shipped release does not compile.
//! - `DEBUG_CRASH` calls (`isIntersectedByLineSegment` treats every shape as
//!   a sphere "for now", and `makeRandomOffsetOnPerimeter` of a sphere or a
//!   cylinder is unimplemented) are compiled out in release builds. The
//!   translation does what the release build does: the sphere/cylinder
//!   perimeter point is the centre, with `z` left alone.

use super::base_type::{clamp, sqr, Coord2D, Coord3D, Region2D, PI};
use super::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, UserData};
use super::random_value::RandomValues;
use super::xfer::{Snapshot, Xfer, XferResult, XferStatus, XferVersion};
use petir::Real;

/// `GeometryNames` (`DEFINE_GEOMETRY_NAMES`), in `GeometryType` order.
pub const GEOMETRY_NAMES: &[&str] = &["SPHERE", "CYLINDER", "BOX"];

/// `enum GeometryType`. The discriminants are upstream's values, which the
/// partition manager's collide-test table depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum GeometryType {
    /// `GEOMETRY_SPHERE`: `majorRadius` is the radius.
    Sphere = 0,
    /// `GEOMETRY_CYLINDER`: `majorRadius` is the radius, `height` the height.
    Cylinder = 1,
    /// `GEOMETRY_BOX`: `majorRadius` and `minorRadius` are the half-lengths
    /// forward and to the side.
    Box = 2,
}

impl GeometryType {
    /// The value of an index into [`GEOMETRY_NAMES`], or `None` outside it
    /// (upstream's `(GeometryType)` cast of an `Int`).
    pub fn from_index(i: i32) -> Option<Self> {
        match i {
            0 => Some(GeometryType::Sphere),
            1 => Some(GeometryType::Cylinder),
            2 => Some(GeometryType::Box),
            _ => None,
        }
    }
}

/// `class GeometryInfo` (`Snapshot`): the data members in upstream's order.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryInfo {
    geom_type: GeometryType,
    /// If true, the geometry fits in one partition cell.
    is_small: bool,
    height: f32,
    major_radius: f32,
    minor_radius: f32,
    /// Not in the INI file: the radius of the bounding circle (2D).
    bounding_circle_radius: f32,
    /// Not in the INI file: the radius of the bounding sphere (3D).
    bounding_sphere_radius: f32,
}

/// `sqrt` of a `float` sum, through C's `double` `sqrt` (exact either way).
fn c_sqrt(v: f32) -> f32 {
    Real::sqrt(f64::from(v)) as f32
}

/// `(Real)cos(angle)` of a `float`, through the `double` `cos`.
fn c_cos(angle: f32) -> f32 {
    Real::cos(f64::from(angle)) as f32
}

/// `(Real)sin(angle)` of a `float`, through the `double` `sin`.
fn c_sin(angle: f32) -> f32 {
    Real::sin(f64::from(angle)) as f32
}

fn calc_dot_product(a: &Coord3D, b: &Coord3D) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn calc_dist_squared(a: &Coord3D, b: &Coord3D) -> f32 {
    sqr(a.x - b.x) + sqr(a.y - b.y) + sqr(a.z - b.z)
}

/// `calcPointToLineDistSquared` (Geometry.cpp, file-local).
fn calc_point_to_line_dist_squared(pt: &Coord3D, line_start: &Coord3D, line_end: &Coord3D) -> f32 {
    let line = Coord3D {
        x: line_end.x - line_start.x,
        y: line_end.y - line_start.y,
        z: line_end.z - line_start.z,
    };
    let line_to_pt = Coord3D {
        x: pt.x - line_start.x,
        y: pt.y - line_start.y,
        z: pt.z - line_start.z,
    };

    let dot = calc_dot_product(&line_to_pt, &line);
    if dot <= 0.0f32 {
        // angle is obtuse
        return calc_dist_squared(pt, line_start);
    }

    let line_len_sqr = calc_dist_squared(line_start, line_end);
    if line_len_sqr <= dot {
        return calc_dist_squared(pt, line_end);
    }

    let tmp = dot / line_len_sqr;
    let closest = Coord3D {
        x: line_start.x + tmp * line.x,
        y: line_start.y + tmp * line.y,
        z: line_start.z + tmp * line.z,
    };
    calc_dist_squared(pt, &closest)
}

/// `isWithin(a, b, c)`.
fn is_within(a: f32, b: f32, c: f32) -> bool {
    a <= b && b <= c
}

impl GeometryInfo {
    /// `GeometryInfo(type, isSmall, height, majorRadius, minorRadius)`.
    pub fn new(geom_type: GeometryType, is_small: bool, height: f32, major_radius: f32, minor_radius: f32) -> Self {
        let mut g = GeometryInfo {
            geom_type,
            is_small: false,
            height: 0.0,
            major_radius: 0.0,
            minor_radius: 0.0,
            bounding_circle_radius: 0.0,
            bounding_sphere_radius: 0.0,
        };
        g.set(geom_type, is_small, height, major_radius, minor_radius);
        g
    }

    /// `calcBoundingStuff`.
    fn calc_bounding_stuff(&mut self) {
        match self.geom_type {
            GeometryType::Sphere => {
                self.bounding_sphere_radius = self.major_radius;
                self.bounding_circle_radius = self.major_radius;
            }
            GeometryType::Cylinder => {
                self.bounding_circle_radius = self.major_radius;
                // `m_height*0.5` is a `double` product rounded to `Real`;
                // multiplying by 0.5 is exact, so the `float` product is the same.
                self.bounding_sphere_radius = self.height * 0.5;
                if self.bounding_sphere_radius < self.major_radius {
                    self.bounding_sphere_radius = self.major_radius;
                }
            }
            GeometryType::Box => {
                let planar = sqr(self.major_radius) + sqr(self.minor_radius);
                self.bounding_circle_radius = c_sqrt(planar);
                // `sqr(m_height*0.5)`: `0.5` is a `double` literal, so the
                // half height is a `double` and so is its square (no overflow
                // at a `float` height near 2^64). The sum is `float + double`,
                // so it is taken in `double`, and `sqrt` is the `double` one.
                let half = f64::from(self.height) * 0.5;
                self.bounding_sphere_radius = Real::sqrt(f64::from(planar) + half * half) as f32;
            }
        }
    }

    /// `set(type, isSmall, height, majorRadius, minorRadius)`: which of the
    /// arguments a shape keeps (a sphere takes its radius for all three
    /// extents, a cylinder its radius and height, a box all of them).
    pub fn set(&mut self, geom_type: GeometryType, is_small: bool, height: f32, major_radius: f32, minor_radius: f32) {
        self.geom_type = geom_type;
        self.is_small = is_small;
        match geom_type {
            GeometryType::Sphere => {
                self.major_radius = major_radius;
                self.minor_radius = major_radius;
                self.height = major_radius;
            }
            GeometryType::Cylinder => {
                self.major_radius = major_radius;
                self.minor_radius = major_radius;
                self.height = height;
            }
            GeometryType::Box => {
                self.major_radius = major_radius;
                self.minor_radius = minor_radius;
                self.height = height;
            }
        }
        self.calc_bounding_stuff();
    }

    /// `setMajorRadius` ("legacy code"): sets it and recalculates the bounds.
    pub fn set_major_radius(&mut self, major_radius: f32) {
        self.major_radius = major_radius;
        self.calc_bounding_stuff();
    }

    /// `setMinorRadius`.
    pub fn set_minor_radius(&mut self, minor_radius: f32) {
        self.minor_radius = minor_radius;
        self.calc_bounding_stuff();
    }

    /// `getGeomType`.
    pub fn get_geom_type(&self) -> GeometryType {
        self.geom_type
    }
    /// `getIsSmall`.
    pub fn get_is_small(&self) -> bool {
        self.is_small
    }
    /// `getMajorRadius` (the x-axis).
    pub fn get_major_radius(&self) -> f32 {
        self.major_radius
    }
    /// `getMinorRadius` (the y-axis).
    pub fn get_minor_radius(&self) -> f32 {
        self.minor_radius
    }
    /// The stored height (`m_height`), which xfer writes. Upstream has no
    /// getter for it (`getGeomHeight` was removed); for a sphere it is the
    /// radius it was set to, which `getMaxHeightAbovePosition` does not show.
    pub fn get_height(&self) -> f32 {
        self.height
    }

    /// `getBoundingCircleRadius`.
    pub fn get_bounding_circle_radius(&self) -> f32 {
        self.bounding_circle_radius
    }
    /// `getBoundingSphereRadius`.
    pub fn get_bounding_sphere_radius(&self) -> f32 {
        self.bounding_sphere_radius
    }

    /// `isIntersectedByLineSegment`: upstream treats every shape as a sphere
    /// ("does not work properly for nonspheres yet").
    pub fn is_intersected_by_line_segment(&self, loc: &Coord3D, from: &Coord3D, to: &Coord3D) -> bool {
        let dist_squared = calc_point_to_line_dist_squared(loc, from, to);
        dist_squared <= sqr(self.get_bounding_sphere_radius())
    }

    /// `getFootprintArea`.
    pub fn get_footprint_area(&self) -> f32 {
        match self.geom_type {
            GeometryType::Sphere | GeometryType::Cylinder => PI * sqr(self.bounding_circle_radius),
            GeometryType::Box => 4.0f32 * self.major_radius * self.minor_radius,
        }
    }

    /// `getMaxHeightAbovePosition`: how far above the canonical position the
    /// geometry's top reaches.
    pub fn get_max_height_above_position(&self) -> f32 {
        match self.geom_type {
            GeometryType::Sphere => self.major_radius,
            GeometryType::Box | GeometryType::Cylinder => self.height,
        }
    }

    /// `getMaxHeightBelowPosition`: how far below the canonical position the
    /// geometry's bottom reaches.
    pub fn get_max_height_below_position(&self) -> f32 {
        match self.geom_type {
            GeometryType::Sphere => self.major_radius,
            GeometryType::Box | GeometryType::Cylinder => 0.0,
        }
    }

    /// `setMaxHeightAbovePosition`: sets the top's height (the radius of a
    /// sphere, the height of a box or cylinder) and recalculates the bounds.
    pub fn set_max_height_above_position(&mut self, z: f32) {
        match self.geom_type {
            GeometryType::Sphere => self.major_radius = z,
            GeometryType::Box | GeometryType::Cylinder => self.height = z,
        }
        self.calc_bounding_stuff();
    }

    /// `getZDeltaToCenterPosition`: 0 for a sphere, half the height otherwise.
    pub fn get_z_delta_to_center_position(&self) -> f32 {
        if self.geom_type == GeometryType::Sphere {
            0.0
        } else {
            self.height * 0.5f32
        }
    }

    /// `getCenterPosition(pos, center)`: `pos` lifted to the geometry's centre.
    pub fn get_center_position(&self, pos: &Coord3D) -> Coord3D {
        let mut center = *pos;
        center.z += self.get_z_delta_to_center_position();
        center
    }

    /// `calcPitches(thisPos, that, thatPos, minPitch, maxPitch)`: the lowest
    /// and highest pitch, as `(minPitch, maxPitch)`, from this geometry's
    /// centre to `that` (taken at its top-centre and bottom-centre).
    pub fn calc_pitches(&self, this_pos: &Coord3D, that: &GeometryInfo, that_pos: &Coord3D) -> (f32, f32) {
        let this_center = self.get_center_position(this_pos);

        let dxy = c_sqrt(sqr(that_pos.x - this_center.x) + sqr(that_pos.y - this_center.y));

        let dz = (that_pos.z + that.get_max_height_above_position()) - this_center.z;
        let max_pitch = crate::ww::wwmath::atan2(dz, dxy);

        let dz = (that_pos.z - that.get_max_height_below_position()) - this_center.z;
        let min_pitch = crate::ww::wwmath::atan2(dz, dxy);
        (min_pitch, max_pitch)
    }

    /// `expandFootprint(radius)`: grows both horizontal radii.
    pub fn expand_footprint(&mut self, radius: f32) {
        self.major_radius += radius;
        self.minor_radius += radius;
        self.calc_bounding_stuff();
    }

    /// `get2DBounds(geomCenter, angle, bounds)`: the axis-aligned box of the
    /// footprint, turned by `angle` (box only; a sphere or cylinder is the
    /// square around its circle).
    pub fn get_2d_bounds(&self, geom_center: &Coord3D, angle: f32) -> Region2D {
        match self.geom_type {
            GeometryType::Sphere | GeometryType::Cylinder => Region2D {
                lo: Coord2D {
                    x: geom_center.x - self.major_radius,
                    y: geom_center.y - self.major_radius,
                },
                hi: Coord2D {
                    x: geom_center.x + self.major_radius,
                    y: geom_center.y + self.major_radius,
                },
            },
            GeometryType::Box => {
                let c = c_cos(angle);
                let s = c_sin(angle);
                let exc = self.major_radius * c;
                let eyc = self.minor_radius * c;
                let exs = self.major_radius * s;
                let eys = self.minor_radius * s;

                let x = geom_center.x - exc - eys;
                let y = geom_center.y + eyc - exs;
                let mut b = Region2D {
                    lo: Coord2D { x, y },
                    hi: Coord2D { x, y },
                };

                let corners = [
                    (geom_center.x + exc - eys, geom_center.y + eyc + exs),
                    (geom_center.x + exc + eys, geom_center.y - eyc + exs),
                    (geom_center.x - exc + eys, geom_center.y - eyc - exs),
                ];
                for (x, y) in corners {
                    if b.lo.x > x {
                        b.lo.x = x;
                    }
                    if b.lo.y > y {
                        b.lo.y = y;
                    }
                    if b.hi.x < x {
                        b.hi.x = x;
                    }
                    if b.hi.y < y {
                        b.hi.y = y;
                    }
                }
                b
            }
        }
    }

    /// `makeRandomOffsetWithinFootprint(pt)`: a random point inside the
    /// footprint, with `z = 0`. A disc is sampled by rejection from its
    /// bounding square, which spreads the points evenly (sampling radius and
    /// angle would bunch them at the centre).
    ///
    /// Precondition: a sphere or cylinder needs `major_radius >= 0` (or NaN).
    /// With a negative radius the rejection loop never ends, upstream and
    /// here alike; the differential test does not send such inputs.
    pub fn make_random_offset_within_footprint(&self, rng: &mut RandomValues, pt: &mut Coord3D) {
        match self.geom_type {
            GeometryType::Sphere | GeometryType::Cylinder => {
                // upstream's `do { ... } while (distSqr > maxDistSqr)`; a NaN is
                // not greater, so the loop ends on NaN, as upstream's does
                let max_dist_sqr = sqr(self.major_radius);
                let mut again = true;
                while again {
                    pt.x = rng.game_logic_random_value_real(-self.major_radius, self.major_radius);
                    pt.y = rng.game_logic_random_value_real(-self.major_radius, self.major_radius);
                    pt.z = 0.0f32;
                    let dist_sqr = sqr(pt.x) + sqr(pt.y);
                    again = dist_sqr > max_dist_sqr;
                }
            }
            GeometryType::Box => {
                pt.x = rng.game_logic_random_value_real(-self.major_radius, self.major_radius);
                pt.y = rng.game_logic_random_value_real(-self.minor_radius, self.minor_radius);
                pt.z = 0.0f32;
            }
        }
    }

    /// `makeRandomOffsetOnPerimeter(pt)`: a random point on the border of a
    /// box. A sphere or cylinder has no perimeter here: the point is the
    /// centre (release builds), with `z` left as it was.
    pub fn make_random_offset_on_perimeter(&self, rng: &mut RandomValues, pt: &mut Coord3D) {
        match self.geom_type {
            GeometryType::Sphere | GeometryType::Cylinder => {
                pt.x = 0.0f32;
                pt.y = 0.0f32;
            }
            GeometryType::Box => {
                if rng.game_logic_random_value_real(0.0f32, 1.0f32) < 0.5f32 {
                    // A random point on the x axis, and y at one of its extremes.
                    pt.x = rng.game_logic_random_value_real(-self.major_radius, self.major_radius);
                    pt.y = if rng.game_logic_random_value_real(0.0f32, 1.0f32) < 0.5f32 {
                        -self.minor_radius
                    } else {
                        self.minor_radius
                    };
                } else {
                    // A random point on the y axis, and x at one of its extremes.
                    pt.y = rng.game_logic_random_value_real(-self.minor_radius, self.minor_radius);
                    pt.x = if rng.game_logic_random_value_real(0.0f32, 1.0f32) < 0.5f32 {
                        -self.major_radius
                    } else {
                        self.major_radius
                    };
                }
                pt.z = 0.0f32;
            }
        }
    }

    /// `clipPointToFootprint(geomCenter, ptToClip)`: moves a point that lies
    /// outside the footprint onto its edge (radially for a disc, per axis for
    /// a box). `z` is not touched.
    pub fn clip_point_to_footprint(&self, geom_center: &Coord3D, pt_to_clip: &mut Coord3D) {
        match self.geom_type {
            GeometryType::Sphere | GeometryType::Cylinder => {
                let dx = pt_to_clip.x - geom_center.x;
                let dy = pt_to_clip.y - geom_center.y;
                let radius = c_sqrt(sqr(dx) + sqr(dy));
                if radius > self.major_radius {
                    let ratio = self.major_radius / radius;
                    pt_to_clip.x = geom_center.x + dx * ratio;
                    pt_to_clip.y = geom_center.y + dy * ratio;
                }
            }
            GeometryType::Box => {
                pt_to_clip.x = clamp(
                    geom_center.x - self.major_radius,
                    pt_to_clip.x,
                    geom_center.x + self.major_radius,
                );
                pt_to_clip.y = clamp(
                    geom_center.y - self.minor_radius,
                    pt_to_clip.y,
                    geom_center.y + self.minor_radius,
                );
            }
        }
    }

    /// `isPointInFootprint(geomCenter, pt)`: the horizontal test only.
    pub fn is_point_in_footprint(&self, geom_center: &Coord3D, pt: &Coord3D) -> bool {
        match self.geom_type {
            GeometryType::Sphere | GeometryType::Cylinder => {
                let dx = pt.x - geom_center.x;
                let dy = pt.y - geom_center.y;
                let radius = c_sqrt(sqr(dx) + sqr(dy));
                radius <= self.major_radius
            }
            GeometryType::Box => {
                is_within(geom_center.x - self.major_radius, pt.x, geom_center.x + self.major_radius)
                    && is_within(geom_center.y - self.minor_radius, pt.y, geom_center.y + self.minor_radius)
            }
        }
    }
}

impl Snapshot for GeometryInfo {
    /// `crc`: empty upstream (the light CRC does not cover geometry).
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `xfer`, version 1: type (the enum's 4 bytes), is-small, height, the
    /// two radii, and the two derived radii.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;

        // `xferUser(&m_type, sizeof(GeometryType))`: the enum as an `int`.
        let mut type_bytes = (self.geom_type as i32).to_le_bytes();
        xfer.xfer_user(&mut type_bytes)?;
        // A value outside the enum (a corrupt save) has no `GeometryType`.
        self.geom_type = GeometryType::from_index(i32::from_le_bytes(type_bytes))
            .ok_or(XferStatus::InvalidParameters)?;

        xfer.xfer_bool(&mut self.is_small)?;
        xfer.xfer_real(&mut self.height)?;
        xfer.xfer_real(&mut self.major_radius)?;
        xfer.xfer_real(&mut self.minor_radius)?;
        xfer.xfer_real(&mut self.bounding_circle_radius)?;
        xfer.xfer_real(&mut self.bounding_sphere_radius)?;
        Ok(())
    }

    /// `loadPostProcess`: empty upstream.
    fn load_post_process(&mut self) {}
}

// ---------------------------------------------------------------------------
// INI field parsers (static members of GeometryInfo upstream)
// ---------------------------------------------------------------------------

/// `GeometryInfo::parseGeometryType`: `Geometry = SPHERE | CYLINDER | BOX`.
pub fn parse_geometry_type(ini: &mut Ini<'_>, g: &mut GeometryInfo, _ud: &UserData) -> IniResult<()> {
    let index = ini.parse_index_list(GEOMETRY_NAMES)?;
    // `scanIndexList` returns an index into `GEOMETRY_NAMES`, always in range.
    g.geom_type = GeometryType::from_index(index)
        .ok_or_else(|| IniError::new(IniErrorKind::InvalidNameList))?;
    g.calc_bounding_stuff();
    Ok(())
}

/// `GeometryInfo::parseGeometryIsSmall`: `GeometryIsSmall = Yes | No`.
pub fn parse_geometry_is_small(ini: &mut Ini<'_>, g: &mut GeometryInfo, _ud: &UserData) -> IniResult<()> {
    g.is_small = ini.parse_bool()?;
    g.calc_bounding_stuff();
    Ok(())
}

/// `GeometryInfo::parseGeometryHeight`.
pub fn parse_geometry_height(ini: &mut Ini<'_>, g: &mut GeometryInfo, _ud: &UserData) -> IniResult<()> {
    g.height = ini.parse_real()?;
    g.calc_bounding_stuff();
    Ok(())
}

/// `GeometryInfo::parseGeometryMajorRadius`.
pub fn parse_geometry_major_radius(ini: &mut Ini<'_>, g: &mut GeometryInfo, _ud: &UserData) -> IniResult<()> {
    g.major_radius = ini.parse_real()?;
    g.calc_bounding_stuff();
    Ok(())
}

/// `GeometryInfo::parseGeometryMinorRadius`.
pub fn parse_geometry_minor_radius(ini: &mut Ini<'_>, g: &mut GeometryInfo, _ud: &UserData) -> IniResult<()> {
    g.minor_radius = ini.parse_real()?;
    g.calc_bounding_stuff();
    Ok(())
}

/// The five `Geometry*` entries of `ThingTemplate.cpp`'s field table, as
/// the thing-template parser will use them.
pub static GEOMETRY_FIELDS: [FieldParse<GeometryInfo>; 6] = [
    FieldParse::new("Geometry", parse_geometry_type, UserData::None),
    FieldParse::new("GeometryMajorRadius", parse_geometry_major_radius, UserData::None),
    FieldParse::new("GeometryMinorRadius", parse_geometry_minor_radius, UserData::None),
    FieldParse::new("GeometryHeight", parse_geometry_height, UserData::None),
    FieldParse::new("GeometryIsSmall", parse_geometry_is_small, UserData::None),
    FieldParse::end(),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: f32, y: f32, z: f32) -> Coord3D {
        Coord3D { x, y, z }
    }

    #[test]
    fn type_index_round_trips_and_rejects_outside_the_names() {
        for (i, t) in [GeometryType::Sphere, GeometryType::Cylinder, GeometryType::Box].into_iter().enumerate() {
            assert_eq!(GeometryType::from_index(i as i32), Some(t));
            assert_eq!(t as i32, i as i32);
        }
        assert_eq!(GEOMETRY_NAMES.len(), 3);
        assert_eq!(GeometryType::from_index(3), None);
        assert_eq!(GeometryType::from_index(-1), None);
    }

    #[test]
    fn a_sphere_takes_its_radius_for_every_extent() {
        let g = GeometryInfo::new(GeometryType::Sphere, true, 99.0, 5.0, 7.0);
        assert!(g.get_is_small());
        assert_eq!(g.get_height(), 5.0);
        assert_eq!(g.get_minor_radius(), 5.0);
        assert_eq!(g.get_bounding_sphere_radius(), 5.0);
        assert_eq!(g.get_max_height_above_position(), 5.0);
        assert_eq!(g.get_max_height_below_position(), 5.0);
    }

    #[test]
    fn a_box_has_the_pythagorean_bounds_and_the_footprint_area() {
        let g = GeometryInfo::new(GeometryType::Box, false, 12.0, 3.0, 4.0);
        assert_eq!(g.get_bounding_circle_radius(), 5.0);
        assert_eq!(g.get_bounding_sphere_radius(), 61.0f32.sqrt());
        assert_eq!(g.get_footprint_area(), 48.0);
        assert_eq!(g.get_z_delta_to_center_position(), 6.0);
        assert_eq!(g.get_max_height_below_position(), 0.0);
    }

    #[test]
    fn a_tall_box_bounding_sphere_does_not_overflow_at_large_heights() {
        // the half height is squared in double, as upstream's sqr(m_height*0.5) is
        let g = GeometryInfo::new(GeometryType::Box, false, 6.0e19, 1.0, 1.0);
        assert!(g.get_bounding_sphere_radius().is_finite());
        assert_eq!(g.get_bounding_sphere_radius(), 3.0e19);
    }

    #[test]
    fn the_pitches_span_the_top_and_bottom_of_the_other_shape() {
        let me = GeometryInfo::new(GeometryType::Sphere, false, 0.0, 1.0, 1.0);
        let other = GeometryInfo::new(GeometryType::Sphere, false, 0.0, 1.0, 1.0);
        let (min, max) = me.calc_pitches(&c(0.0, 0.0, 0.0), &other, &c(10.0, 0.0, 10.0));
        // the top of `other` is 1 above its centre, its bottom 1 below (a sphere)
        assert_eq!(max, 11.0f32.atan2(10.0));
        assert_eq!(min, 9.0f32.atan2(10.0));
    }

    #[test]
    fn a_point_on_a_segment_intersects_and_a_far_point_does_not() {
        let g = GeometryInfo::new(GeometryType::Sphere, false, 0.0, 2.0, 2.0);
        assert!(g.is_intersected_by_line_segment(&c(5.0, 0.0, 0.0), &c(0.0, 0.0, 0.0), &c(10.0, 0.0, 0.0)));
        assert!(!g.is_intersected_by_line_segment(&c(5.0, 9.0, 0.0), &c(0.0, 0.0, 0.0), &c(10.0, 0.0, 0.0)));
    }

    #[test]
    fn an_axis_aligned_box_has_its_own_bounds() {
        let g = GeometryInfo::new(GeometryType::Box, false, 0.0, 3.0, 2.0);
        let b = g.get_2d_bounds(&c(10.0, 20.0, 0.0), 0.0);
        assert_eq!((b.lo.x, b.lo.y, b.hi.x, b.hi.y), (7.0, 18.0, 13.0, 22.0));
    }

    #[test]
    fn clipping_puts_an_outside_point_on_the_edge() {
        let disc = GeometryInfo::new(GeometryType::Cylinder, false, 4.0, 5.0, 5.0);
        let mut p = c(10.0, 0.0, 3.0);
        disc.clip_point_to_footprint(&c(0.0, 0.0, 0.0), &mut p);
        assert_eq!((p.x, p.y, p.z), (5.0, 0.0, 3.0));
        assert!(disc.is_point_in_footprint(&c(0.0, 0.0, 0.0), &p));
        let boxed = GeometryInfo::new(GeometryType::Box, false, 0.0, 1.0, 2.0);
        let mut q = c(-9.0, 9.0, 0.0);
        boxed.clip_point_to_footprint(&c(0.0, 0.0, 0.0), &mut q);
        assert_eq!((q.x, q.y), (-1.0, 2.0));
    }

    #[test]
    fn a_box_perimeter_point_is_on_its_border() {
        let g = GeometryInfo::new(GeometryType::Box, false, 0.0, 3.0, 2.0);
        let mut rng = RandomValues::new(7);
        for _ in 0..200 {
            let mut p = c(0.0, 0.0, 0.0);
            g.make_random_offset_on_perimeter(&mut rng, &mut p);
            let on_border = p.x.abs() == 3.0 || p.y.abs() == 2.0;
            assert!(on_border && p.x.abs() <= 3.0 && p.y.abs() <= 2.0, "{p:?}");
            assert_eq!(p.z, 0.0);
        }
    }

    #[test]
    fn expanding_and_resetting_the_radii_recalculates_the_bounds() {
        let mut g = GeometryInfo::new(GeometryType::Box, false, 0.0, 3.0, 4.0);
        g.expand_footprint(1.0);
        assert_eq!((g.get_major_radius(), g.get_minor_radius()), (4.0, 5.0));
        assert_eq!(g.get_bounding_circle_radius(), 41.0f32.sqrt());
        g.set_major_radius(0.0);
        assert_eq!(g.get_bounding_circle_radius(), 5.0);
        g.set_minor_radius(0.0);
        assert_eq!(g.get_bounding_circle_radius(), 0.0);
        g.set_max_height_above_position(8.0);
        assert_eq!(g.get_max_height_above_position(), 8.0);
    }

    #[test]
    fn the_parse_table_names_the_thing_template_fields() {
        let names: Vec<&str> = GEOMETRY_FIELDS.iter().filter_map(|f| f.token).collect();
        assert_eq!(
            names,
            ["Geometry", "GeometryMajorRadius", "GeometryMinorRadius", "GeometryHeight", "GeometryIsSmall"]
        );
    }
}
