// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/PartitionManager.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/PartitionManager.cpp
//   (`getClosestObjects` and the queries built on it, the distance
//   functions, the cell walks, line of sight, `findPositionAround`)
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

//! The queries of the partition manager: the closest object, every object in
//! range (with its sort order), every object, potential collisions, distances
//! and vectors between objects, the relative angle, cell walks, terrain
//! extremes and line of sight along a line, and the search for a legal
//! position around a point.
//!
//! # The search, and why its order is simulation state
//!
//! `getClosestObjects` visits the cells in rings of increasing radius around
//! the query cell (`m_radiusVec`), each ring in the table's order, and the
//! intersections of each cell in that cell's list order (newest first). A
//! nearest-object query keeps the first object at the smallest squared
//! distance (`<` against the running best), so ties are decided by that visit
//! order. A range query appends each hit to a [`SimpleObjectIterator`], which
//! prepends: the unsorted result is the *reverse* visit order, and the sort
//! (a stable-in-structure merge sort, see `logic::object_iter`) keeps equal
//! keys in the order its tie rule gives. Both are reproduced exactly; the
//! differential test compares result lists in order.
//!
//! # Where upstream is undefined
//!
//! - `iterateCellsBreadthFirst` tests `curY + 1 > 0`, which is always true,
//!   where it means `curY + 1 < m_cellCountY`: on the last row it reads and
//!   pushes cells beyond the array. Here the neighbour below the last row is
//!   skipped, and the walk reports that it got there
//!   ([`PartitionManager::iterate_cells_breadth_first`]'s second result), so a
//!   comparison can leave such walks out. A start cell off the map is the same
//!   case (upstream indexes outside the bit field).
//! - A query before the grid exists finds nothing (upstream indexes an empty vector).

use super::collide::{self, DistanceCalculationType, HUGE_DIST, HUGE_DIST_SQR};
use super::filters::{filters_allow, is_unit, PartitionFilter};
use super::host::{PartitionFilterHost, PartitionHost, LAYER_GROUND, PathfindCellType};
use super::manager::PartitionManager;
use super::cell::PartitionCell;
use crate::common::base_type::{real_to_int_ceil, real_to_int_floor, Coord2D, Coord3D, TWO_PI};
use crate::common::geometry::{GeometryInfo, GeometryType};
use crate::common::kind_of::{KINDOF_STRUCTURE};
use crate::common::trig;
use crate::logic::object_iter::{IterOrderType, SimpleObjectIterator};
use crate::logic::world::ids::ObjectId;
use petir::Real;
use std::collections::VecDeque;

/// `FindPositionFlags`.
pub mod fpf {
    /// `FPF_NONE`.
    pub const NONE: u32 = 0x0000_0000;
    /// `FPF_IGNORE_WATER`: a position found underwater is ok.
    pub const IGNORE_WATER: u32 = 0x0000_0001;
    /// `FPF_WATER_ONLY`: find positions in the water only.
    pub const WATER_ONLY: u32 = 0x0000_0002;
    /// `FPF_IGNORE_ALL_OBJECTS`: positions inside objects are ok.
    pub const IGNORE_ALL_OBJECTS: u32 = 0x0000_0004;
    /// `FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS` (requires a relationship object).
    pub const IGNORE_ALLY_OR_NEUTRAL_UNITS: u32 = 0x0000_0008;
    /// `FPF_IGNORE_ALLY_OR_NEUTRAL_STRUCTURES`.
    pub const IGNORE_ALLY_OR_NEUTRAL_STRUCTURES: u32 = 0x0000_0010;
    /// `FPF_IGNORE_ENEMY_UNITS`.
    pub const IGNORE_ENEMY_UNITS: u32 = 0x0000_0020;
    /// `FPF_IGNORE_ENEMY_STRUCTURES`.
    pub const IGNORE_ENEMY_STRUCTURES: u32 = 0x0000_0040;
    /// `FPF_USE_HIGHEST_LAYER`: examine the position on the highest layer at the given xy.
    pub const USE_HIGHEST_LAYER: u32 = 0x0000_0080;
    /// `FPF_CLEAR_CELLS_ONLY`: reject anything that is not `PathFindCell::Clear`.
    pub const CLEAR_CELLS_ONLY: u32 = 0x0000_0100;
}

/// `RANDOM_START_ANGLE`: no start angle (an unlikely number to use for one).
pub const RANDOM_START_ANGLE: f32 = -99999.9;

/// `ringSpacing`: how fast the concentric ring search of `findPositionAround` expands.
const RING_SPACING: f32 = 5.0;

/// `PATHFIND_CELL_SIZE` (`AIPathfind.h`).
const PATHFIND_CELL_SIZE: f32 = 10.0;

/// `struct FindPositionOptions`.
#[derive(Clone, Debug, PartialEq)]
pub struct FindPositionOptions {
    /// `flags`: `FPF_*` bits.
    pub flags: u32,
    /// `minRadius`.
    pub min_radius: f32,
    /// `maxRadius`.
    pub max_radius: f32,
    /// `startAngle`: [`RANDOM_START_ANGLE`] for a random one.
    pub start_angle: f32,
    /// `maxZDelta`: the maximum delta-z allowed.
    pub max_z_delta: f32,
    /// `ignoreObject`: ignore this object in legal position checks.
    pub ignore_object: Option<ObjectId>,
    /// `sourceToPathToDest`: an object that must be able to path to the position chosen.
    pub source_to_path_to_dest: Option<ObjectId>,
    /// `relationshipObject`: the object used for relationship tests.
    pub relationship_object: Option<ObjectId>,
}

impl Default for FindPositionOptions {
    /// `FindPositionOptions::FindPositionOptions()`.
    fn default() -> Self {
        FindPositionOptions {
            flags: fpf::NONE,
            min_radius: 0.0,
            max_radius: 0.0,
            start_angle: RANDOM_START_ANGLE,
            max_z_delta: 1e10,
            ignore_object: None,
            source_to_path_to_dest: None,
            relationship_object: None,
        }
    }
}

/// What `getClosestObject` returns through its out parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestObject {
    /// The return value: the closest object, `None` (upstream `NULL`) if none passed.
    pub object: Option<ObjectId>,
    /// `*closestDist`: `sqrtf(closestDistSqr)`, which is `maxDist` itself when nothing was found.
    pub dist: f32,
    /// `*closestDistVec`: the vector to the closest object (shrunk for the boundary kinds). Zero when
    /// nothing was found (upstream leaves it uninitialised).
    pub vec: Coord3D,
}

impl PartitionManager {
    // ---- the closest object, and ranges ---------------------------------------

    /// `getClosestObjects(obj, pos, maxDist, dc, filters, iter, closestDist, closestVec)`
    /// (protected upstream): exactly one of `obj` and `pos` is the origin. With
    /// `iter`, every satisfactory object is appended to it (not just the closest).
    #[allow(clippy::too_many_arguments)] // the signature of upstream's `getClosestObjects`
    pub fn get_closest_objects<H: PartitionFilterHost>(
        &mut self,
        host: &H,
        obj: Option<ObjectId>,
        pos: Option<&Coord3D>,
        max_dist: f32,
        dc: DistanceCalculationType,
        filters: &[PartitionFilter],
        mut iter: Option<&mut SimpleObjectIterator<ObjectId>>,
    ) -> ClosestObject {
        // the origin: a position, or an object's position (and shape, for the boundary kinds)
        let obj_geometry = obj.and_then(|o| host.object_geometry(o));
        let obj_pos: Coord3D = match (pos, &obj_geometry) {
            (Some(p), _) => *p,
            (None, Some(g)) => g.position,
            (None, None) => return ClosestObject { object: None, dist: Real::sqrt(max_dist * max_dist), vec: Coord3D::default() },
        };
        let obj_to_use_geom: Option<&GeometryInfo> = if pos.is_some() { None } else { obj_geometry.as_ref().map(|g| &g.geometry) };
        let (cell_center_x, cell_center_y) = self.world_to_cell(obj_pos.x, obj_pos.y);

        let mut closest_obj: Option<ObjectId> = None;
        let mut closest_dist_sqr = max_dist * max_dist; // if it's not closer than this, we shouldn't consider it anyway...
        let mut closest_vec = Coord3D::default();

        let mut max_radius = self.max_gco_radius;
        if max_dist < HUGE_DIST {
            // don't go outwards any farther than necessary.
            max_radius = max_radius.min(self.world_to_cell_dist(max_dist));
        }
        let mut max_radius_limit = max_radius;

        let mut found_any = false;

        self.iter_flag = self.iter_flag.wrapping_add(1);
        let iter_flag = self.iter_flag;

        // m_radiusVec[curRadius] contains a list of the cells that could contain objects that are
        // <= (curRadius * cellSize) distance away from cell (0,0).
        let mut cur_radius: i32 = 0;
        while cur_radius <= max_radius_limit {
            let n_offsets = match usize::try_from(cur_radius).ok().and_then(|r| self.radius_vec.get(r)) {
                Some(v) => v.len(),
                None => {
                    cur_radius += 1;
                    continue;
                }
            };
            for k in 0..n_offsets {
                let off = self.radius_vec[cur_radius as usize][k];
                let Some(this_cell) = self.cell_index(cell_center_x.wrapping_add(off.x), cell_center_y.wrapping_add(off.y)) else {
                    continue;
                };

                let mut this_coi = self.cells[this_cell].first_coi;
                while let Some(r) = this_coi {
                    let this_mod = self.coi(r).module.map(|m| m as usize);
                    this_coi = self.coi(r).next;
                    let Some(this_mod) = this_mod else {
                        continue;
                    };
                    let this_obj = self.md(this_mod).object;

                    // never compare against ourself.
                    let Some(this_obj) = this_obj else {
                        continue;
                    };
                    if Some(this_obj) == obj {
                        continue;
                    }

                    // since an object can exist in multiple COIs, we use this to avoid processing
                    // the same one more than once.
                    if self.md(this_mod).done_flag == iter_flag {
                        continue;
                    }
                    self.md_mut(this_mod).done_flag = iter_flag;

                    let Some(this_g) = host.object_geometry(this_obj) else {
                        continue;
                    };
                    let d = collide::dist_calc(dc, &obj_pos, obj_to_use_geom, &this_g.position, Some(&this_g.geometry), closest_dist_sqr);
                    if !d.within {
                        continue;
                    }

                    if !filters_allow(self, host, filters, this_obj) {
                        continue;
                    }

                    // ok, this is within the range, and the filters allow it.
                    // add it to the iter, if we have one....
                    if let Some(it) = iter.as_deref_mut() {
                        let _ = it.insert(Some(this_obj), d.dist_sqr);
                    } else {
                        // hey, this is the new closest object! cool.
                        // (note that we can't break out now 'cuz we have to finish examining the
                        // rest of curRadius)
                        closest_obj = Some(this_obj);
                        closest_dist_sqr = d.dist_sqr;
                        closest_vec = d.vec;

                        if !found_any {
                            // if not adding to iterArg, we want to stop once we have the closest object.
                            max_radius_limit = cur_radius;
                        }
                        found_any = true;
                    }
                }
            }
            cur_radius += 1;
        }

        ClosestObject { object: closest_obj, dist: Real::sqrt(closest_dist_sqr), vec: closest_vec }
    }

    /// `getClosestObject(obj, maxDist, dc, filters, &closestDist, &closestDistVec)`.
    pub fn get_closest_object<H: PartitionFilterHost>(&mut self, host: &H, obj: ObjectId, max_dist: f32, dc: DistanceCalculationType, filters: &[PartitionFilter]) -> ClosestObject {
        self.get_closest_objects(host, Some(obj), None, max_dist, dc, filters, None)
    }

    /// `getClosestObject(pos, maxDist, dc, filters, &closestDist, &closestDistVec)`.
    pub fn get_closest_object_at<H: PartitionFilterHost>(&mut self, host: &H, pos: &Coord3D, max_dist: f32, dc: DistanceCalculationType, filters: &[PartitionFilter]) -> ClosestObject {
        self.get_closest_objects(host, None, Some(pos), max_dist, dc, filters, None)
    }

    /// `iterateObjectsInRange(obj, maxDist, dc, filters, order)`.
    pub fn iterate_objects_in_range<H: PartitionFilterHost>(
        &mut self,
        host: &H,
        obj: ObjectId,
        max_dist: f32,
        dc: DistanceCalculationType,
        filters: &[PartitionFilter],
        order: IterOrderType,
    ) -> SimpleObjectIterator<ObjectId> {
        let mut iter = SimpleObjectIterator::new();
        self.get_closest_objects(host, Some(obj), None, max_dist, dc, filters, Some(&mut iter));
        iter.sort(order, |o| host.build_cost(*o));
        iter
    }

    /// `iterateObjectsInRange(pos, maxDist, dc, filters, order)`.
    pub fn iterate_objects_in_range_at<H: PartitionFilterHost>(
        &mut self,
        host: &H,
        pos: &Coord3D,
        max_dist: f32,
        dc: DistanceCalculationType,
        filters: &[PartitionFilter],
        order: IterOrderType,
    ) -> SimpleObjectIterator<ObjectId> {
        let mut iter = SimpleObjectIterator::new();
        self.get_closest_objects(host, None, Some(pos), max_dist, dc, filters, Some(&mut iter));
        iter.sort(order, |o| host.build_cost(*o));
        iter
    }

    /// `iteratePotentialCollisions(pos, geom, angle, use2D)`: the objects that
    /// would collide with the given geometry (a little slop is added to the range).
    pub fn iterate_potential_collisions<H: PartitionFilterHost>(&mut self, host: &H, pos: &Coord3D, geom: &GeometryInfo, angle: f32, use_2d: bool) -> SimpleObjectIterator<ObjectId> {
        let mut max_dist = geom.get_bounding_sphere_radius();
        max_dist *= 1.1; // just a little slop

        let mut iter = SimpleObjectIterator::new();

        let filters = [PartitionFilter::WouldCollide { position: *pos, geom: geom.clone(), angle, desired: true }];

        let dc = if use_2d { DistanceCalculationType::FromBoundingSphere2D } else { DistanceCalculationType::FromBoundingSphere3D };
        self.get_closest_objects(host, None, Some(pos), max_dist, dc, &filters, Some(&mut iter));

        iter
    }

    /// `iterateAllObjects(filters)`: every registered object that passes, in
    /// module-list order (newest registration first), with numeric key 0.
    pub fn iterate_all_objects<H: PartitionFilterHost>(&mut self, host: &H, filters: &[PartitionFilter]) -> SimpleObjectIterator<ObjectId> {
        let mut iter = SimpleObjectIterator::new();

        // no distance constraints; we're gonna have to process 'em all anyway,
        // so go thru in the fastest way we know how.
        let mut m = self.module_list;
        while let Some(slot) = m {
            let next_mod = self.md(slot).next;
            if let Some(obj) = self.md(slot).object {
                if filters_allow(self, host, filters, obj) {
                    let _ = iter.insert(Some(obj), 0.0);
                }
            }
            m = next_mod;
        }

        iter
    }

    // ---- distances and vectors --------------------------------------------------

    /// The origin/target data for the distance functions: position and shape of an object.
    fn geom_of<H: PartitionHost>(host: &H, id: ObjectId) -> Option<super::host::ObjectGeometry> {
        host.object_geometry(id)
    }

    /// `getVectorTo(obj, otherObj, dc, vec)`: the vector from `obj` to `other_obj` (`None` if either is gone).
    pub fn get_vector_to<H: PartitionHost>(&self, host: &H, obj: ObjectId, other_obj: ObjectId, dc: DistanceCalculationType) -> Option<Coord3D> {
        let (a, b) = (Self::geom_of(host, obj)?, Self::geom_of(host, other_obj)?);
        Some(collide::dist_calc(dc, &a.position, Some(&a.geometry), &b.position, Some(&b.geometry), HUGE_DIST_SQR).vec)
    }

    /// `getVectorTo(obj, pos, dc, vec)`.
    pub fn get_vector_to_pos<H: PartitionHost>(&self, host: &H, obj: ObjectId, pos: &Coord3D, dc: DistanceCalculationType) -> Option<Coord3D> {
        let a = Self::geom_of(host, obj)?;
        Some(collide::dist_calc(dc, &a.position, Some(&a.geometry), pos, None, HUGE_DIST_SQR).vec)
    }

    /// `getDistanceSquared(obj, otherObj, dc, &vec)`: the squared distance and the vector.
    pub fn get_distance_squared<H: PartitionHost>(&self, host: &H, obj: ObjectId, other_obj: ObjectId, dc: DistanceCalculationType) -> Option<(f32, Coord3D)> {
        let (a, b) = (Self::geom_of(host, obj)?, Self::geom_of(host, other_obj)?);
        let r = collide::dist_calc(dc, &a.position, Some(&a.geometry), &b.position, Some(&b.geometry), HUGE_DIST_SQR);
        Some((r.dist_sqr, r.vec))
    }

    /// `getDistanceSquared(obj, pos, dc, &vec)`.
    pub fn get_distance_squared_pos<H: PartitionHost>(&self, host: &H, obj: ObjectId, pos: &Coord3D, dc: DistanceCalculationType) -> Option<(f32, Coord3D)> {
        let a = Self::geom_of(host, obj)?;
        let r = collide::dist_calc(dc, &a.position, Some(&a.geometry), pos, None, HUGE_DIST_SQR);
        Some((r.dist_sqr, r.vec))
    }

    /// `getGoalDistanceSquared(obj, goalPos, otherObj, dc, &vec)`: the distance if `obj` were at `goal_pos`.
    pub fn get_goal_distance_squared<H: PartitionHost>(&self, host: &H, obj: ObjectId, goal_pos: &Coord3D, other_obj: ObjectId, dc: DistanceCalculationType) -> Option<(f32, Coord3D)> {
        let (a, b) = (Self::geom_of(host, obj)?, Self::geom_of(host, other_obj)?);
        let r = collide::dist_calc(dc, goal_pos, Some(&a.geometry), &b.position, Some(&b.geometry), HUGE_DIST_SQR);
        Some((r.dist_sqr, r.vec))
    }

    /// `getGoalDistanceSquared(obj, goalPos, otherPos, dc, &vec)`.
    pub fn get_goal_distance_squared_pos<H: PartitionHost>(&self, host: &H, obj: ObjectId, goal_pos: &Coord3D, other_pos: &Coord3D, dc: DistanceCalculationType) -> Option<(f32, Coord3D)> {
        let a = Self::geom_of(host, obj)?;
        let r = collide::dist_calc(dc, goal_pos, Some(&a.geometry), other_pos, None, HUGE_DIST_SQR);
        Some((r.dist_sqr, r.vec))
    }

    /// `getRelativeAngle2D(obj, otherObj)`.
    pub fn get_relative_angle_2d<H: PartitionHost>(&self, host: &H, obj: ObjectId, other_obj: ObjectId) -> Option<f32> {
        let p = Self::geom_of(host, other_obj)?.position;
        self.get_relative_angle_2d_pos(host, obj, &p)
    }

    /// `getRelativeAngle2D(obj, pos)`: the signed angle from the object's facing
    /// to the direction of `pos`, in (-PI, PI].
    pub fn get_relative_angle_2d_pos<H: PartitionHost>(&self, host: &H, obj: ObjectId, pos: &Coord3D) -> Option<f32> {
        let obj_pos = Self::geom_of(host, obj)?.position;

        // compute vector to given position
        let mut v = Coord3D { x: pos.x - obj_pos.x, y: pos.y - obj_pos.y, z: 0.0 };

        let dist = Real::sqrt(v.x * v.x + v.y * v.y);

        // normalize
        if dist == 0.0 {
            return Some(0.0);
        }

        let dir = host.unit_direction_vector_2d(obj);

        let dist_inv = 1.0 / dist;
        v.x *= dist_inv;
        v.y *= dist_inv;
        v.z *= dist_inv;

        // dot of two unit vectors is cos of angle
        let mut c = dir.x * v.x + dir.y * v.y;

        // bound it in case of numerical error
        c = c.clamp(-1.0, 1.0); // (a NaN passes through, as in the if/else-if chain)

        let mut value = trig::acos(c);

        // Determine sign by checking Z component of dir cross v
        let perp_z = dir.x * v.y - dir.y * v.x;
        if perp_z < 0.0 {
            value = -value;
        }

        Some(value)
    }

    /// `geomCollidesWithGeom(pos1, geom1, angle1, pos2, geom2, angle2)`.
    pub fn geom_collides_with_geom(&self, pos1: &Coord3D, geom1: &GeometryInfo, angle1: f32, pos2: &Coord3D, geom2: &GeometryInfo, angle2: f32) -> bool {
        collide::geom_collides_with_geom(pos1, geom1, angle1, pos2, geom2, angle2)
    }

    // ---- lines and walks over the grid ---------------------------------------------

    /// `iterateCellsAlongLine(pos, posOther, proc, userData)`: Bresenham from the
    /// cell of `pos` to the cell of `pos_other`; `proc` returns non-zero to stop,
    /// and that value is returned. Cells off the grid are stepped over.
    pub fn iterate_cells_along_line(&self, pos: &Coord3D, pos_other: &Coord3D, mut proc: impl FnMut(&PartitionCell) -> i32) -> i32 {
        let (start_x, start_y) = self.world_to_cell(pos.x, pos.y);
        let (end_x, end_y) = self.world_to_cell(pos_other.x, pos_other.y);

        let delta_x = end_x.wrapping_sub(start_x).wrapping_abs(); // The difference between the x's
        let delta_y = end_y.wrapping_sub(start_y).wrapping_abs(); // The difference between the y's
        let mut x = start_x; // Start x off at the first pixel
        let mut y = start_y; // Start y off at the first pixel

        let (mut xinc1, mut xinc2) = if end_x >= start_x { (1, 1) } else { (-1, -1) };
        let (mut yinc1, mut yinc2) = if end_y >= start_y { (1, 1) } else { (-1, -1) };

        let den: i32;
        let mut num: i32;
        let numadd: i32;
        let numpixels: i32;
        if delta_x >= delta_y {
            // There is at least one x-value for every y-value
            xinc1 = 0; // Don't change the x when numerator >= denominator
            yinc2 = 0; // Don't change the y for every iteration
            den = delta_x;
            num = delta_x / 2;
            numadd = delta_y;
            numpixels = delta_x; // There are more x-values than y-values
        } else {
            // There is at least one y-value for every x-value
            xinc2 = 0; // Don't change the x for every iteration
            yinc1 = 0; // Don't change the y when numerator >= denominator
            den = delta_y;
            num = delta_y / 2;
            numadd = delta_x;
            numpixels = delta_y; // There are more y-values than x-values
        }

        let mut curpixel = 0;
        while curpixel <= numpixels {
            if let Some(cell) = self.cell_index(x, y) {
                // might be null if off the edge
                let ret = proc(&self.cells[cell]);
                if ret != 0 {
                    return ret; // bail early
                }
            }

            num = num.wrapping_add(numadd); // Increase the numerator by the top of the fraction
            if num >= den {
                // Check if numerator >= denominator
                num = num.wrapping_sub(den); // Calculate the new numerator value
                x = x.wrapping_add(xinc1); // Change the x as appropriate
                y = y.wrapping_add(yinc1); // Change the y as appropriate
            }
            x = x.wrapping_add(xinc2); // Change the x as appropriate
            y = y.wrapping_add(yinc2); // Change the y as appropriate
            curpixel += 1;
        }

        0
    }

    /// `estimateTerrainExtremesAlongLine(start, end, &minZ, &maxZ, &minZPos, &maxZPos)`:
    /// rough-and-quick extremes from the cells' cached height ranges (`PM_CACHE_TERRAIN_HEIGHT`).
    /// Returns whether any cell was visited, and the extremes with the cell centres they come from.
    pub fn estimate_terrain_extremes_along_line(&self, start_world: &Coord3D, end_world: &Coord3D) -> TerrainExtremes {
        let mut out = TerrainExtremes { is_valid: false, min_z: HUGE_DIST, max_z: -HUGE_DIST, min_z_pos: None, max_z_pos: None };
        let mut min_z = HUGE_DIST;
        let mut max_z = -HUGE_DIST;
        let mut min_pos: Option<Coord2D> = None;
        let mut max_pos: Option<Coord2D> = None;
        let mut valid = false;
        let half = self.cell_size * 0.5;
        let lo = self.world_extents.lo;
        self.iterate_cells_along_line(start_world, end_world, |cell| {
            valid = true;
            let center = |c: &PartitionCell| Coord2D {
                x: lo.x + (c.get_cell_x() as f32 * self.cell_size) + half,
                y: lo.y + (c.get_cell_y() as f32 * self.cell_size) + half,
            };
            let tmp = cell.get_lo_terrain();
            if tmp < min_z {
                min_z = tmp;
                min_pos = Some(center(cell));
            }
            let tmp = cell.get_hi_terrain();
            if tmp > max_z {
                max_z = tmp;
                max_pos = Some(center(cell));
            }
            0 // zero to continue
        });
        out.is_valid = valid;
        out.min_z = min_z;
        out.max_z = max_z;
        out.min_z_pos = min_pos;
        out.max_z_pos = max_pos;
        out
    }

    /// `iterateCellsBreadthFirst(pos, proc, userData)`: starting at `pos`, left,
    /// up, right, down, breadth first. `proc` returns true to stop; the result is
    /// the index of the cell that stopped it (`-1` if none did). The second
    /// result is true if the walk reached a case where upstream reads out of
    /// bounds (see the module docs).
    ///
    /// Upstream expands the neighbours of the cell processed *before* the one it
    /// pops (it adds the neighbours of `cur`, then pops the front and sets `cur`
    /// to it); that lag is kept.
    pub fn iterate_cells_breadth_first(&self, pos: &Coord3D, mut proc: impl FnMut(&PartitionCell) -> bool) -> (i32, bool) {
        let (cell_x, cell_y) = self.world_to_cell(pos.x, pos.y);
        let Some(start) = self.cell_index(cell_x, cell_y) else {
            return (-1, true);
        };

        let cell_count = self.cells.len();
        // 0 means not done, 1 means done.
        let mut bit_field = vec![false; cell_count];
        let mut cell_q: VecDeque<usize> = VecDeque::new();

        // mark the starting cell done
        bit_field[start] = true;
        cell_q.push_back(start);

        let w = self.cell_count_x;
        let mut cur_x = cell_x;
        let mut cur_y = cell_y;
        let mut undefined = false;
        while !cell_q.is_empty() {
            // first, add the neighbors
            // left
            if cur_x > 0 {
                let i = (cur_y * w + cur_x - 1) as usize;
                if !bit_field[i] {
                    bit_field[i] = true;
                    cell_q.push_back(i);
                }
            }

            // top
            if cur_y > 0 {
                let i = ((cur_y - 1) * w + cur_x) as usize;
                if !bit_field[i] {
                    bit_field[i] = true;
                    cell_q.push_back(i);
                }
            }

            // right
            if cur_x + 1 < w {
                let i = (cur_y * w + cur_x + 1) as usize;
                if !bit_field[i] {
                    bit_field[i] = true;
                    cell_q.push_back(i);
                }
            }

            // bottom (upstream tests `curY + 1 > 0`; see the module docs)
            if cur_y + 1 < self.cell_count_y {
                let i = ((cur_y + 1) * w + cur_x) as usize;
                if !bit_field[i] {
                    bit_field[i] = true;
                    cell_q.push_back(i);
                }
            } else {
                undefined = true;
            }

            // now process the current top.
            let Some(cur_cell) = cell_q.pop_front() else {
                break;
            };
            cur_x = self.cells[cur_cell].get_cell_x();
            cur_y = self.cells[cur_cell].get_cell_y();

            if proc(&self.cells[cur_cell]) {
                return (self.cells[cur_cell].get_cell_y() * w + self.cells[cur_cell].get_cell_x(), undefined);
            }
        }

        (-1, undefined)
    }

    // ---- line of sight -------------------------------------------------------------

    /// `isClearLineOfSightTerrain(obj, objPos, other, otherPos)`: whether there is
    /// clear line of sight between the two positions, taking only terrain into
    /// account. An object contributes its position raised by the top of its
    /// collision shape (the "eyes"); otherwise the given position is used.
    pub fn is_clear_line_of_sight_terrain<H: PartitionHost>(&self, host: &H, obj: Option<ObjectId>, obj_pos: &Coord3D, other: Option<ObjectId>, other_pos: &Coord3D) -> bool {
        let pos = match obj.and_then(|o| host.object_geometry(o)) {
            Some(g) => {
                let mut p = g.position;
                // note that we want to measure from the top of the collision shape, not the bottom!
                p.z += g.geometry.get_max_height_above_position();
                p
            }
            None => *obj_pos,
        };

        let pos_other = match other.and_then(|o| host.object_geometry(o)) {
            Some(g) => {
                let mut p = g.position;
                p.z += g.geometry.get_max_height_above_position();
                p
            }
            None => *other_pos,
        };

        host.is_clear_line_of_sight(&pos, &pos_other)
    }

    // ---- ground and structure height ---------------------------------------------------

    /// `getGroundOrStructureHeight(posx, posy)`: the terrain height there, plus
    /// the height of the tallest structure within a cell-ish radius.
    pub fn get_ground_or_structure_height<H: PartitionFilterHost>(&mut self, host: &H, posx: f32, posy: f32) -> f32 {
        // get the terrain height
        let terrain_height_here = host.ground_height(posx, posy);

        // scan all objects in the radius of our extent and find the tallest height among them
        let mut mask = crate::common::kind_of::KindOfMaskType::new();
        mask.set(KINDOF_STRUCTURE, 1);
        let filters = [PartitionFilter::AcceptByKindOf { must_be_set: mask, must_be_clear: crate::common::kind_of::KindOfMaskType::new() }];
        let pos = Coord3D { x: posx, y: posy, z: terrain_height_here };
        const RANGE: f32 = 1.0;
        let mut iter = self.iterate_objects_in_range_at(host, &pos, RANGE, DistanceCalculationType::FromBoundingSphere2D, &filters, IterOrderType::Fastest);

        let mut tallest_height = 0.0f32;
        let mut o = iter.first();
        while let Some(obj) = o {
            // store the height of the tallest object under us
            if let Some(g) = host.object_geometry(obj) {
                let this_height = g.geometry.get_max_height_above_position();
                if this_height > tallest_height {
                    tallest_height = this_height;
                }
            }
            o = iter.next();
        }

        terrain_height_here + tallest_height
    }

    // ---- finding a legal position -----------------------------------------------------------

    /// `tryPosition(center, dist, angle, options, &result)`: whether the spot
    /// `dist` from `center` at `angle` is legal under `options` (terrain, cliffs,
    /// impassable cells, water, objects, and a path from the source object).
    pub fn try_position<H: PartitionFilterHost>(&mut self, host: &H, center: &Coord3D, dist: f32, angle: f32, options: &FindPositionOptions) -> Option<Coord3D> {
        // compute the spot on the terrain we've picked
        let mut pos = Coord3D { x: dist * trig::cos(angle) + center.x, y: dist * trig::sin(angle) + center.y, z: 0.0 };

        let mut layer = LAYER_GROUND;
        if options.flags & fpf::USE_HIGHEST_LAYER != 0 {
            pos.z = 99999.0;
            layer = host.highest_layer_for_destination(&pos);
            pos.z = host.layer_height(pos.x, pos.y, layer);
            // ensure we are slightly above the bridge, to account for fudge & sloppy art
            if layer != LAYER_GROUND {
                pos.z += 1.0;
            }
        } else {
            pos.z = host.ground_height(pos.x, pos.y);
        }

        if (pos.z - center.z).abs() > options.max_z_delta {
            return None;
        }

        // we don't usually find positions on cliffs.
        if host.is_cliff_cell(pos.x, pos.y) && layer == LAYER_GROUND {
            return None;
        }

        // we don't usually find positions on impassable areas.
        {
            let cell_x = real_to_int_floor(pos.x / PATHFIND_CELL_SIZE);
            let cell_y = real_to_int_floor(pos.y / PATHFIND_CELL_SIZE);
            match host.pathfind_cell_type(layer, cell_x, cell_y) {
                None | Some(PathfindCellType::Impassable) => return None,
                Some(t) => {
                    if options.flags & fpf::CLEAR_CELLS_ONLY != 0 && t != PathfindCellType::Clear {
                        return None;
                    }
                }
            }
        }

        // we don't usually find positions in the water unless we explicitly say that's OK,
        // or if the option is set we can only pick position underwater
        if options.flags & fpf::IGNORE_WATER == 0 {
            let is_underwater = host.is_underwater(pos.x, pos.y);

            // if we want water spots only and this is underwater it's no good, otherwise we want
            // the default behavior where underwater spots are invalid
            // (upstream: `if (WATER_ONLY && (!underwater || !ground)) return FALSE; else if
            // (underwater && ground) return FALSE;`: the two refusals are one condition)
            if (options.flags & fpf::WATER_ONLY != 0 && (!is_underwater || layer != LAYER_GROUND)) || (is_underwater && layer == LAYER_GROUND) {
                return None;
            }
        }

        // object checks
        if options.flags & fpf::IGNORE_ALL_OBJECTS == 0 {
            // iterate the potential collisions at this location using a very small sphere geometry
            // around the point
            let geometry = GeometryInfo::new(GeometryType::Sphere, true, 5.0, 5.0, 5.0);
            let mut iter = self.iterate_potential_collisions(host, &pos, &geometry, angle, true);

            let mut them_opt = iter.first();
            while let Some(them) = them_opt {
                them_opt = iter.next();

                // if this is the ignore object, ignore it
                if Some(them) == options.ignore_object {
                    continue;
                }

                // relationship checks
                if let Some(rel_obj) = options.relationship_object {
                    use crate::common::game_common::Relationship::Enemies;
                    // if this is an ally/neutral unit and we ignore those, do so
                    if options.flags & fpf::IGNORE_ALLY_OR_NEUTRAL_UNITS != 0 && host.relationship(rel_obj, them) != Enemies && is_unit(host, them) {
                        continue;
                    }

                    // if this is an ally/neutral structure and we ignore those, do so
                    if options.flags & fpf::IGNORE_ALLY_OR_NEUTRAL_STRUCTURES != 0 && host.relationship(rel_obj, them) != Enemies && host.is_kind_of(them, KINDOF_STRUCTURE) {
                        continue;
                    }

                    // if this is an enemy unit and we ignore those, do so
                    if options.flags & fpf::IGNORE_ENEMY_UNITS != 0 && host.relationship(rel_obj, them) == Enemies && is_unit(host, them) {
                        continue;
                    }

                    // if this is an enemy structure and we ignore those, do so
                    if options.flags & fpf::IGNORE_ENEMY_STRUCTURES != 0 && host.relationship(rel_obj, them) == Enemies && host.is_kind_of(them, KINDOF_STRUCTURE) {
                        continue;
                    }
                }

                // if we have a source that must path to the destination we will ignore that too
                if Some(them) == options.source_to_path_to_dest {
                    continue;
                }

                // oops, we have overlapped something
                return None;
            }
        }

        // finally ... if sourceToPathDest is valid ... the source object must be able to find a path
        // the the position we have selected.
        if let Some(src) = options.source_to_path_to_dest {
            if host.quick_does_path_exist(src, &pos) == Some(false) {
                return None;
            }
        }

        // save result and return TRUE for position found
        Some(pos)
    }

    /// `findPositionAround(center, options, &result)`: a legal position at least
    /// `minRadius` and at most `maxRadius` from `center`, searching concentric
    /// rings outward, ping-ponging around the start angle on each ring. A centre
    /// off the map is returned as it is (a scripted setup). `None` when no
    /// legal position exists.
    pub fn find_position_around<H: PartitionFilterHost>(&mut self, host: &mut H, center: &Coord3D, options: &FindPositionOptions) -> Option<Coord3D> {
        let extent = host.maximum_pathfind_extent();
        // If the goal is off the map, it is a scripted setup, so just use the center.
        if !extent.is_in_region_no_z(center) {
            return Some(*center);
        }

        // pick a random angle from the center location to start at
        let start_angle = if options.start_angle == RANDOM_START_ANGLE { host.game_logic_random_value_real(0.0, TWO_PI) } else { options.start_angle };

        // start the search at the most inner ring (minRadius) and expand to outer most ring (maxRadius)
        let mut dist = options.min_radius;
        while dist <= options.max_radius {
            // given the spacing that we've been using for the angles on the 'idea' inner circle, we
            // will need more points on larger circles to cover more ground
            let angle_spacing = if dist == options.min_radius { TWO_PI } else { (RING_SPACING / (dist + 1.0)) * (TWO_PI / 6.0) };

            // on this "ring", try all the angles available to us in a circle ... we'll start at the
            // 'startAngle', then try 'angleSpacing' to the left, then to the right, then '2
            // angleSpacings' to the left etc., ping ponging around the start angle
            let samples = real_to_int_ceil((TWO_PI / angle_spacing) / 2.0);
            let mut i = 0;
            while i < samples {
                // try one "side"
                if let Some(r) = self.try_position(&*host, center, dist, start_angle + angle_spacing * i as f32, options) {
                    return Some(r);
                }

                // try the other "side" ... note for the first point at minRadius this position is
                // the same so we won't test it again in that case
                if i != 0 {
                    if let Some(r) = self.try_position(&*host, center, dist, start_angle - angle_spacing * i as f32, options) {
                        return Some(r);
                    }
                }
                i += 1;
            }

            dist += RING_SPACING;
        }

        // no position was able to be found
        None
    }
}

/// What `estimateTerrainExtremesAlongLine` fills in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainExtremes {
    /// The return value: some cell was visited.
    pub is_valid: bool,
    /// `*minZ` (`HUGE_DIST` if no cell was lower).
    pub min_z: f32,
    /// `*maxZ` (`-HUGE_DIST` if no cell was higher).
    pub max_z: f32,
    /// `*minZPos`: the centre of the cell with the lowest terrain, if any improved on `HUGE_DIST`.
    pub min_z_pos: Option<Coord2D>,
    /// `*maxZPos`.
    pub max_z_pos: Option<Coord2D>,
}
