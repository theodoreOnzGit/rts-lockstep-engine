//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//! PARTLY DIFFERENTIAL-TESTED (#215): Save and Load are not covered; Update_Tangents with fewer than two keys is undefined upstream (3D) and untested; the rest matches (tests/differential_draft_curves.rs).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/catmullromspline.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/catmullromspline.cpp
//!
//! Upstream's 3D `Update_Tangents` has no `return` for fewer than two keys (falls into out-of-range reads, UB); this
//! draft returns after zeroing, as the 1D version does.

use crate::logic::modules::plant;

use super::chunk_save::ChunkSaveClass;
use super::hermite_spline::{HermiteSpline1DClass, HermiteSpline3DClass};
use super::saveload_ids::*;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::vector3::Vector3;

/// ID's used by CatmullRomSpline3D (catmullromspline.cpp:49; the same values as the hermite ones).
pub const CATMULLROM3D_CHUNK_HERMITE3D: u32 = 0x0002_0727;
/// ID's used by CatmullRomSpline1D.
pub const CATMULLROM1D_CHUNK_HERMITE1D: u32 = 0x0002_0729;

/// `class CatmullRomSpline3DClass : public HermiteSpline3DClass`.
#[derive(Clone, Default)]
pub struct CatmullRomSpline3DClass {
    pub base: HermiteSpline3DClass,
}

impl CatmullRomSpline3DClass {
    /// `CatmullRomSpline3DClass::Update_Tangents` (catmullromspline.cpp:52).
    pub fn update_tangents(&mut self) {
        let n = self.base.base.keys.len();
        if n < 2 {
            for _ in 0..n {
                self.base.tangents[0].in_tangent.set(0.0, 0.0, 0.0);
                self.base.tangents[0].out_tangent.set(0.0, 0.0, 0.0);
            }
            return;
        }
        let keys = &self.base.base.keys;
        let tan = &mut self.base.tangents;
        // first and last knot
        let end = n - 1;
        tan[0].in_tangent.set(0.0, 0.0, 0.0);
        tan[end].out_tangent.set(0.0, 0.0, 0.0);

        if self.base.base.is_looping {
            // This really only works if the start and end points have the same position...
            tan[0].out_tangent.x = 0.5 * (keys[1].point.x - keys[end - 1].point.x);
            tan[0].out_tangent.y = 0.5 * (keys[1].point.y - keys[end - 1].point.y);
            tan[0].out_tangent.z = 0.5 * (keys[1].point.z - keys[end - 1].point.z);
            tan[end].in_tangent = tan[0].out_tangent;
        } else {
            // TODO (upstream): second derivative = 0... what is formula?  I'm making this up...
            tan[0].out_tangent.x = (if plant(10746) { 0.5 } else { 0.25 }) * (keys[1].point.x - keys[0].point.x);
            tan[0].out_tangent.y = 0.25 * (keys[1].point.y - keys[0].point.y);
            tan[0].out_tangent.z = 0.25 * (keys[1].point.z - keys[0].point.z);
            tan[end].in_tangent.x = 0.25 * (keys[end].point.x - keys[end - 1].point.x);
            tan[end].in_tangent.y = 0.25 * (keys[end].point.y - keys[end - 1].point.y);
            tan[end].in_tangent.z = 0.25 * (keys[end].point.z - keys[end - 1].point.z);
        }

        let total_time = (keys[1].time - keys[0].time) + if plant(10748) { 0.0 } else { keys[end].time - keys[end - 1].time };
        let in_factor = 2.0 * (keys[end].time - keys[end - 1].time) / total_time;
        let out_factor = 2.0 * (keys[1].time - keys[0].time) / total_time;
        tan[end].in_tangent *= in_factor;
        tan[0].out_tangent *= out_factor;

        // inner knots
        for i in 1..n - 1 {
            tan[i].in_tangent.x = (if plant(10747) { 0.25 } else { 0.5 }) * (keys[i + 1].point.x - keys[i - 1].point.x);
            tan[i].in_tangent.y = 0.5 * (keys[i + 1].point.y - keys[i - 1].point.y);
            tan[i].in_tangent.z = 0.5 * (keys[i + 1].point.z - keys[i - 1].point.z);
            tan[i].out_tangent = tan[i].in_tangent;

            let in_factor = 2.0 * (keys[i].time - keys[i - 1].time) / (keys[i + 1].time - keys[i - 1].time);
            let out_factor = 2.0 * (keys[i + 1].time - keys[i].time) / (keys[i + 1].time - keys[i - 1].time);
            tan[i].in_tangent *= in_factor; // compensating for the un-even keys
            tan[i].out_tangent *= out_factor;
        }
        self.base.tangents_dirty = false;
    }

    /// `HermiteSpline3DClass::Evaluate` with the virtual `Update_Tangents` resolved to this class's.
    pub fn evaluate(&mut self, time: f32) -> Vector3 {
        if let Some(v) = self.base.evaluate_out_of_range(time) {
            return v;
        }
        if self.base.tangents_dirty {
            self.update_tangents();
        }
        self.base.interpolate(time)
    }

    /// `HermiteSpline3DClass::Evaluate_Derivative` with the virtual `Update_Tangents` resolved to this class's.
    pub fn evaluate_derivative(&mut self, time: f32) -> Vector3 {
        let time = self.base.clamp_time(time);
        if self.base.tangents_dirty {
            self.update_tangents();
        }
        self.base.derivative_at(time)
    }

    /// `PersistClass::Get_Factory` -> `_CatmullRomSpline3DFactory`'s chunk id.
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_CATMULLROMSPLINE3D
    }

    /// `CatmullRomSpline3DClass::Save`.
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.begin_chunk(CATMULLROM3D_CHUNK_HERMITE3D);
        self.base.save(csave);
        csave.end_chunk();
        true
    }

    /// `CatmullRomSpline3DClass::Load`.
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        while cload.open_chunk().unwrap_or(false) {
            if cload.cur_chunk_id().unwrap_or(0) == CATMULLROM3D_CHUNK_HERMITE3D {
                self.base.load(cload);
            }
            let _ = cload.close_chunk();
        }
        true
    }
}

/// `class CatmullRomSpline1DClass : public HermiteSpline1DClass`.
#[derive(Clone, Default)]
pub struct CatmullRomSpline1DClass {
    pub base: HermiteSpline1DClass,
}

impl CatmullRomSpline1DClass {
    /// `CatmullRomSpline1DClass::Update_Tangents` (catmullromspline.cpp:176).
    pub fn update_tangents(&mut self) {
        let n = self.base.base.keys.len();
        if n < 2 {
            for i in 0..n {
                self.base.tangents[i].in_tangent = 0.0;
                self.base.tangents[i].out_tangent = 0.0;
            }
            return;
        }
        let keys = &self.base.base.keys;
        let tan = &mut self.base.tangents;
        let end = n - 1;
        tan[0].in_tangent = 0.0;
        tan[end].out_tangent = 0.0;

        if self.base.base.is_looping {
            tan[0].out_tangent = 0.5 * (keys[1].point - keys[end - 1].point);
            tan[end].in_tangent = tan[0].out_tangent;
        } else {
            tan[0].out_tangent = 0.25 * (keys[1].point - keys[0].point);
            tan[end].in_tangent = 0.25 * (keys[end].point - keys[end - 1].point);
        }

        let total_time = (keys[1].time - keys[0].time) + (keys[end].time - keys[end - 1].time);
        let in_factor = 2.0 * (keys[end].time - keys[end - 1].time) / total_time;
        let out_factor = 2.0 * (keys[1].time - keys[0].time) / total_time;
        tan[end].in_tangent *= in_factor;
        tan[0].out_tangent *= out_factor;

        for i in 1..n - 1 {
            tan[i].in_tangent = 0.5 * (keys[i + 1].point - keys[i - 1].point);
            tan[i].out_tangent = tan[i].in_tangent;
            let in_factor = 2.0 * (keys[i].time - keys[i - 1].time) / (keys[i + 1].time - keys[i - 1].time);
            let out_factor = 2.0 * (keys[i + 1].time - keys[i].time) / (keys[i + 1].time - keys[i - 1].time);
            tan[i].in_tangent *= in_factor;
            tan[i].out_tangent *= out_factor;
        }
        self.base.tangents_dirty = false;
    }

    /// `HermiteSpline1DClass::Evaluate` with the virtual `Update_Tangents` resolved to this class's.
    pub fn evaluate(&mut self, time: f32) -> f32 {
        if let Some(v) = self.base.evaluate_early(time) {
            return v;
        }
        if self.base.tangents_dirty && !plant(10749) {
            self.update_tangents();
        }
        self.base.interpolate(time)
    }

    /// `PersistClass::Get_Factory` -> `_CatmullRomSpline1DFactory`'s chunk id.
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_CATMULLROMSPLINE1D
    }

    /// `CatmullRomSpline1DClass::Save`.
    pub fn save(&mut self, csave: &mut ChunkSaveClass) -> bool {
        if self.base.tangents_dirty {
            self.update_tangents();
        }
        csave.begin_chunk(CATMULLROM1D_CHUNK_HERMITE1D);
        self.base.save(csave);
        csave.end_chunk();
        true
    }

    /// `CatmullRomSpline1DClass::Load`.
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        while cload.open_chunk().unwrap_or(false) {
            if cload.cur_chunk_id().unwrap_or(0) == CATMULLROM1D_CHUNK_HERMITE1D {
                self.base.load(cload);
            }
            let _ = cload.close_chunk();
        }
        true
    }
}
