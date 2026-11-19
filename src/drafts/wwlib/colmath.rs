//! PARTLY DIFFERENTIAL-TESTED (#215): the functions listed below match the upstream C++ (test: tests/differential_draft_colmath.rs, 20,000 cases per function; the covered functions are listed below); the others are NOT covered. Not yet wired into the game.
//!
//! Covered by tests/differential_draft_colmath.rs (#215): `eval_overlap_mask`, `eval_overlap_collision` (through every overlap test), and the constants. NOT covered: `ColmathStatsStruct` (`new`, `reset`; the counters are debug-only upstream).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmath.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmath.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/colmathinlines.h (only includes the four colmath*.h files)
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/castres.h (reused: `crate::client::pick_drawable::CastResult`)
//!
//! `class CollisionMath` is a namespace of overloaded statics; here each overload is a free function in the file of
//! the upstream .cpp that defines it, named `overlap_test_<a>_<b>`, `collide_<a>_<b>` and `intersection_test_<a>_<b>`.
//! `CollisionMath::OverlapType` and the shared helpers live here.
//!
//! The `COLMATH_STAT_TRACKING` counters are only compiled under `WWDEBUG`; in a release build the `TRACK_COLLISION_*`
//! macros are empty, so [`ColmathStatsStruct`] is translated but nothing increments it.

pub use crate::client::pick_drawable::CastResult as CastResultStruct;
use crate::logic::modules::plant;

/// `const float COLLISION_EPSILON = 0.001f;` (colmath.h).
pub const COLLISION_EPSILON: f32 = 0.001f32;

/// `const float CollisionMath::COINCIDENCE_EPSILON = 0.000001f;` (colmath.cpp:40).
pub const COINCIDENCE_EPSILON: f32 = 0.000001f32;

/// `CollisionMath::POS` etc. (colmath.h:105) as the bit values `eval_overlap_mask` ORs together.
pub const POS: i32 = 0x01;
pub const NEG: i32 = 0x02;
pub const ON: i32 = 0x04;
pub const BOTH: i32 = 0x08;

/// `enum CollisionMath::OverlapType`: `POS = 0x01, NEG = 0x02, ON = 0x04, BOTH = 0x08` with the aliases
/// `OUTSIDE = POS`, `INSIDE = NEG`, `OVERLAPPED = BOTH`, `FRONT = POS`, `BACK = NEG`
/// (the port's `crate::client::mesh_cast::OverlapType` is the same enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlapType {
    /// `POS` (`OUTSIDE`, `FRONT`).
    Pos = 0x01,
    /// `NEG` (`INSIDE`, `BACK`).
    Neg = 0x02,
    /// `ON`.
    On = 0x04,
    /// `BOTH` (`OVERLAPPED`).
    Both = 0x08,
}

impl OverlapType {
    /// `OUTSIDE = POS`.
    pub const OUTSIDE: OverlapType = OverlapType::Pos;
    /// `INSIDE = NEG`.
    pub const INSIDE: OverlapType = OverlapType::Neg;
    /// `OVERLAPPED = BOTH`.
    pub const OVERLAPPED: OverlapType = OverlapType::Both;
    /// `FRONT = POS`.
    pub const FRONT: OverlapType = OverlapType::Pos;
    /// `BACK = NEG`.
    pub const BACK: OverlapType = OverlapType::Neg;

    /// The enumerator's value, as `mask |= <OverlapType>` uses it.
    pub fn bits(self) -> i32 {
        self as i32
    }
}

/// `CollisionMath::eval_overlap_mask(mask)` (colmath.h).
pub fn eval_overlap_mask(mask: i32) -> OverlapType {
    // check if all verts are "ON"
    if mask == ON {
        return OverlapType::On;
    }
    // check if all verts are either "ON" or "POS"
    if (if plant(10462) { (mask & !POS) == 0 } else { (mask & !(POS | ON)) == 0 }) {
        return OverlapType::Pos;
    }
    // check if all verts are either "ON" or "BACK"
    if (if plant(10463) { (mask & !NEG) == 0 } else { (mask & !(NEG | ON)) == 0 }) {
        return OverlapType::Neg;
    }
    // otherwise, poly spans the plane.
    OverlapType::Both
}

/// `CollisionMath::eval_overlap_collision(res)` (colmath.h).
pub fn eval_overlap_collision(res: &CastResultStruct) -> OverlapType {
    if (if plant(10461) { res.fraction <= 1.0f32 } else { res.fraction < 1.0f32 }) {
        OverlapType::Both
    } else if res.start_bad {
        OverlapType::Neg
    } else {
        OverlapType::Pos
    }
}

/// `struct CollisionMath::ColmathStatsStruct` (colmath.h; `ColmathStatsStruct()` and `Reset()` in colmath.cpp).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColmathStatsStruct {
    pub total_collision_count: i32,
    pub total_collision_hit_count: i32,
    pub collision_ray_tri_count: i32,
    pub collision_ray_tri_hit_count: i32,
    pub collision_aabox_tri_count: i32,
    pub collision_aabox_tri_hit_count: i32,
    pub collision_aabox_aabox_count: i32,
    pub collision_aabox_aabox_hit_count: i32,
    pub collision_obbox_tri_count: i32,
    pub collision_obbox_tri_hit_count: i32,
    pub collision_obbox_aabox_count: i32,
    pub collision_obbox_aabox_hit_count: i32,
    pub collision_obbox_obbox_count: i32,
    pub collision_obbox_obbox_hit_count: i32,
}

impl ColmathStatsStruct {
    /// `ColmathStatsStruct::ColmathStatsStruct()`: `Reset()`.
    pub fn new() -> Self {
        let mut s = ColmathStatsStruct::default();
        s.reset();
        s
    }

    /// `ColmathStatsStruct::Reset` (colmath.cpp:46).
    pub fn reset(&mut self) {
        self.total_collision_count = 0;
        self.total_collision_hit_count = 0;
        self.collision_ray_tri_count = 0;
        self.collision_ray_tri_hit_count = 0;
        self.collision_aabox_tri_count = 0;
        self.collision_aabox_tri_hit_count = 0;
        self.collision_aabox_aabox_count = 0;
        self.collision_aabox_aabox_hit_count = 0;
        self.collision_obbox_tri_count = 0;
        self.collision_obbox_tri_hit_count = 0;
        self.collision_obbox_aabox_count = 0;
        self.collision_obbox_aabox_hit_count = 0;
        self.collision_obbox_obbox_count = 0;
        self.collision_obbox_obbox_hit_count = 0;
    }
}
