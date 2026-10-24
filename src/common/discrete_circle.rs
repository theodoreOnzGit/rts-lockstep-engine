// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/DiscreteCircle.h
//   GeneralsMD/Code/GameEngine/Source/Common/DiscreteCircle.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Created: John McDonald, September 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `DiscreteCircle`: the horizontal scanlines of a filled circle on the
//! partition grid (Bresenham; the top half, mirrored when drawn). The
//! partition manager uses it for shroud and look-at circles.
//!
//! # Integer semantics
//!
//! Upstream shifts negative `Int`s left (`d = (1 - radius) << 1`, and
//! `m_yPosDoubled = yCenter << 1` for a negative centre). That is undefined
//! in C++98-17 and defined since C++20 as multiplication by 2 (two's
//! complement), which is what MSVC always did, so the shipped game and
//! every compiler agree. **Not an owner decision yet:** the translation
//! treats these shifts as defined (`* 2`) because they are the normal path
//! (every radius from 2), and flags it for review.
//!
//! What is an error here ([`DiscreteCircleError`]): a negative radius
//! (upstream's `reserve(radius << 1)` throws `std::length_error`), and any
//! `Int` arithmetic that overflows (huge radii or centres; undefined
//! upstream).

/// `struct HorzLine`: draw `(xStart, yPos)` to `(xEnd, yPos)`, inclusive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HorzLine {
    pub y_pos: i32,
    pub x_start: i32,
    pub x_end: i32,
}

/// Where upstream throws or is undefined (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiscreteCircleError {
    /// `reserve(radius << 1)` of a negative size: `std::length_error`.
    NegativeRadius,
    /// `Int` arithmetic overflowed (undefined upstream).
    Overflow,
}

impl std::fmt::Display for DiscreteCircleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DiscreteCircleError::NegativeRadius => "DiscreteCircle: negative radius (std::length_error upstream)",
            DiscreteCircleError::Overflow => "DiscreteCircle: Int overflow (undefined upstream)",
        })
    }
}

/// `class DiscreteCircle`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscreteCircle {
    edges: Vec<HorzLine>,
    y_pos: i32,
    y_pos_doubled: i32,
}

fn fit(v: i64) -> Result<i32, DiscreteCircleError> {
    i32::try_from(v).map_err(|_| DiscreteCircleError::Overflow)
}

impl DiscreteCircle {
    /// `DiscreteCircle(xCenter, yCenter, radius)`.
    pub fn new(x_center: i32, y_center: i32, radius: i32) -> Result<Self, DiscreteCircleError> {
        let y_pos_doubled = fit(i64::from(y_center) * 2)?;
        let reserve = fit(i64::from(radius) * 2)?;
        if reserve < 0 {
            return Err(DiscreteCircleError::NegativeRadius);
        }
        let mut c = DiscreteCircle { edges: Vec::new(), y_pos: y_center, y_pos_doubled };
        c.generate_edge_pairs(x_center, y_center, radius)?;
        c.remove_duplicates();
        Ok(c)
    }

    /// `getEdges()`.
    pub fn get_edges(&self) -> &[HorzLine] {
        &self.edges
    }

    /// `getEdgeCount()`.
    pub fn get_edge_count(&self) -> i32 {
        self.edges.len() as i32
    }

    /// `drawCircle(func, parm)`: each edge, then its mirror below the centre
    /// unless it is the centre line.
    pub fn draw_circle(&self, mut draw: impl FnMut(i32, i32, i32)) -> Result<(), DiscreteCircleError> {
        for e in &self.edges {
            draw(e.x_start, e.x_end, e.y_pos);
            if e.y_pos != self.y_pos {
                draw(e.x_start, e.x_end, fit(i64::from(self.y_pos_doubled) - i64::from(e.y_pos))?);
            }
        }
        Ok(())
    }

    /// `generateEdgePairs`: Bresenham from the top of the circle down to the
    /// centre line.
    fn generate_edge_pairs(&mut self, x_center: i32, y_center: i32, radius: i32) -> Result<(), DiscreteCircleError> {
        let mut x: i32 = 0;
        let mut y: i32 = radius;
        let mut d: i32 = fit((1 - i64::from(radius)) * 2)?;
        while y >= 0 {
            self.edges.push(HorzLine {
                x_start: fit(i64::from(x_center) - i64::from(x))?,
                x_end: fit(i64::from(x_center) + i64::from(x))?,
                y_pos: fit(i64::from(y_center) + i64::from(y))?,
            });
            if fit(i64::from(d) + i64::from(y))? > 0 {
                y -= 1;
                d = fit(i64::from(d) - (i64::from(y) * 2 - 1))?;
            }
            if x > d {
                x += 1;
                d = fit(i64::from(d) + (i64::from(x) * 2 + 1))?;
            }
        }
        Ok(())
    }

    /// `removeDuplicates`: of consecutive edges on the same row, keep the
    /// last (the widest).
    fn remove_duplicates(&mut self) {
        let mut i = 0;
        while i + 1 < self.edges.len() {
            if self.edges[i].y_pos == self.edges[i + 1].y_pos {
                self.edges.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_circles() {
        let c = DiscreteCircle::new(0, 0, 0).unwrap();
        assert_eq!(c.get_edges(), &[HorzLine { y_pos: 0, x_start: 0, x_end: 0 }]);
        // Radius 3, worked by hand: Bresenham gives rows 23 (twice: x 0
        // then 1), 22, 21, 20; the duplicate row keeps its later, wider
        // edge; every row but the centre (20) is mirrored.
        let c = DiscreteCircle::new(10, 20, 3).unwrap();
        let mut rows = Vec::new();
        c.draw_circle(|a, b, y| rows.push((y, a, b))).unwrap();
        assert_eq!(rows, [(23, 9, 11), (17, 9, 11), (22, 8, 12), (18, 8, 12), (21, 7, 13), (19, 7, 13), (20, 7, 13)]);
        assert_eq!(DiscreteCircle::new(0, 0, -1), Err(DiscreteCircleError::NegativeRadius));
        assert_eq!(DiscreteCircle::new(0, i32::MAX, 1), Err(DiscreteCircleError::Overflow));
    }
}
