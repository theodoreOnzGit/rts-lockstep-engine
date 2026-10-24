// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/PartitionManager.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp
//   (the collide tests, the four distance procs, `CollideInfo`,
//   `PartitionData::collidesWith`, `PartitionManager::geomCollidesWithGeom`)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, September 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Pure geometry of the partition manager: the nine shape-against-shape
//! collision tests and the four distance procedures. No state, no object
//! access; the manager hands them positions and [`GeometryInfo`]s.
//!
//! # Translation choices
//!
//! - **Function tables become `match`es.** Upstream indexes
//!   `theCollideTestProcs[(a - GEOMETRY_FIRST) * GEOMETRY_NUM_TYPES + (b - GEOMETRY_FIRST)]`
//!   and `theDistCalcProcs[dc]`; the Rust dispatches on the same two
//!   enumerations in the same order, so a test's behaviour is by construction
//!   the one upstream's table index picks. The `xyproc` function-pointer
//!   parameter of `z_collideTest_*` is an [`XyProc`] value.
//! - **Optional out-parameters** (`CollideLocAndNormal *cinfo`, null when the
//!   caller does not want them) are `Option<&mut CollideLocAndNormal>`. As
//!   upstream, `loc` and `normal` are written only when the result is `true`
//!   (and, for the sphere tests, only the fields each branch sets).
//! - **Maths.** `Cos`/`Sin` are the translated `common::trig` ones; `sqrtf` is
//!   `petir::Real::sqrt` in `f32`; there is no fused multiply-add anywhere.
//!   NaN follows C: every comparison with NaN is false, and the branch taken
//!   is the one C takes.

use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::geometry::{GeometryInfo, GeometryType};
use crate::common::trig;
use petir::Real;

/// `HUGE_DIST` (`PartitionManager.h`).
pub const HUGE_DIST: f32 = 1_000_000.0;
/// `HUGE_DIST_SQR` (`PartitionManager.cpp`): `HUGE_DIST*HUGE_DIST` in `float`.
pub const HUGE_DIST_SQR: f32 = HUGE_DIST * HUGE_DIST;

/// `enum DistanceCalculationType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DistanceCalculationType {
    /// `FROM_CENTER_2D`: measure from object centre in 2d.
    FromCenter2D = 0,
    /// `FROM_CENTER_3D`.
    FromCenter3D = 1,
    /// `FROM_BOUNDINGSPHERE_2D`: measure from the bounding circle in 2d.
    FromBoundingSphere2D = 2,
    /// `FROM_BOUNDINGSPHERE_3D`.
    FromBoundingSphere3D = 3,
}

impl DistanceCalculationType {
    /// The enumerator for an index, `None` outside `0..4` (upstream indexes
    /// its table with the raw value).
    pub fn from_index(i: i32) -> Option<Self> {
        match i {
            0 => Some(Self::FromCenter2D),
            1 => Some(Self::FromCenter3D),
            2 => Some(Self::FromBoundingSphere2D),
            3 => Some(Self::FromBoundingSphere3D),
            _ => None,
        }
    }
}

/// `struct CollideLocAndNormal`: optional results of a collision test.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CollideLocAndNormal {
    /// `loc`.
    pub loc: Coord3D,
    /// `normal`.
    pub normal: Coord3D,
}

/// `struct CollideInfo` (local to `PartitionManager.cpp`).
#[derive(Clone, Debug)]
pub struct CollideInfo {
    /// `position`.
    pub position: Coord3D,
    /// `geom`.
    pub geom: GeometryInfo,
    /// `angle`.
    pub angle: f32,
}

impl CollideInfo {
    /// `CollideInfo(p, g, a)`.
    pub fn new(p: &Coord3D, g: &GeometryInfo, a: f32) -> CollideInfo {
        CollideInfo {
            position: *p,
            geom: g.clone(),
            angle: a,
        }
    }
}

/// The 2-d test a z-test hands over to (`CollideTestProc xyproc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum XyProc {
    CircleCircle,
    CircleRect,
    RectCircle,
    RectRect,
}

fn xy_dispatch(
    p: XyProc,
    a: &CollideInfo,
    b: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    match p {
        XyProc::CircleCircle => xy_collide_test_circle_circle(a, b, cinfo),
        XyProc::CircleRect => xy_collide_test_circle_rect(a, b, cinfo),
        XyProc::RectCircle => xy_collide_test_rect_circle(a, b, cinfo),
        XyProc::RectRect => xy_collide_test_rect_rect(a, b, cinfo),
    }
}

fn vec_diff_2d(a: &Coord3D, b: &Coord3D) -> Coord3D {
    Coord3D {
        x: a.x - b.x,
        y: a.y - b.y,
        z: 0.0,
    }
}

fn vec_diff_3d(a: &Coord3D, b: &Coord3D) -> Coord3D {
    Coord3D {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

fn calc_sqr_dist_2d(d: &Coord3D) -> f32 {
    d.x * d.x + d.y * d.y
}

fn calc_sqr_dist_3d(d: &Coord3D) -> f32 {
    d.x * d.x + d.y * d.y + d.z * d.z
}

/// `projectCoord3D`.
fn project_coord3d(coord: &mut Coord3D, unit_dir: &Coord3D, dist: f32) {
    coord.x += unit_dir.x * dist;
    coord.y += unit_dir.y * dist;
    coord.z += unit_dir.z * dist;
}

/// `flipCoord3D`.
fn flip_coord3d(coord: &mut Coord3D) {
    coord.x = -coord.x;
    coord.y = -coord.y;
    coord.z = -coord.z;
}

/// `testRotatedPointsAgainstRect`.
fn test_rotated_points_against_rect(
    pts: &[Coord2D; 4],
    a: &CollideInfo,
    avg: &mut Coord2D,
    avg_tot: &mut i32,
) {
    let major = a.geom.get_major_radius();
    let minor = if a.geom.get_geom_type() == GeometryType::Sphere {
        a.geom.get_major_radius()
    } else {
        a.geom.get_minor_radius()
    };

    let c = trig::cos(-a.angle);
    let s = trig::sin(-a.angle);

    for pt in pts {
        // convert to a delta relative to rect ctr
        let ptx = pt.x - a.position.x;
        let pty = pt.y - a.position.y;

        // inverse-rotate it to the right coord system
        let ptx_new = (ptx * c - pty * s).abs();
        let pty_new = (ptx * s + pty * c).abs();

        if ptx_new <= major && pty_new <= minor {
            avg.x += pt.x;
            avg.y += pt.y;
            *avg_tot += 1;
        }
    }
}

/// `rectToFourPoints` (z is ignored).
fn rect_to_four_points(a: &CollideInfo) -> [Coord2D; 4] {
    let c = trig::cos(a.angle);
    let s = trig::sin(a.angle);

    let exc = a.geom.get_major_radius() * c;
    let eyc = a.geom.get_minor_radius() * c;
    let exs = a.geom.get_major_radius() * s;
    let eys = a.geom.get_minor_radius() * s;

    [
        // tl
        Coord2D {
            x: a.position.x - exc - eys,
            y: a.position.y + eyc - exs,
        },
        // tr
        Coord2D {
            x: a.position.x + exc - eys,
            y: a.position.y + eyc + exs,
        },
        // bl
        Coord2D {
            x: a.position.x - exc + eys,
            y: a.position.y - eyc - exs,
        },
        // br
        Coord2D {
            x: a.position.x + exc + eys,
            y: a.position.y - eyc + exs,
        },
    ]
}

/// `xy_collideTest_Circle_Rect`: 2d routine for rect/cyl and rect/sphere.
fn xy_collide_test_circle_rect(
    a: &CollideInfo,
    b: &CollideInfo,
    mut cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    let result = xy_collide_test_rect_circle(b, a, cinfo.as_deref_mut());
    if let Some(ci) = cinfo {
        flip_coord3d(&mut ci.normal);
    }
    result
}

/// `xy_collideTest_Rect_Circle`: the circle is tested as its bounding square
/// (upstream's active `#if 1` branch; the other is dead text).
fn xy_collide_test_rect_circle(
    a: &CollideInfo,
    b: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    let mut btmp = b.clone();
    let major = btmp.geom.get_major_radius();
    btmp.geom.set_minor_radius(major);
    xy_collide_test_rect_rect(a, &btmp, cinfo)
}

/// `xy_collideTest_Circle_Circle`.
fn xy_collide_test_circle_circle(
    a: &CollideInfo,
    b: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    let diff = vec_diff_2d(&b.position, &a.position);
    let dist_sqr = calc_sqr_dist_2d(&diff);
    let r = a.geom.get_major_radius() + b.geom.get_major_radius();
    let touching_dist_sqr = r * r;
    if dist_sqr <= touching_dist_sqr {
        // calc the approximate location and normal.
        if let Some(ci) = cinfo {
            ci.normal = diff;
            ci.normal.normalize();
            ci.loc = a.position;
            let n = ci.normal;
            project_coord3d(&mut ci.loc, &n, a.geom.get_major_radius());
        }
        return true;
    }
    false
}

/// `xy_collideTest_Rect_Rect`.
fn xy_collide_test_rect_rect(
    a: &CollideInfo,
    b: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    let mut avg = Coord2D { x: 0.0, y: 0.0 };
    let mut avg_tot: i32 = 0;

    let pts = rect_to_four_points(a);
    test_rotated_points_against_rect(&pts, b, &mut avg, &mut avg_tot);

    let pts = rect_to_four_points(b);
    test_rotated_points_against_rect(&pts, a, &mut avg, &mut avg_tot);

    if avg_tot > 0 {
        if let Some(ci) = cinfo {
            // this is a little hokey, but generally adequate...
            ci.loc.x = avg.x / avg_tot as f32;
            ci.loc.y = avg.y / avg_tot as f32;
            ci.loc.z = (a.position.z + b.position.z) * 0.5;

            // the center-to-center normal (see upstream's long comment).
            ci.normal = vec_diff_2d(&b.position, &a.position);
            ci.normal.normalize();
        }
        return true;
    }
    false
}

/// `z_collideTest_Sphere_Nonsphere`.
fn z_collide_test_sphere_nonsphere(
    xyproc: XyProc,
    a: &CollideInfo,
    b: &CollideInfo,
    mut cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    // case 1: center of sphere is within the nonsphere z-space.
    if a.position.z >= b.position.z
        && a.position.z <= b.position.z + b.geom.get_max_height_above_position()
    {
        if xy_dispatch(xyproc, a, b, cinfo.as_deref_mut()) {
            // a side-to-side collision; just need to adjust the z-coord of collideLoc
            if let Some(ci) = cinfo {
                ci.loc.z = a.position.z;
            }
            return true;
        }
        return false;
    }

    // case 2: center of sphere is above or below nonsphere, but within
    // major-radius of top or bottom.

    // 2a: sphere is below the nonsphere
    let b_bot = b.position.z;
    if a.position.z < b_bot && a.position.z + a.geom.get_major_radius() >= b_bot {
        // find the radius of the slice of the sphere that is at b_bot
        let mut amod = a.clone();
        amod.position.z = b_bot;
        let dz = b_bot - a.position.z;
        amod.geom.set_major_radius(Real::sqrt(
            a.geom.get_major_radius() * a.geom.get_major_radius() - dz * dz,
        ));
        return xy_dispatch(xyproc, &amod, b, cinfo);
    }

    // 2b: sphere is above the nonsphere
    let b_top = b.position.z + b.geom.get_max_height_above_position();
    if a.position.z > b_top && a.position.z - a.geom.get_major_radius() <= b_top {
        let mut amod = a.clone();
        amod.position.z = b_top;
        let dz = a.position.z - b_top;
        amod.geom.set_major_radius(Real::sqrt(
            a.geom.get_major_radius() * a.geom.get_major_radius() - dz * dz,
        ));
        return xy_dispatch(xyproc, &amod, b, cinfo);
    }

    false
}

/// `z_collideTest_Nonsphere_Nonsphere`.
fn z_collide_test_nonsphere_nonsphere(
    xyproc: XyProc,
    a: &CollideInfo,
    b: &CollideInfo,
    mut cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    // note: we already know that there is a z-intersection (our caller filters that out)

    // Preflight for close objects.
    let dx = a.position.x - b.position.x;
    let dy = a.position.y - b.position.y;
    let d_sqr = dx * dx + dy * dy;
    let mut min_radius = a.geom.get_major_radius();
    let mut r = a.geom.get_minor_radius();
    if min_radius > r {
        min_radius = r;
    }
    r = b.geom.get_major_radius();
    if min_radius > r {
        min_radius = r;
    }
    r = b.geom.get_minor_radius();
    if min_radius > r {
        min_radius = r;
    }

    let close_enough = min_radius * min_radius > d_sqr;

    if close_enough || xy_dispatch(xyproc, a, b, cinfo.as_deref_mut()) {
        // case 1: side-to-side collision (see upstream's comment)
        if let Some(ci) = cinfo {
            if b.position.z > a.position.z {
                ci.loc.z =
                    (b.position.z + a.position.z + a.geom.get_max_height_above_position()) * 0.5;
            } else {
                ci.loc.z =
                    (a.position.z + b.position.z + b.geom.get_max_height_above_position()) * 0.5;
            }
        }
        return true;
    }

    false
}

fn collide_test_sphere_sphere(
    a: &CollideInfo,
    b: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    let diff = vec_diff_3d(&b.position, &a.position);
    let dist_sqr = calc_sqr_dist_3d(&diff);
    let r = a.geom.get_major_radius() + b.geom.get_major_radius();
    let touching_dist_sqr = r * r;
    if dist_sqr <= touching_dist_sqr {
        if let Some(ci) = cinfo {
            ci.normal = diff;
            ci.normal.normalize();
            ci.loc = a.position;
            let n = ci.normal;
            project_coord3d(&mut ci.loc, &n, a.geom.get_major_radius());
        }
        return true;
    }
    false
}

/// The nine tests of `theCollideTestProcs`, selected by the two shapes (the
/// table index `(a - GEOMETRY_FIRST) * GEOMETRY_NUM_TYPES + (b - GEOMETRY_FIRST)`).
pub fn collide_test(
    a: &CollideInfo,
    b: &CollideInfo,
    mut cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    use GeometryType::{Box, Cylinder, Sphere};
    match (a.geom.get_geom_type(), b.geom.get_geom_type()) {
        (Sphere, Sphere) => collide_test_sphere_sphere(a, b, cinfo),
        (Sphere, Cylinder) => z_collide_test_sphere_nonsphere(XyProc::CircleCircle, a, b, cinfo),
        (Sphere, Box) => z_collide_test_sphere_nonsphere(XyProc::CircleRect, a, b, cinfo),
        (Cylinder, Sphere) => {
            let result =
                z_collide_test_sphere_nonsphere(XyProc::CircleCircle, b, a, cinfo.as_deref_mut());
            if let Some(ci) = cinfo {
                flip_coord3d(&mut ci.normal);
            }
            result
        }
        (Cylinder, Cylinder) => {
            z_collide_test_nonsphere_nonsphere(XyProc::CircleCircle, a, b, cinfo)
        }
        (Cylinder, Box) => z_collide_test_nonsphere_nonsphere(XyProc::CircleRect, a, b, cinfo),
        (Box, Sphere) => {
            let result =
                z_collide_test_sphere_nonsphere(XyProc::CircleRect, b, a, cinfo.as_deref_mut());
            if let Some(ci) = cinfo {
                flip_coord3d(&mut ci.normal);
            }
            result
        }
        (Box, Cylinder) => z_collide_test_nonsphere_nonsphere(XyProc::RectCircle, a, b, cinfo),
        (Box, Box) => z_collide_test_nonsphere_nonsphere(XyProc::RectRect, a, b, cinfo),
    }
}

/// `PartitionData::collidesWith`'s geometry, after the `KINDOF_NO_COLLIDE`
/// test: the z-overlap check (with `getMaxHeightBelowPosition`) and then the
/// shape test. `this`/`that` are the two objects' position, geometry and
/// orientation.
pub fn collides_with(
    this: &CollideInfo,
    that: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    // invariant for all geometries: first do z collision check.
    let this_top = this.position.z + this.geom.get_max_height_above_position();
    let this_bot = this.position.z - this.geom.get_max_height_below_position();
    let that_top = that.position.z + that.geom.get_max_height_above_position();
    let that_bot = that.position.z - that.geom.get_max_height_below_position();
    if this_top >= that_bot && this_bot <= that_top {
        collide_test(this, that, cinfo)
    } else {
        // no z-intersection -> no collision.
        false
    }
}

/// `PartitionManager::geomCollidesWithGeom` (and the body of
/// `PartitionFilterWouldCollide::allow`, which repeats it): note its z test
/// differs from [`collides_with`] (no "below" extent).
pub fn geom_collides_with_geom(
    pos1: &Coord3D,
    geom1: &GeometryInfo,
    angle1: f32,
    pos2: &Coord3D,
    geom2: &GeometryInfo,
    angle2: f32,
) -> bool {
    let this_info = CollideInfo::new(pos1, geom1, angle1);
    let that_info = CollideInfo::new(pos2, geom2, angle2);

    // invariant for all geometries: first do z collision check.
    if this_info.position.z + this_info.geom.get_max_height_above_position() >= that_info.position.z
        && this_info.position.z
            <= that_info.position.z + that_info.geom.get_max_height_above_position()
    {
        let mut cloc = CollideLocAndNormal::default();
        collide_test(&this_info, &that_info, Some(&mut cloc))
    } else {
        // no z-intersection -> no collision.
        false
    }
}

/// The result of a distance procedure: whether the squared distance is under
/// the limit, the squared distance and the vector `B - A` (shrunk for the
/// boundary kinds). The last two are set whether or not the first is true.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistResult {
    /// The return value: `abDistSqr < maxDistSqr`.
    pub within: bool,
    /// `abDistSqr`.
    pub dist_sqr: f32,
    /// `abVec`.
    pub vec: Coord3D,
}

/// `theDistCalcProcs[dc](posA, objA, posB, objB, ..., maxDistSqr)`. `geom_a` and
/// `geom_b` are the geometries of `objA`/`objB`, `None` for a bare position.
pub fn dist_calc(
    dc: DistanceCalculationType,
    pos_a: &Coord3D,
    geom_a: Option<&GeometryInfo>,
    pos_b: &Coord3D,
    geom_b: Option<&GeometryInfo>,
    max_dist_sqr: f32,
) -> DistResult {
    match dc {
        DistanceCalculationType::FromCenter2D => {
            let diff = Coord3D {
                x: pos_b.x - pos_a.x,
                y: pos_b.y - pos_a.y,
                z: 0.0,
            };
            let d = diff.x * diff.x + diff.y * diff.y;
            DistResult {
                within: d < max_dist_sqr,
                dist_sqr: d,
                vec: diff,
            }
        }
        DistanceCalculationType::FromCenter3D => {
            let diff = Coord3D {
                x: pos_b.x - pos_a.x,
                y: pos_b.y - pos_a.y,
                z: pos_b.z - pos_a.z,
            };
            let d = diff.x * diff.x + diff.y * diff.y + diff.z * diff.z;
            DistResult {
                within: d < max_dist_sqr,
                dist_sqr: d,
                vec: diff,
            }
        }
        DistanceCalculationType::FromBoundingSphere2D => {
            let mut diff = Coord3D {
                x: pos_b.x - pos_a.x,
                y: pos_b.y - pos_a.y,
                z: 0.0,
            };
            let actual_dist_sqr = diff.x * diff.x + diff.y * diff.y;

            let mut shrink_factor = 1.0f32;
            let mut shrunken_dist_sqr = actual_dist_sqr;
            let total_rad = geom_a.map_or(0.0, |g| g.get_bounding_circle_radius())
                + geom_b.map_or(0.0, |g| g.get_bounding_circle_radius());

            if total_rad > 0.0 {
                let actual_dist = Real::sqrt(actual_dist_sqr);
                let shrunken_dist = actual_dist - total_rad;
                if shrunken_dist <= 0.0 {
                    shrink_factor = 0.0;
                    shrunken_dist_sqr = 0.0; // sorry, distances can't be negative
                } else {
                    shrink_factor = shrunken_dist / actual_dist;
                    shrunken_dist_sqr = shrunken_dist * shrunken_dist;
                }
            }

            diff.x *= shrink_factor;
            diff.y *= shrink_factor;
            DistResult {
                within: shrunken_dist_sqr < max_dist_sqr,
                dist_sqr: shrunken_dist_sqr,
                vec: diff,
            }
        }
        DistanceCalculationType::FromBoundingSphere3D => {
            let mut diff = Coord3D {
                x: pos_b.x - pos_a.x,
                y: pos_b.y - pos_a.y,
                z: (pos_b.z + geom_b.map_or(0.0, |g| g.get_z_delta_to_center_position()))
                    - (pos_a.z + geom_a.map_or(0.0, |g| g.get_z_delta_to_center_position())),
            };

            let actual_dist_sqr = diff.x * diff.x + diff.y * diff.y + diff.z * diff.z;

            let mut shrink_factor = 1.0f32;
            let mut shrunken_dist_sqr = actual_dist_sqr;
            let total_rad = geom_a.map_or(0.0, |g| g.get_bounding_sphere_radius())
                + geom_b.map_or(0.0, |g| g.get_bounding_sphere_radius());
            if total_rad > 0.0 {
                let actual_dist = Real::sqrt(actual_dist_sqr);
                let shrunken_dist = actual_dist - total_rad;
                if shrunken_dist <= 0.0 {
                    shrink_factor = 0.0;
                    shrunken_dist_sqr = 0.0;
                } else {
                    shrink_factor = shrunken_dist / actual_dist;
                    shrunken_dist_sqr = shrunken_dist * shrunken_dist;
                }
            }

            diff.x *= shrink_factor;
            diff.y *= shrink_factor;
            diff.z *= shrink_factor;
            DistResult {
                within: shrunken_dist_sqr < max_dist_sqr,
                dist_sqr: shrunken_dist_sqr,
                vec: diff,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(x: f32, y: f32, z: f32, g: GeometryInfo, angle: f32) -> CollideInfo {
        CollideInfo::new(&Coord3D { x, y, z }, &g, angle)
    }

    fn sphere(r: f32) -> GeometryInfo {
        GeometryInfo::new(GeometryType::Sphere, false, r, r, r)
    }

    #[test]
    fn two_spheres_touch_at_the_sum_of_radii_and_report_the_surface_point() {
        let a = info(0.0, 0.0, 0.0, sphere(2.0), 0.0);
        let b = info(4.0, 0.0, 0.0, sphere(2.0), 0.0);
        let mut ci = CollideLocAndNormal::default();
        assert!(collide_test(&a, &b, Some(&mut ci)));
        assert_eq!(
            ci.normal,
            Coord3D {
                x: 1.0,
                y: 0.0,
                z: 0.0
            }
        );
        assert_eq!(
            ci.loc,
            Coord3D {
                x: 2.0,
                y: 0.0,
                z: 0.0
            }
        );
        let b = info(4.01, 0.0, 0.0, sphere(2.0), 0.0);
        assert!(!collide_test(&a, &b, None));
    }

    #[test]
    fn a_cylinder_sphere_test_is_the_flipped_sphere_cylinder_test() {
        let cyl = GeometryInfo::new(GeometryType::Cylinder, false, 10.0, 3.0, 3.0);
        let a = info(0.0, 0.0, 0.0, cyl, 0.0);
        let b = info(4.0, 0.0, 5.0, sphere(2.0), 0.0);
        let mut ab = CollideLocAndNormal::default();
        let mut ba = CollideLocAndNormal::default();
        assert!(collide_test(&a, &b, Some(&mut ab)));
        assert!(collide_test(&b, &a, Some(&mut ba)));
        assert_eq!(ab.normal.x, -ba.normal.x);
        assert_eq!(ab.normal.y, -ba.normal.y);
    }

    #[test]
    fn boxes_collide_through_rotation_and_miss_when_apart() {
        let bx = GeometryInfo::new(GeometryType::Box, false, 4.0, 5.0, 1.0);
        let a = info(0.0, 0.0, 0.0, bx.clone(), 0.0);
        let b = info(0.0, 4.0, 0.0, bx.clone(), 0.0);
        assert!(!collide_test(&a, &b, None), "1 + 1 < 4 in y");
        let b = info(0.0, 4.0, 0.0, bx, core::f32::consts::FRAC_PI_2);
        assert!(
            collide_test(&a, &b, None),
            "rotated 90 degrees its long side reaches"
        );
    }

    #[test]
    fn collides_with_uses_the_below_extent_and_geom_collides_does_not() {
        // a sphere centred at z=0 reaches 2 below and 2 above; a box standing at z = -3, 2 high, tops out at -1
        let s = info(0.0, 0.0, 0.0, sphere(2.0), 0.0);
        let bx = info(
            0.0,
            0.0,
            -3.0,
            GeometryInfo::new(GeometryType::Box, false, 2.0, 1.0, 1.0),
            0.0,
        );
        // geomCollidesWithGeom: this.z (0) <= that top (-1) is false
        assert!(!geom_collides_with_geom(
            &s.position,
            &s.geom,
            0.0,
            &bx.position,
            &bx.geom,
            0.0
        ));
        // collidesWith: this bottom (-2) <= that top (-1), then the sphere/box test (case 2b) finds the slice
        assert!(collides_with(&s, &bx, None));
    }

    #[test]
    fn boundary_distance_shrinks_by_the_radii_and_never_goes_negative() {
        let g = sphere(2.0);
        let a = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = Coord3D {
            x: 10.0,
            y: 0.0,
            z: 0.0,
        };
        let r = dist_calc(
            DistanceCalculationType::FromBoundingSphere2D,
            &a,
            Some(&g),
            &b,
            Some(&g),
            HUGE_DIST_SQR,
        );
        assert_eq!(r.dist_sqr, 36.0);
        assert_eq!(
            r.vec,
            Coord3D {
                x: 6.0,
                y: 0.0,
                z: 0.0
            }
        );
        let near = Coord3D {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
        let r = dist_calc(
            DistanceCalculationType::FromBoundingSphere2D,
            &a,
            Some(&g),
            &near,
            Some(&g),
            HUGE_DIST_SQR,
        );
        assert_eq!(r.dist_sqr, 0.0);
        let c = dist_calc(
            DistanceCalculationType::FromCenter3D,
            &a,
            None,
            &b,
            None,
            50.0,
        );
        assert!(!c.within);
        assert_eq!(c.dist_sqr, 100.0);
    }

    #[test]
    fn nan_distance_is_never_within() {
        let a = Coord3D {
            x: f32::NAN,
            y: 0.0,
            z: 0.0,
        };
        let b = Coord3D {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
        let r = dist_calc(
            DistanceCalculationType::FromCenter2D,
            &a,
            None,
            &b,
            None,
            HUGE_DIST_SQR,
        );
        assert!(!r.within && r.dist_sqr.is_nan());
    }
}
