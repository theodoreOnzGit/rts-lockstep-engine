//! UNTESTED DRAFT (#202, #209): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/cardinalspline.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/cardinalspline.cpp
//!
//! Defect carried over from upstream, not fixed: for fewer than two keys the 1D `Update_Tangents` falls through into
//! indexing `Keys[end-1]` (undefined behaviour in C++); here it returns after zeroing, as the 3D and Catmull-Rom 1D
//! versions do. Also, upstream's `CardinalSpline1DClass::Add_Key(float,float)` hides the base's three-argument one.

use super::chunk_save::ChunkSaveClass;
use super::curve::read_f32;
use super::hermite_spline::{HermiteSpline1DClass, HermiteSpline3DClass};
use super::saveload_ids::*;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::vector3::Vector3;

/// ID's used by CardinalSpline3D (cardinalspline.cpp:44).
pub const CARDINAL3D_CHUNK_HERMITE3D: u32 = 0x0207_0957;
pub const CARDINAL3D_CHUNK_TIGHTNESSKEYS: u32 = 0x0207_0958;
/// ID's used by CardinalSpline1D.
pub const CARDINAL1D_CHUNK_HERMITE1D: u32 = 0x0207_0959;
pub const CARDINAL1D_CHUNK_TIGHTNESSKEYS: u32 = 0x0207_095A;

/// `class CardinalSpline3DClass : public HermiteSpline3DClass`.
#[derive(Clone, Default)]
pub struct CardinalSpline3DClass {
    pub base: HermiteSpline3DClass,
    pub tightness: Vec<f32>,
}

impl CardinalSpline3DClass {
    /// `CardinalSpline3DClass::Add_Key`.
    pub fn add_key(&mut self, point: &Vector3, t: f32) -> i32 {
        let index = self.base.add_key(point, t);
        let tightness = 0.5f32;
        self.tightness.insert(index as usize, tightness);
        index
    }

    /// `CardinalSpline3DClass::Remove_Key`.
    pub fn remove_key(&mut self, i: i32) {
        self.tightness.remove(i as usize);
        self.base.remove_key(i);
    }

    /// `CardinalSpline3DClass::Clear_Keys`.
    pub fn clear_keys(&mut self) {
        self.tightness.clear();
        self.base.clear_keys();
    }

    /// `CardinalSpline3DClass::Set_Tightness`.
    pub fn set_tightness(&mut self, i: i32, tightness: f32) {
        debug_assert!(i >= 0);
        debug_assert!((i as usize) < self.tightness.len());
        self.tightness[i as usize] = tightness;
        self.base.tangents_dirty = true;
    }

    /// `CardinalSpline3DClass::Get_Tightness`.
    pub fn get_tightness(&self, i: i32) -> f32 {
        self.tightness[i as usize]
    }

    /// `CardinalSpline3DClass::Update_Tangents` (cardinalspline.cpp:112).
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
        let tn = &self.tightness;
        // First and Last Key:
        // Only need to compute the OutTangent for key[0] and the InTangent for key[end]
        let end = n - 1;
        tan[0].in_tangent.set(0.0, 0.0, 0.0);
        tan[end].out_tangent.set(0.0, 0.0, 0.0);

        if self.base.base.is_looping {
            // This really only works if the start and end points have the same position...
            tan[0].out_tangent.x = (1.0 - tn[0]) * (keys[1].point.x - keys[end - 1].point.x);
            tan[0].out_tangent.y = (1.0 - tn[0]) * (keys[1].point.y - keys[end - 1].point.y);
            tan[0].out_tangent.z = (1.0 - tn[0]) * (keys[1].point.z - keys[end - 1].point.z);
            tan[end].in_tangent = tan[0].out_tangent;
        } else {
            tan[0].out_tangent.x = (1.0 - tn[0]) * (keys[1].point.x - keys[0].point.x);
            tan[0].out_tangent.y = (1.0 - tn[0]) * (keys[1].point.y - keys[0].point.y);
            tan[0].out_tangent.z = (1.0 - tn[0]) * (keys[1].point.z - keys[0].point.z);
            tan[end].in_tangent.x = (1.0 - tn[0]) * (keys[end].point.x - keys[end - 1].point.x);
            tan[end].in_tangent.y = (1.0 - tn[0]) * (keys[end].point.y - keys[end - 1].point.y);
            tan[end].in_tangent.z = (1.0 - tn[0]) * (keys[end].point.z - keys[end - 1].point.z);
        }

        let total_time = (keys[1].time - keys[0].time) + (keys[end].time - keys[end - 1].time);
        let in_factor = 2.0 * (keys[end].time - keys[end - 1].time) / total_time;
        let out_factor = 2.0 * (keys[1].time - keys[0].time) / total_time;
        tan[end].in_tangent *= in_factor;
        tan[0].out_tangent *= out_factor;

        // inner knots
        for i in 1..n - 1 {
            tan[i].in_tangent.x = (1.0 - tn[i]) * (keys[i + 1].point.x - keys[i - 1].point.x);
            tan[i].in_tangent.y = (1.0 - tn[i]) * (keys[i + 1].point.y - keys[i - 1].point.y);
            tan[i].in_tangent.z = (1.0 - tn[i]) * (keys[i + 1].point.z - keys[i - 1].point.z);
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
        if self.base.tangents_dirty {
            self.update_tangents();
        }
        self.base.evaluate(time)
    }

    /// `PersistClass::Get_Factory` -> `_CardinalSpline3DFactory`'s chunk id.
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_CARDINALSPLINE3D
    }

    /// `CardinalSpline3DClass::Save` (cardinalspline.cpp:197).
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.begin_chunk(CARDINAL3D_CHUNK_HERMITE3D);
        self.base.save(csave);
        csave.end_chunk();

        csave.begin_chunk(CARDINAL3D_CHUNK_TIGHTNESSKEYS);
        for t in &self.tightness {
            csave.write_floats(&[*t]);
        }
        csave.end_chunk();
        true
    }

    /// `CardinalSpline3DClass::Load` (cardinalspline.cpp:217).
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        self.tightness.clear();
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                CARDINAL3D_CHUNK_HERMITE3D => {
                    self.base.load(cload);
                }
                CARDINAL3D_CHUNK_TIGHTNESSKEYS => {
                    for _ in 0..self.base.base.keys.len() {
                        self.tightness.push(read_f32(cload));
                    }
                }
                _ => {}
            }
            let _ = cload.close_chunk();
        }
        true
    }
}

/// `class CardinalSpline1DClass : public HermiteSpline1DClass`.
#[derive(Clone, Default)]
pub struct CardinalSpline1DClass {
    pub base: HermiteSpline1DClass,
    pub tightness: Vec<f32>,
}

impl CardinalSpline1DClass {
    /// `CardinalSpline1DClass::Add_Key(float point, float t)`: `extra` is the base default 0.
    pub fn add_key(&mut self, point: f32, t: f32) -> i32 {
        let index = self.base.add_key(point, t, 0);
        let tightness = 0.5f32;
        self.tightness.insert(index as usize, tightness);
        index
    }

    /// `CardinalSpline1DClass::Remove_Key`.
    pub fn remove_key(&mut self, i: i32) {
        self.tightness.remove(i as usize);
        self.base.remove_key(i);
    }

    /// `CardinalSpline1DClass::Clear_Keys`.
    pub fn clear_keys(&mut self) {
        self.tightness.clear();
        self.base.clear_keys();
    }

    /// `CardinalSpline1DClass::Set_Tightness`.
    pub fn set_tightness(&mut self, i: i32, tightness: f32) {
        debug_assert!(i >= 0);
        debug_assert!((i as usize) < self.tightness.len());
        self.tightness[i as usize] = tightness;
        self.base.tangents_dirty = true;
    }

    /// `CardinalSpline1DClass::Get_Tightness`.
    pub fn get_tightness(&self, i: i32) -> f32 {
        self.tightness[i as usize]
    }

    /// `CardinalSpline1DClass::Update_Tangents` (cardinalspline.cpp:300).
    pub fn update_tangents(&mut self) {
        let n = self.base.base.keys.len();
        if n < 2 {
            for _ in 0..n {
                self.base.tangents[0].in_tangent = 0.0;
                self.base.tangents[0].out_tangent = 0.0;
            }
            // upstream has no `return` here and falls into out-of-range reads; see the module note.
            return;
        }
        let keys = &self.base.base.keys;
        let tan = &mut self.base.tangents;
        let tn = &self.tightness;
        let end = n - 1;
        tan[0].in_tangent = 0.0;
        tan[end].out_tangent = 0.0;

        if self.base.base.is_looping {
            tan[0].out_tangent = (1.0 - tn[0]) * (keys[1].point - keys[end - 1].point);
            tan[end].in_tangent = tan[0].out_tangent;
        } else {
            tan[0].out_tangent = (1.0 - tn[0]) * (keys[1].point - keys[0].point);
            tan[end].in_tangent = (1.0 - tn[0]) * (keys[end].point - keys[end - 1].point);
        }

        let total_time = (keys[1].time - keys[0].time) + (keys[end].time - keys[end - 1].time);
        let in_factor = 2.0 * (keys[end].time - keys[end - 1].time) / total_time;
        let out_factor = 2.0 * (keys[1].time - keys[0].time) / total_time;
        tan[end].in_tangent *= in_factor;
        tan[0].out_tangent *= out_factor;

        for i in 1..n - 1 {
            tan[i].in_tangent = (1.0 - tn[i]) * (keys[i + 1].point - keys[i - 1].point);
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
        if self.base.tangents_dirty && self.base.base.keys.len() != 1 {
            self.update_tangents();
        }
        self.base.evaluate(time)
    }

    /// `PersistClass::Get_Factory` -> `_CardinalSpline1DFactory`'s chunk id.
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_CARDINALSPLINE1D
    }

    /// `CardinalSpline1DClass::Save` (the base's `Update_Tangents` call resolves to this class's).
    pub fn save(&mut self, csave: &mut ChunkSaveClass) -> bool {
        if self.base.tangents_dirty {
            self.update_tangents();
        }
        csave.begin_chunk(CARDINAL1D_CHUNK_HERMITE1D);
        self.base.save(csave);
        csave.end_chunk();

        csave.begin_chunk(CARDINAL1D_CHUNK_TIGHTNESSKEYS);
        for t in &self.tightness {
            csave.write_floats(&[*t]);
        }
        csave.end_chunk();
        true
    }

    /// `CardinalSpline1DClass::Load`.
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        self.tightness.clear();
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                CARDINAL1D_CHUNK_HERMITE1D => {
                    self.base.load(cload);
                }
                CARDINAL1D_CHUNK_TIGHTNESSKEYS => {
                    for _ in 0..self.base.base.keys.len() {
                        self.tightness.push(read_f32(cload));
                    }
                }
                _ => {}
            }
            let _ = cload.close_chunk();
        }
        true
    }
}
