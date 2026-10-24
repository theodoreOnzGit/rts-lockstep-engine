// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/BezierSegment.h
//   GeneralsMD/Code/GameEngine/Source/Common/Bezier/BezierSegment.cpp
//   GeneralsMD/Code/GameEngine/Include/Common/BezFwdIterator.h
//   GeneralsMD/Code/GameEngine/Source/Common/Bezier/BezFwdIterator.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   John K McDonald, Jr., September 2002 (BezFwdIterator: "DO NOT DISTRIBUTE")
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BezierSegment`: a cubic Bezier curve in 3D (four control points), with
//! evaluation at `t`, subdivision, an approximate length, and the forward
//! differencing iterator that samples it at evenly spaced `t` values.
//!
//! # The D3DX operations
//!
//! Upstream uses `D3DXVec4Transform` and `D3DXVec4Dot` on the cubic basis
//! matrix. They are written out here as the D3DX inline formulas (row vector
//! times matrix, and a four-term dot product summed left to right). That is
//! the transcription used by the differential build too; it is not checked
//! against the Win32 `d3dx8` library, which the shipped game linked.
//!
//! # Quirks kept
//!
//! - [`BezFwdIterator::start`] returns before setting the current point when
//!   there are at most one step, so a one-step sample is the zero point, not
//!   the first control point. Kept (it is what the shipped code returns).
//! - [`BezierSegment::get_approximate_length`] recurses until the control
//!   polygon is within tolerance. Upstream's recursion has no limit, so a
//!   tolerance finer than the rounding noise of the curve overflows the
//!   stack. Here the recursion stops at [`MAX_SUBDIVISION_DEPTH`] and returns
//!   [`BezierError::SubdivisionLimit`].

use super::base_type::Coord3D;

/// `#define USUAL_TOLERANCE 1.0f`.
pub const USUAL_TOLERANCE: f32 = 1.0;

/// Deepest subdivision [`BezierSegment::get_approximate_length`] follows
/// before giving up (upstream never gives up; see the module docs).
pub const MAX_SUBDIVISION_DEPTH: u32 = 64;

/// Where upstream throws or never returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BezierError {
    /// `getSegmentPoints` with a negative count: `std::vector::resize` throws.
    NegativeSegmentCount,
    /// `getApproximateLength` needed more than [`MAX_SUBDIVISION_DEPTH`]
    /// levels (upstream recurses until the stack overflows).
    SubdivisionLimit,
}

impl std::fmt::Display for BezierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BezierError::NegativeSegmentCount => {
                "BezierSegment::getSegmentPoints: negative count (std::length_error upstream)"
            }
            BezierError::SubdivisionLimit => {
                "BezierSegment::getApproximateLength: subdivision limit (stack overflow upstream)"
            }
        })
    }
}

/// `s_bezBasisMatrix` (`D3DXMATRIX`, row-major): the cubic Bernstein basis.
const BEZ_BASIS: [[f32; 4]; 4] = [
    [-1.0, 3.0, -3.0, 1.0],
    [3.0, -6.0, 3.0, 0.0],
    [-3.0, 3.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0],
];

/// `D3DXVec4Transform(out, v, m)`: the row vector `v` times the matrix `m`.
fn vec4_transform(v: [f32; 4], m: &[[f32; 4]; 4]) -> [f32; 4] {
    let mut out = [0.0f32; 4];
    for (j, o) in out.iter_mut().enumerate() {
        *o = m[0][j] * v[0] + m[1][j] * v[1] + m[2][j] * v[2] + m[3][j] * v[3];
    }
    out
}

/// `D3DXVec4Dot(a, b)`.
fn vec4_dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// `class BezierSegment`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BezierSegment {
    control_points: [Coord3D; 4],
}

impl Default for BezierSegment {
    /// `BezierSegment()`: four zero control points.
    fn default() -> Self {
        BezierSegment::new()
    }
}

impl BezierSegment {
    /// `BezierSegment()`: four zero control points.
    pub fn new() -> Self {
        BezierSegment {
            control_points: [Coord3D::default(); 4],
        }
    }

    /// `BezierSegment(cp0, cp1, cp2, cp3)`.
    pub fn from_points(cp0: Coord3D, cp1: Coord3D, cp2: Coord3D, cp3: Coord3D) -> Self {
        BezierSegment {
            control_points: [cp0, cp1, cp2, cp3],
        }
    }

    /// `BezierSegment(Real cp[12])`: x, y, z of each control point in turn.
    pub fn from_floats(cp: &[f32; 12]) -> Self {
        let p = |i: usize| Coord3D {
            x: cp[3 * i],
            y: cp[3 * i + 1],
            z: cp[3 * i + 2],
        };
        BezierSegment {
            control_points: [p(0), p(1), p(2), p(3)],
        }
    }

    /// The control points (`m_controlPoints`).
    pub fn control_points(&self) -> &[Coord3D; 4] {
        &self.control_points
    }

    /// `evaluateBezSegmentAtT(tValue, outResult)`: the point at parameter `t`.
    pub fn evaluate_at_t(&self, t: f32) -> Coord3D {
        let t_vec = [t * t * t, t * t, t, 1.0f32];
        let cp = &self.control_points;
        let t_result = vec4_transform(t_vec, &BEZ_BASIS);
        Coord3D {
            x: vec4_dot([cp[0].x, cp[1].x, cp[2].x, cp[3].x], t_result),
            y: vec4_dot([cp[0].y, cp[1].y, cp[2].y, cp[3].y], t_result),
            z: vec4_dot([cp[0].z, cp[1].z, cp[2].z, cp[3].z], t_result),
        }
    }

    /// `getSegmentPoints(numSegments, outResult)`: `numSegments` points from
    /// [`BezFwdIterator`]. The output is cleared first, as upstream does.
    pub fn get_segment_points(
        &self,
        num_segments: i32,
        out: &mut Vec<Coord3D>,
    ) -> Result<(), BezierError> {
        out.clear();
        if num_segments < 0 {
            return Err(BezierError::NegativeSegmentCount);
        }
        let mut iter = BezFwdIterator::new_with(num_segments, self);
        iter.start();
        while !iter.done() {
            out.push(*iter.get_current());
            iter.next();
        }
        Ok(())
    }

    /// `getApproximateLength(withinTolerance)`: half the sum of the control
    /// polygon's length and the chord's length, subdividing at `t = 0.5`
    /// while their difference exceeds the tolerance.
    pub fn get_approximate_length(&self, within_tolerance: f32) -> Result<f32, BezierError> {
        self.approximate_length_at_depth(within_tolerance, 0)
    }

    fn approximate_length_at_depth(
        &self,
        within_tolerance: f32,
        depth: u32,
    ) -> Result<f32, BezierError> {
        let cp = &self.control_points;
        let p0p1 = Coord3D {
            x: cp[1].x - cp[0].x,
            y: cp[1].y - cp[0].y,
            z: cp[1].z - cp[0].z,
        };
        let p1p2 = Coord3D {
            x: cp[2].x - cp[1].x,
            y: cp[2].y - cp[1].y,
            z: cp[2].z - cp[1].z,
        };
        let p2p3 = Coord3D {
            x: cp[3].x - cp[2].x,
            y: cp[3].y - cp[2].y,
            z: cp[3].z - cp[2].z,
        };
        let p0p3 = Coord3D {
            x: cp[3].x - cp[0].x,
            y: cp[3].y - cp[0].y,
            z: cp[3].z - cp[0].z,
        };

        let length0 = p0p3.length();
        let length1 = p0p1.length() + p1p2.length() + p2p3.length();

        if (length1 - length0) > within_tolerance {
            if depth >= MAX_SUBDIVISION_DEPTH {
                return Err(BezierError::SubdivisionLimit);
            }
            let (seg1, seg2) = self.split_segment_at_t(0.5f32);
            let a = seg1.approximate_length_at_depth(within_tolerance, depth + 1)?;
            let b = seg2.approximate_length_at_depth(within_tolerance, depth + 1)?;
            return Ok(a + b);
        }
        // `INT_TO_REAL(length0 + length1) / 2.0f`: `INT_TO_REAL` is a cast to `Real`.
        Ok((length0 + length1) / 2.0f32)
    }

    /// `splitSegmentAtT(tValue, outSeg1, outSeg2)`: de Casteljau subdivision
    /// into the parts before and after `t`.
    pub fn split_segment_at_t(&self, t: f32) -> (BezierSegment, BezierSegment) {
        let cp = &self.control_points;
        let mut p0p1 = Coord3D {
            x: cp[1].x - cp[0].x,
            y: cp[1].y - cp[0].y,
            z: cp[1].z - cp[0].z,
        };
        let mut p1p2 = Coord3D {
            x: cp[2].x - cp[1].x,
            y: cp[2].y - cp[1].y,
            z: cp[2].z - cp[1].z,
        };
        let mut p2p3 = Coord3D {
            x: cp[3].x - cp[2].x,
            y: cp[3].y - cp[2].y,
            z: cp[3].z - cp[2].z,
        };
        p0p1.scale(t);
        p1p2.scale(t);
        p2p3.scale(t);
        p0p1.add(&cp[0]);
        p1p2.add(&cp[1]);
        p2p3.add(&cp[2]);

        let mut tri_left = Coord3D {
            x: p1p2.x - p0p1.x,
            y: p1p2.y - p0p1.y,
            z: p1p2.z - p0p1.z,
        };
        let mut tri_right = Coord3D {
            x: p2p3.x - p1p2.x,
            y: p2p3.y - p1p2.y,
            z: p2p3.z - p1p2.z,
        };
        tri_left.scale(t);
        tri_right.scale(t);
        tri_left.add(&p0p1);
        tri_right.add(&p1p2);

        let mid = self.evaluate_at_t(t);
        let seg1 = BezierSegment::from_points(cp[0], p0p1, tri_left, mid);
        let seg2 = BezierSegment::from_points(mid, tri_right, p2p3, cp[3]);
        (seg1, seg2)
    }
}

/// `class BezFwdIterator`: samples a segment at `steps` evenly spaced `t`
/// values by forward differencing (the first, second and third derivatives
/// of each axis are carried from one sample to the next).
#[derive(Clone, Debug, PartialEq)]
pub struct BezFwdIterator {
    step: i32,
    steps_desired: i32,
    bez_seg: BezierSegment,
    curr_point: Coord3D,
    /// First derivative.
    dq: Coord3D,
    /// Second derivative.
    ddq: Coord3D,
    /// Third derivative.
    dddq: Coord3D,
}

impl Default for BezFwdIterator {
    /// `BezFwdIterator()`: no steps, zero everything.
    fn default() -> Self {
        BezFwdIterator {
            step: 0,
            steps_desired: 0,
            bez_seg: BezierSegment::new(),
            curr_point: Coord3D::default(),
            dq: Coord3D::default(),
            ddq: Coord3D::default(),
            dddq: Coord3D::default(),
        }
    }
}

impl BezFwdIterator {
    /// `BezFwdIterator()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `BezFwdIterator(stepsDesired, bezSeg)`. The derivatives are set by
    /// [`BezFwdIterator::start`].
    pub fn new_with(steps_desired: i32, bez_seg: &BezierSegment) -> Self {
        BezFwdIterator {
            steps_desired,
            bez_seg: *bez_seg,
            ..Self::default()
        }
    }

    /// `start()`: rewinds to the first sample and computes the differences.
    /// With at most one step the current point is left at zero (upstream
    /// returns before setting it; kept).
    pub fn start(&mut self) {
        self.step = 0;
        if self.steps_desired <= 1 {
            return;
        }
        let d = 1.0f32 / ((self.steps_desired - 1) as f32);
        let d2 = d * d;
        let d3 = d * d2;
        let cp = self.bez_seg.control_points;
        let coeffs = [
            vec4_transform([cp[0].x, cp[1].x, cp[2].x, cp[3].x], &BEZ_BASIS),
            vec4_transform([cp[0].y, cp[1].y, cp[2].y, cp[3].y], &BEZ_BASIS),
            vec4_transform([cp[0].z, cp[1].z, cp[2].z, cp[3].z], &BEZ_BASIS),
        ];
        self.curr_point = cp[0];
        // Axis `i`: `(a, b, c)` are the cubic, quadratic and linear coefficients.
        for (i, c) in coeffs.iter().enumerate() {
            let (a, b, cc) = (c[0], c[1], c[2]);
            let d_val = a * d3 + b * d2 + cc * d;
            let dd_val = 6.0f32 * a * d3 + 2.0f32 * b * d2;
            let ddd_val = 6.0f32 * a * d3;
            set_axis(&mut self.dq, i, d_val);
            set_axis(&mut self.ddq, i, dd_val);
            set_axis(&mut self.dddq, i, ddd_val);
        }
    }

    /// `done()`.
    pub fn done(&self) -> bool {
        self.step >= self.steps_desired
    }

    /// `getCurrent()`.
    pub fn get_current(&self) -> &Coord3D {
        &self.curr_point
    }

    /// `next()`: advances one sample.
    pub fn next(&mut self) {
        self.curr_point.add(&self.dq);
        self.dq.add(&self.ddq);
        self.ddq.add(&self.dddq);
        self.step += 1;
    }
}

fn set_axis(p: &mut Coord3D, axis: usize, v: f32) {
    match axis {
        0 => p.x = v,
        1 => p.y = v,
        _ => p.z = v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32, z: f32) -> Coord3D {
        Coord3D { x, y, z }
    }

    #[test]
    fn constructors_place_the_control_points_in_order() {
        assert_eq!(
            BezierSegment::new().control_points(),
            &[Coord3D::default(); 4]
        );
        assert_eq!(BezierSegment::default(), BezierSegment::new());
        let s = BezierSegment::from_points(
            p(1.0, 2.0, 3.0),
            p(4.0, 5.0, 6.0),
            p(7.0, 8.0, 9.0),
            p(10.0, 11.0, 12.0),
        );
        let f = BezierSegment::from_floats(&[
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        ]);
        assert_eq!(s, f);
        assert_eq!(s.control_points()[3], p(10.0, 11.0, 12.0));
    }

    #[test]
    fn endpoints_are_the_first_and_last_control_points() {
        let s = BezierSegment::from_points(
            p(0.0, 0.0, 0.0),
            p(1.0, 3.0, 0.0),
            p(2.0, -3.0, 1.0),
            p(3.0, 0.0, 2.0),
        );
        assert_eq!(s.evaluate_at_t(0.0), p(0.0, 0.0, 0.0));
        assert_eq!(s.evaluate_at_t(1.0), p(3.0, 0.0, 2.0));
    }

    #[test]
    fn iterator_without_steps_is_done_and_at_zero() {
        let mut it = BezFwdIterator::new();
        it.start();
        assert!(it.done());
        assert_eq!(*it.get_current(), Coord3D::default());
    }

    #[test]
    fn one_sample_is_the_zero_point_as_upstream_returns_it() {
        let s = BezierSegment::from_points(
            p(5.0, 5.0, 5.0),
            p(0.0, 0.0, 0.0),
            p(0.0, 0.0, 0.0),
            p(9.0, 9.0, 9.0),
        );
        let mut out = Vec::new();
        s.get_segment_points(1, &mut out).expect("one point");
        assert_eq!(out, vec![Coord3D::default()]);
    }

    #[test]
    fn negative_counts_are_errors_and_clear_the_output() {
        let s = BezierSegment::new();
        let mut out = vec![p(1.0, 1.0, 1.0)];
        assert_eq!(
            s.get_segment_points(-1, &mut out),
            Err(BezierError::NegativeSegmentCount)
        );
        assert!(out.is_empty());
    }

    #[test]
    fn errors_have_messages() {
        assert!(BezierError::NegativeSegmentCount
            .to_string()
            .contains("negative"));
        assert!(BezierError::SubdivisionLimit
            .to_string()
            .contains("subdivision"));
    }
}
