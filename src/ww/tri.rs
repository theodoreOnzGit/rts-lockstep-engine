// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/tri.h
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WWMath)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Point_In_Triangle_2D` of `tri.h`, the pure 2D point-in-triangle test on
//! the projection onto two axes, which `TerrainLogic` uses for bridge
//! footprints. The rest of `tri.h` (`TriClass`, the ray casts) is not
//! translated.
//!
//! Upstream bug kept: in the degenerate branch `p2p0dist2` is computed from
//! `p1p2` (`p2p0dist2 = p1p2.Length2()`), so the 'outer points' choice is
//! made on the wrong length.

use super::vector2::Vector2;
use super::vector3::Vector3;

/// `TRI_RAYCAST_FLAG_HIT_EDGE`.
pub const TRI_RAYCAST_FLAG_HIT_EDGE: u8 = 0x01;

/// `Point_In_Triangle_2D(tri_point0, tri_point1, tri_point2, test_point,
/// axis_1, axis_2, flags)`. `flags` gets `TRI_RAYCAST_FLAG_HIT_EDGE` set when
/// the point is on an edge, and is otherwise untouched, as upstream.
/// `axis_1` and `axis_2` are 0, 1 or 2 (anything else is an out-of-range
/// `operator[]` upstream and panics here).
pub fn point_in_triangle_2d(
    tri_point0: &Vector3,
    tri_point1: &Vector3,
    tri_point2: &Vector3,
    test_point: &Vector3,
    axis_1: usize,
    axis_2: usize,
    flags: &mut u8,
) -> bool {
    let p0p1 = Vector2::new(tri_point1[axis_1] - tri_point0[axis_1], tri_point1[axis_2] - tri_point0[axis_2]);
    let p1p2 = Vector2::new(tri_point2[axis_1] - tri_point1[axis_1], tri_point2[axis_2] - tri_point1[axis_2]);
    let p2p0 = Vector2::new(tri_point0[axis_1] - tri_point2[axis_1], tri_point0[axis_2] - tri_point2[axis_2]);

    // Which side P2 is relative to P0P1 (also: is the triangle degenerate).
    let p0p2 = Vector2::new(tri_point2[axis_1] - tri_point0[axis_1], tri_point2[axis_2] - tri_point0[axis_2]);
    let p0p1p2 = Vector2::perp_dot_product(&p0p1, &p0p2);
    if p0p1p2 != 0.0f32 {
        // The triangle is not degenerate - test three sides.
        let side_factor = if p0p1p2 > 0.0f32 { 1.0f32 } else { -1.0f32 };
        let mut factors = [0.0f32; 3];

        let p0p_t = Vector2::new(test_point[axis_1] - tri_point0[axis_1], test_point[axis_2] - tri_point0[axis_2]);
        factors[0] = Vector2::perp_dot_product(&p0p1, &p0p_t);
        if factors[0] * side_factor < 0.0f32 {
            return false;
        }
        let p1p_t = Vector2::new(test_point[axis_1] - tri_point1[axis_1], test_point[axis_2] - tri_point1[axis_2]);
        factors[1] = Vector2::perp_dot_product(&p1p2, &p1p_t);
        if factors[1] * side_factor < 0.0f32 {
            return false;
        }
        let p2p_t = Vector2::new(test_point[axis_1] - tri_point2[axis_1], test_point[axis_2] - tri_point2[axis_2]);
        factors[2] = Vector2::perp_dot_product(&p2p0, &p2p_t);
        if factors[2] * side_factor < 0.0f32 {
            return false;
        }
        if factors[0] == 0.0f32 || factors[1] == 0.0f32 || factors[2] == 0.0f32 {
            *flags |= TRI_RAYCAST_FLAG_HIT_EDGE;
        }
        true
    } else {
        // The triangle is degenerate. Find the two outer points along the
        // triangle's line ('start' and 'end' points).
        let p0p1dist2 = p0p1.length2();
        let p1p2dist2 = p1p2.length2();
        let p2p0dist2 = p1p2.length2(); // upstream: p1p2, not p2p0
        let max_dist2;
        let p_sp_e: Vector2;
        let p_sp_t: Vector2;
        if p0p1dist2 > p1p2dist2 {
            if p0p1dist2 > p2p0dist2 {
                // points 0 and 1 are the 'outer' points. 0 is 'start' and 1 is 'end'.
                p_sp_e = p0p1;
                p_sp_t = Vector2::new(test_point[axis_1] - tri_point0[axis_1], test_point[axis_2] - tri_point0[axis_2]);
                max_dist2 = p0p1dist2;
            } else {
                // points 0 and 2 are the 'outer' points. 2 is 'start' and 0 is 'end'.
                p_sp_e = p2p0;
                p_sp_t = Vector2::new(test_point[axis_1] - tri_point2[axis_1], test_point[axis_2] - tri_point2[axis_2]);
                max_dist2 = p2p0dist2;
            }
        } else if p1p2dist2 > p2p0dist2 {
            // points 1 and 2 are the 'outer' points. 1 is 'start' and 2 is 'end'.
            p_sp_e = p1p2;
            p_sp_t = Vector2::new(test_point[axis_1] - tri_point1[axis_1], test_point[axis_2] - tri_point1[axis_2]);
            max_dist2 = p1p2dist2;
        } else {
            // points 0 and 2 are the 'outer' points. 2 is 'start' and 0 is 'end'.
            p_sp_e = p2p0;
            p_sp_t = Vector2::new(test_point[axis_1] - tri_point2[axis_1], test_point[axis_2] - tri_point2[axis_2]);
            max_dist2 = p2p0dist2;
        }

        if max_dist2 != 0.0f32 {
            // Triangle is line segment, check if test point is colinear with it.
            if Vector2::perp_dot_product(&p_sp_e, &p_sp_t) != 0.0f32 {
                // Not colinear.
                false
            } else {
                // Colinear - is test point's distance from start and end <= segment length?
                let p_ep_t = p_sp_t - p_sp_e;
                if p_sp_t.length2() <= max_dist2 && p_ep_t.length2() <= max_dist2 {
                    *flags |= TRI_RAYCAST_FLAG_HIT_EDGE;
                    true
                } else {
                    false
                }
            }
        } else if p_sp_t.length2() == 0.0f32 {
            // All triangle points coincide, check if test point coincides with them.
            *flags |= TRI_RAYCAST_FLAG_HIT_EDGE;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_outside_and_edge() {
        let a = Vector3::new(0.0, 0.0, 0.0);
        let b = Vector3::new(4.0, 0.0, 0.0);
        let c = Vector3::new(0.0, 4.0, 0.0);
        let mut f = 0u8;
        assert!(point_in_triangle_2d(&a, &b, &c, &Vector3::new(1.0, 1.0, 9.0), 0, 1, &mut f));
        assert_eq!(f, 0);
        assert!(!point_in_triangle_2d(&a, &b, &c, &Vector3::new(3.0, 3.0, 0.0), 0, 1, &mut f));
        assert!(point_in_triangle_2d(&a, &b, &c, &Vector3::new(2.0, 0.0, 0.0), 0, 1, &mut f));
        assert_eq!(f, TRI_RAYCAST_FLAG_HIT_EDGE);
    }

    #[test]
    fn degenerate_triangles() {
        let a = Vector3::new(0.0, 0.0, 0.0);
        let b = Vector3::new(2.0, 2.0, 0.0);
        let c = Vector3::new(4.0, 4.0, 0.0);
        let mut f = 0u8;
        // colinear with the segment (0,0)-(4,4). Because of the kept upstream
        // bug (`p2p0dist2` is the length of `p1p2`, 8 not 32) the segment
        // length is taken as 8, so (1,1), which is on the segment, is
        // reported outside, and (2,2) is the only point inside.
        assert!(!point_in_triangle_2d(&a, &b, &c, &Vector3::new(1.0, 1.0, 0.0), 0, 1, &mut f));
        assert_eq!(f, 0);
        assert!(point_in_triangle_2d(&a, &b, &c, &Vector3::new(2.0, 2.0, 0.0), 0, 1, &mut f));
        assert_eq!(f, TRI_RAYCAST_FLAG_HIT_EDGE);
        // off the line
        assert!(!point_in_triangle_2d(&a, &b, &c, &Vector3::new(1.0, 2.0, 0.0), 0, 1, &mut f));
        // all three coincide
        let mut f = 0u8;
        assert!(point_in_triangle_2d(&a, &a, &a, &a, 0, 1, &mut f));
        assert_eq!(f, TRI_RAYCAST_FLAG_HIT_EDGE);
        assert!(!point_in_triangle_2d(&a, &a, &a, &Vector3::new(1.0, 0.0, 0.0), 0, 1, &mut f));
    }
}
