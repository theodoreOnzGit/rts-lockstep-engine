//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/hermitespline.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/hermitespline.cpp
//!
//! `virtual Update_Tangents` is a hook: the derived splines (cardinal, Catmull-Rom, TCB) own a `base` hermite spline
//! and, before calling the base `evaluate`, run their own `update_tangents` when `base.tangents_dirty` is set (which
//! clears it, as upstream's overrides do). The base `update_tangents` only clears the flag, as upstream's inline does.

use super::chunk_save::ChunkSaveClass;
use super::curve::{read_f32, Curve1DClass, Curve3DClass};
use super::saveload_ids::*;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::vector3::Vector3;

/// ID's used by HermiteSpline3D (hermitespline.cpp:48).
pub const HERMITE3D_CHUNK_CURVE3D: u32 = 0x0002_0727;
pub const HERMITE3D_CHUNK_TANGENTS: u32 = 0x0002_0728;
/// ID's used by HermiteSpline1D.
pub const HERMITE1D_CHUNK_CURVE1D: u32 = 0x0002_0729;
pub const HERMITE1D_CHUNK_TANGENTS: u32 = 0x0002_072A;

/// `HermiteSpline3DClass::TangentsClass`.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Tangents3D {
    pub in_tangent: Vector3,
    pub out_tangent: Vector3,
}

/// `class HermiteSpline3DClass : public Curve3DClass`.
#[derive(Clone)]
pub struct HermiteSpline3DClass {
    pub base: Curve3DClass,
    pub tangents_dirty: bool,
    pub tangents: Vec<Tangents3D>,
}

impl Default for HermiteSpline3DClass {
    fn default() -> Self {
        Self::new()
    }
}

impl HermiteSpline3DClass {
    /// `HermiteSpline3DClass()`: `TangentsDirty(true)`.
    pub fn new() -> Self {
        HermiteSpline3DClass { base: Curve3DClass::new(), tangents_dirty: true, tangents: Vec::new() }
    }

    /// `Update_Tangents` (inline): `TangentsDirty = false`.
    pub fn update_tangents(&mut self) {
        self.tangents_dirty = false;
    }

    /// `HermiteSpline3DClass::Set_Looping` (hermitespline.cpp:84).
    pub fn set_looping(&mut self, onoff: bool) {
        if onoff != self.base.is_looping {
            self.base.set_looping(onoff);
            self.tangents_dirty = true;
        }
    }

    /// `HermiteSpline3DClass::Evaluate` (hermitespline.cpp:95).
    pub fn evaluate(&mut self, time: f32) -> Vector3 {
        let keys = &self.base.keys;
        // if we're outside the range, return the start or end...
        if time < keys[0].time {
            return keys[0].point;
        }
        if time > keys[keys.len() - 1].time {
            return keys[keys.len() - 1].point;
        }
        // if the tangents are marked dirty, give derived classes a chance to recompute them
        if self.tangents_dirty {
            self.update_tangents();
        }
        let keys = &self.base.keys;
        let (i0, i1, t) = self.base.find_interval(time);
        let (i0, i1) = (i0 as usize, i1 as usize);
        let t2 = t * t;
        let t3 = t2 * t;
        // hermite basis functions:
        let h0 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h1 = -2.0 * t3 + 3.0 * t2;
        let h2 = t3 - 2.0 * t2 + t;
        let h3 = t3 - t2;
        Vector3::new(
            h0 * keys[i0].point.x + h1 * keys[i1].point.x + h2 * self.tangents[i0].out_tangent.x + h3 * self.tangents[i1].in_tangent.x,
            h0 * keys[i0].point.y + h1 * keys[i1].point.y + h2 * self.tangents[i0].out_tangent.y + h3 * self.tangents[i1].in_tangent.y,
            h0 * keys[i0].point.z + h1 * keys[i1].point.z + h2 * self.tangents[i0].out_tangent.z + h3 * self.tangents[i1].in_tangent.z,
        )
    }

    /// `HermiteSpline3DClass::Evaluate_Derivative` (hermitespline.cpp:140).
    pub fn evaluate_derivative(&mut self, time: f32) -> Vector3 {
        // if we're outside the range, return the value for the start or end...
        let min_time = self.base.keys[0].time;
        let max_time = self.base.keys[self.base.keys.len() - 1].time;
        let mut time = time;
        time = if time > min_time { time } else { min_time }; // MAX(time, min_time)
        time = if time < max_time { time } else { max_time }; // MIN(time, max_time)
        if self.tangents_dirty {
            self.update_tangents();
        }
        let keys = &self.base.keys;
        let (i0, i1, t) = self.base.find_interval(time);
        let (i0, i1) = (i0 as usize, i1 as usize);
        let t2 = t * t;
        // derivatives of hermite basis functions:
        let dh0 = 6.0 * t2 - 6.0 * t;
        let dh1 = -6.0 * t2 + 6.0 * t;
        let dh2 = 3.0 * t2 - 4.0 * t + 1.0;
        let dh3 = 3.0 * t2 - 2.0 * t;
        Vector3::new(
            dh0 * keys[i0].point.x + dh1 * keys[i1].point.x + dh2 * self.tangents[i0].out_tangent.x + dh3 * self.tangents[i1].in_tangent.x,
            dh0 * keys[i0].point.y + dh1 * keys[i1].point.y + dh2 * self.tangents[i0].out_tangent.y + dh3 * self.tangents[i1].in_tangent.y,
            dh0 * keys[i0].point.z + dh1 * keys[i1].point.z + dh2 * self.tangents[i0].out_tangent.z + dh3 * self.tangents[i1].in_tangent.z,
        )
    }

    /// `HermiteSpline3DClass::Set_Key` (hermitespline.cpp:179).
    pub fn set_key(&mut self, i: i32, point: &Vector3) {
        self.base.set_key(i, point);
        self.tangents_dirty = true;
    }

    /// `HermiteSpline3DClass::Add_Key` (hermitespline.cpp:185).
    pub fn add_key(&mut self, point: &Vector3, t: f32) -> i32 {
        let index = self.base.add_key(point, t);
        self.tangents_dirty = true;
        self.tangents.insert(index as usize, Tangents3D::default());
        index
    }

    /// `HermiteSpline3DClass::Remove_Key` (hermitespline.cpp:198).
    pub fn remove_key(&mut self, i: i32) {
        self.tangents.remove(i as usize);
        self.base.remove_key(i);
        self.tangents_dirty = true;
    }

    /// `HermiteSpline3DClass::Clear_Keys`.
    pub fn clear_keys(&mut self) {
        self.tangents.clear();
        self.base.clear_keys();
        self.tangents_dirty = true;
    }

    /// `HermiteSpline3DClass::Set_Tangents`.
    pub fn set_tangents(&mut self, i: i32, in_tan: &Vector3, out_tan: &Vector3) {
        debug_assert!(i >= 0);
        debug_assert!(i < self.base.key_count());
        self.tangents[i as usize].in_tangent = *in_tan;
        self.tangents[i as usize].out_tangent = *out_tan;
    }

    /// `HermiteSpline3DClass::Get_Tangents`: (in, out).
    pub fn get_tangents(&self, i: i32) -> (Vector3, Vector3) {
        debug_assert!(i >= 0);
        debug_assert!(i < self.base.key_count());
        (self.tangents[i as usize].in_tangent, self.tangents[i as usize].out_tangent)
    }

    /// `PersistClass::Get_Factory` -> `_HermiteSpline3DFactory`'s chunk id.
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_HERMITESPLINE3D
    }

    /// `HermiteSpline3DClass::Save` (hermitespline.cpp:241).
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.begin_chunk(HERMITE3D_CHUNK_CURVE3D);
        self.base.save(csave);
        csave.end_chunk();

        csave.begin_chunk(HERMITE3D_CHUNK_TANGENTS);
        for t in &self.tangents {
            csave.write_floats(&[t.in_tangent.x, t.in_tangent.y, t.in_tangent.z]);
            csave.write_floats(&[t.out_tangent.x, t.out_tangent.y, t.out_tangent.z]);
        }
        csave.end_chunk();
        true
    }

    /// `HermiteSpline3DClass::Load` (hermitespline.cpp:262).
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        self.tangents.clear();
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                HERMITE3D_CHUNK_CURVE3D => {
                    self.base.load(cload);
                }
                HERMITE3D_CHUNK_TANGENTS => {
                    for _ in 0..self.base.keys.len() {
                        let mut t = Tangents3D::default();
                        t.in_tangent = Vector3::new(read_f32(cload), read_f32(cload), read_f32(cload));
                        t.out_tangent = Vector3::new(read_f32(cload), read_f32(cload), read_f32(cload));
                        self.tangents.push(t);
                    }
                }
                _ => {}
            }
            let _ = cload.close_chunk();
        }
        debug_assert!(self.base.keys.len() == self.tangents.len());
        true
    }
}

/// `HermiteSpline1DClass::TangentsClass`.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Tangents1D {
    pub in_tangent: f32,
    pub out_tangent: f32,
}

/// `class HermiteSpline1DClass : public Curve1DClass`.
#[derive(Clone)]
pub struct HermiteSpline1DClass {
    pub base: Curve1DClass,
    pub tangents_dirty: bool,
    pub tangents: Vec<Tangents1D>,
}

impl Default for HermiteSpline1DClass {
    fn default() -> Self {
        Self::new()
    }
}

impl HermiteSpline1DClass {
    /// `HermiteSpline1DClass()`: `TangentsDirty(true)`.
    pub fn new() -> Self {
        HermiteSpline1DClass { base: Curve1DClass::new(), tangents_dirty: true, tangents: Vec::new() }
    }

    /// `Update_Tangents` (inline).
    pub fn update_tangents(&mut self) {
        self.tangents_dirty = false;
    }

    /// `HermiteSpline1DClass::Set_Looping` (hermitespline.cpp:317).
    pub fn set_looping(&mut self, onoff: bool) {
        if onoff != self.base.is_looping {
            self.base.set_looping(onoff);
            self.tangents_dirty = true;
        }
    }

    /// `HermiteSpline1DClass::Evaluate` (hermitespline.cpp:327).
    pub fn evaluate(&mut self, time: f32) -> f32 {
        let n = self.base.keys.len();
        if n == 1 {
            return self.base.keys[0].point;
        }
        if !self.base.is_looping {
            // if we're outside the range, return the start or end...
            if time < self.base.keys[0].time {
                return self.base.keys[0].point;
            }
            if time > self.base.keys[n - 1].time {
                return self.base.keys[n - 1].point;
            }
        }
        if self.tangents_dirty {
            self.update_tangents();
        }
        let keys = &self.base.keys;
        let (i0, i1, t) = self.base.find_interval(time);
        let (i0, i1) = (i0 as usize, i1 as usize);
        let t2 = t * t;
        let t3 = t2 * t;
        let h0 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h1 = -2.0 * t3 + 3.0 * t2;
        let h2 = t3 - 2.0 * t2 + t;
        let h3 = t3 - t2;
        h0 * keys[i0].point + h1 * keys[i1].point + h2 * self.tangents[i0].out_tangent + h3 * self.tangents[i1].in_tangent
    }

    /// `HermiteSpline1DClass::Set_Key`.
    pub fn set_key(&mut self, i: i32, point: f32, extra: u32) {
        self.base.set_key(i, point, extra);
        self.tangents_dirty = true;
    }

    /// `HermiteSpline1DClass::Add_Key`.
    pub fn add_key(&mut self, point: f32, t: f32, extra: u32) -> i32 {
        let index = self.base.add_key(point, t, extra);
        self.tangents_dirty = true;
        self.tangents.insert(index as usize, Tangents1D::default());
        index
    }

    /// `HermiteSpline1DClass::Remove_Key`.
    pub fn remove_key(&mut self, i: i32) {
        self.tangents.remove(i as usize);
        self.base.remove_key(i);
        self.tangents_dirty = true;
    }

    /// `HermiteSpline1DClass::Clear_Keys`.
    pub fn clear_keys(&mut self) {
        self.tangents.clear();
        self.base.clear_keys();
        self.tangents_dirty = true;
    }

    /// `HermiteSpline1DClass::Set_Tangents`.
    pub fn set_tangents(&mut self, i: i32, in_tan: f32, out_tan: f32) {
        debug_assert!(i >= 0);
        debug_assert!(i < self.base.key_count());
        self.tangents[i as usize].in_tangent = in_tan;
        self.tangents[i as usize].out_tangent = out_tan;
    }

    /// `HermiteSpline1DClass::Get_Tangents`: (in, out).
    pub fn get_tangents(&self, i: i32) -> (f32, f32) {
        debug_assert!(i >= 0);
        debug_assert!(i < self.base.key_count());
        (self.tangents[i as usize].in_tangent, self.tangents[i as usize].out_tangent)
    }

    /// `PersistClass::Get_Factory` -> `_HermiteSpline1DFactory`'s chunk id.
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_HERMITESPLINE1D
    }

    /// `HermiteSpline1DClass::Save`: runs `Update_Tangents` first when dirty (the derived override when there is one,
    /// see the module note: derived classes call their own update before this).
    pub fn save(&mut self, csave: &mut ChunkSaveClass) -> bool {
        if self.tangents_dirty {
            self.update_tangents();
        }
        csave.begin_chunk(HERMITE1D_CHUNK_CURVE1D);
        self.base.save(csave);
        csave.end_chunk();

        csave.begin_chunk(HERMITE1D_CHUNK_TANGENTS);
        for t in &self.tangents {
            csave.write_floats(&[t.in_tangent]);
            csave.write_floats(&[t.out_tangent]);
        }
        csave.end_chunk();
        true
    }

    /// `HermiteSpline1DClass::Load`.
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        self.tangents.clear();
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                HERMITE1D_CHUNK_CURVE1D => {
                    self.base.load(cload);
                }
                HERMITE1D_CHUNK_TANGENTS => {
                    for _ in 0..self.base.keys.len() {
                        let in_tangent = read_f32(cload);
                        let out_tangent = read_f32(cload);
                        self.tangents.push(Tangents1D { in_tangent, out_tangent });
                    }
                    self.tangents_dirty = false;
                }
                _ => {}
            }
            let _ = cload.close_chunk();
        }
        debug_assert!(self.base.keys.len() == self.tangents.len());
        true
    }
}
