//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/curve.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/curve.cpp
//!
//! `PersistClass` / `PersistFactoryClass` (WWSaveLoad) are represented by `get_factory_id` (the factory's chunk id,
//! `WWMATH_CHUNKID_LINEARCURVE3D` etc.; see `wwsaveload`'s draft for the registry); `DynamicVectorClass<KeyClass>` by `Vec`.

use super::chunk_save::ChunkSaveClass;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::vector3::Vector3;

use super::saveload_ids::*;

/// ID's used by Curve3D (curve.cpp:46).
pub const CURVE3D_CHUNK_VARIABLES: u32 = 0x0002_0651;
pub const CURVE3D_CHUNK_KEYS: u32 = 0x0002_0652;
pub const CURVE3D_VARIABLE_ISLOOPING: u32 = 0x00;
pub const CURVE3D_VARIABLE_KEYCOUNT: u32 = 0x01;
/// ID's used by LinearCurve3D.
pub const LINEARCURVE3D_CHUNK_CURVE3D: u32 = 0x0002_0653;
/// ID's used by Curve1D.
pub const CURVE1D_CHUNK_VARIABLES: u32 = 0x0002_0655;
pub const CURVE1D_CHUNK_KEYS: u32 = 0x0002_0656;
pub const CURVE1D_VARIABLE_ISLOOPING: u32 = 0x00;
pub const CURVE1D_VARIABLE_KEYCOUNT: u32 = 0x01;
/// ID's used by LinearCurve1D.
pub const LINEARCURVE1D_CHUNK_CURVE1D: u32 = 0x0002_0657;

/// `Curve3DClass::KeyClass`.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Curve3DKey {
    pub point: Vector3,
    pub time: f32,
}

pub fn read_f32(cload: &mut ChunkLoad) -> f32 {
    let mut b = [0u8; 4];
    let _ = cload.read_into(&mut b);
    f32::from_le_bytes(b)
}

pub fn read_u32(cload: &mut ChunkLoad) -> u32 {
    let mut b = [0u8; 4];
    let _ = cload.read_into(&mut b);
    u32::from_le_bytes(b)
}

/// `class Curve3DClass` (curve.h): the base state and behaviour of the 3D curves.
#[derive(Clone, Default)]
pub struct Curve3DClass {
    pub is_looping: bool,
    pub keys: Vec<Curve3DKey>,
}

impl Curve3DClass {
    /// `Curve3DClass::Curve3DClass` (curve.cpp:78).
    pub fn new() -> Self {
        Curve3DClass { is_looping: false, keys: Vec::new() }
    }

    /// `Curve3DClass::Is_Looping`.
    pub fn is_looping(&self) -> bool {
        self.is_looping
    }

    /// `Curve3DClass::Set_Looping`.
    pub fn set_looping(&mut self, onoff: bool) {
        self.is_looping = onoff;
    }

    /// `Curve3DClass::Get_Start_Time`.
    pub fn get_start_time(&self) -> f32 {
        if !self.keys.is_empty() {
            self.keys[0].time
        } else {
            0.0
        }
    }

    /// `Curve3DClass::Get_End_Time`.
    pub fn get_end_time(&self) -> f32 {
        if !self.keys.is_empty() {
            self.keys[self.keys.len() - 1].time
        } else {
            0.0
        }
    }

    /// `Curve3DClass::Key_Count`.
    pub fn key_count(&self) -> i32 {
        self.keys.len() as i32
    }

    /// `Curve3DClass::Get_Key`: (point, time).
    pub fn get_key(&self, i: i32) -> (Vector3, f32) {
        debug_assert!(i >= 0);
        debug_assert!((i as usize) < self.keys.len());
        (self.keys[i as usize].point, self.keys[i as usize].time)
    }

    /// `Curve3DClass::Set_Key`.
    pub fn set_key(&mut self, i: i32, point: &Vector3) {
        debug_assert!(i >= 0);
        debug_assert!((i as usize) < self.keys.len());
        self.keys[i as usize].point = *point;
    }

    /// `Curve3DClass::Add_Key`: inserted in time order.
    pub fn add_key(&mut self, point: &Vector3, t: f32) -> i32 {
        let mut idx = 0usize;
        while idx < self.keys.len() && self.keys[idx].time < t {
            idx += 1;
        }
        self.keys.insert(idx, Curve3DKey { point: *point, time: t });
        idx as i32
    }

    /// `Curve3DClass::Remove_Key`.
    pub fn remove_key(&mut self, i: i32) {
        debug_assert!(i >= 0);
        debug_assert!((i as usize) < self.keys.len());
        self.keys.remove(i as usize);
    }

    /// `Curve3DClass::Clear_Keys`.
    pub fn clear_keys(&mut self) {
        self.keys.clear();
    }

    /// `Curve3DClass::Find_Interval`: (i0, i1, t).
    pub fn find_interval(&self, time: f32) -> (i32, i32, f32) {
        debug_assert!(time >= self.keys[0].time);
        debug_assert!(time <= self.keys[self.keys.len() - 1].time);
        let mut i = 0usize;
        while time > self.keys[i + 1].time {
            i += 1;
        }
        let t = (time - self.keys[i].time) / (self.keys[i + 1].time - self.keys[i].time);
        (i as i32, i as i32 + 1, t)
    }

    /// `Curve3DClass::Save` (curve.cpp:166).
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        let keycount = self.keys.len() as i32;
        csave.begin_chunk(CURVE3D_CHUNK_VARIABLES);
        csave.write_micro_chunk(CURVE3D_VARIABLE_ISLOOPING, &[self.is_looping as u8]);
        csave.write_micro_chunk(CURVE3D_VARIABLE_KEYCOUNT, &keycount.to_le_bytes());
        csave.end_chunk();

        csave.begin_chunk(CURVE3D_CHUNK_KEYS);
        for k in &self.keys {
            csave.write_floats(&[k.point.x, k.point.y, k.point.z]);
            csave.write_floats(&[k.time]);
        }
        csave.end_chunk();
        true
    }

    /// `Curve3DClass::Load` (curve.cpp:187).
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        let mut keycount: i32 = 0;
        self.keys.clear();
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                CURVE3D_CHUNK_VARIABLES => {
                    while cload.open_micro_chunk().unwrap_or(false) {
                        match cload.cur_micro_chunk_id() {
                            CURVE3D_VARIABLE_ISLOOPING => {
                                let mut b = [0u8; 1];
                                let _ = cload.read_into(&mut b);
                                self.is_looping = b[0] != 0;
                            }
                            CURVE3D_VARIABLE_KEYCOUNT => keycount = read_u32(cload) as i32,
                            _ => {}
                        }
                        let _ = cload.close_micro_chunk();
                    }
                }
                CURVE3D_CHUNK_KEYS => {
                    for _ in 0..keycount {
                        let x = read_f32(cload);
                        let y = read_f32(cload);
                        let z = read_f32(cload);
                        let time = read_f32(cload);
                        self.keys.push(Curve3DKey { point: Vector3::new(x, y, z), time });
                    }
                }
                _ => {} // WWDEBUG_SAY("Unhandled Chunk")
            }
            let _ = cload.close_chunk();
        }
        true
    }
}

/// `class LinearCurve3DClass : public Curve3DClass`.
#[derive(Clone, Default)]
pub struct LinearCurve3DClass {
    pub base: Curve3DClass,
}

impl LinearCurve3DClass {
    /// `LinearCurve3DClass::Evaluate` (curve.cpp:268).
    pub fn evaluate(&self, time: f32) -> Vector3 {
        let keys = &self.base.keys;
        if time < keys[0].time {
            return keys[0].point;
        }
        if time >= keys[keys.len() - 1].time {
            return keys[keys.len() - 1].point;
        }
        let (i0, i1, t) = self.base.find_interval(time);
        keys[i0 as usize].point + t * (keys[i1 as usize].point - keys[i0 as usize].point)
    }

    /// `PersistClass::Get_Factory` -> the factory's chunk id (`_LinearCurve3DFactory`).
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_LINEARCURVE3D
    }

    /// `LinearCurve3DClass::Save`.
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.begin_chunk(LINEARCURVE3D_CHUNK_CURVE3D);
        self.base.save(csave);
        csave.end_chunk();
        true
    }

    /// `LinearCurve3DClass::Load`.
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        while cload.open_chunk().unwrap_or(false) {
            if cload.cur_chunk_id().unwrap_or(0) == LINEARCURVE3D_CHUNK_CURVE3D {
                self.base.load(cload);
            }
            let _ = cload.close_chunk();
        }
        true
    }
}

/// `Curve1DClass::KeyClass`.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Curve1DKey {
    pub point: f32,
    pub time: f32,
    pub extra: u32,
}

/// `class Curve1DClass` (curve.h).
#[derive(Clone, Default)]
pub struct Curve1DClass {
    pub is_looping: bool,
    pub keys: Vec<Curve1DKey>,
}

impl Curve1DClass {
    /// `Curve1DClass::Curve1DClass` (curve.cpp:330).
    pub fn new() -> Self {
        Curve1DClass { is_looping: false, keys: Vec::new() }
    }

    /// `Curve1DClass::Is_Looping`.
    pub fn is_looping(&self) -> bool {
        self.is_looping
    }

    /// `Curve1DClass::Set_Looping`.
    pub fn set_looping(&mut self, onoff: bool) {
        self.is_looping = onoff;
    }

    /// `Curve1DClass::Get_Start_Time`.
    pub fn get_start_time(&self) -> f32 {
        if !self.keys.is_empty() {
            self.keys[0].time
        } else {
            0.0
        }
    }

    /// `Curve1DClass::Get_End_Time`.
    pub fn get_end_time(&self) -> f32 {
        if !self.keys.is_empty() {
            self.keys[self.keys.len() - 1].time
        } else {
            0.0
        }
    }

    /// `Curve1DClass::Key_Count`.
    pub fn key_count(&self) -> i32 {
        self.keys.len() as i32
    }

    /// `Curve1DClass::Get_Key`: (point, time, extra).
    pub fn get_key(&self, i: i32) -> (f32, f32, u32) {
        debug_assert!(i >= 0);
        debug_assert!((i as usize) < self.keys.len());
        let k = &self.keys[i as usize];
        (k.point, k.time, k.extra)
    }

    /// `Curve1DClass::Set_Key`.
    pub fn set_key(&mut self, i: i32, point: f32, extra: u32) {
        debug_assert!(i >= 0);
        debug_assert!((i as usize) < self.keys.len());
        self.keys[i as usize].point = point;
        self.keys[i as usize].extra = extra;
    }

    /// `Curve1DClass::Add_Key`.
    pub fn add_key(&mut self, point: f32, t: f32, extra: u32) -> i32 {
        let mut idx = 0usize;
        while idx < self.keys.len() && self.keys[idx].time < t {
            idx += 1;
        }
        self.keys.insert(idx, Curve1DKey { point, time: t, extra });
        idx as i32
    }

    /// `Curve1DClass::Remove_Key`.
    pub fn remove_key(&mut self, i: i32) {
        debug_assert!(i >= 0);
        debug_assert!((i as usize) < self.keys.len());
        self.keys.remove(i as usize);
    }

    /// `Curve1DClass::Clear_Keys`.
    pub fn clear_keys(&mut self) {
        self.keys.clear();
    }

    /// `Curve1DClass::Find_Interval` (curve.cpp): the looping branch wraps over [last, first].
    pub fn find_interval(&self, time: f32) -> (i32, i32, f32) {
        let n = self.keys.len();
        if self.is_looping {
            if time < self.keys[0].time {
                let i0 = n - 1;
                let i1 = 0usize;
                let interval = 1.0 - self.keys[i0].time + self.keys[i1].time;
                let t = (1.0 - self.keys[i0].time + time) / interval;
                return (i0 as i32, i1 as i32, t);
            } else if time > self.keys[n - 1].time {
                let i0 = n - 1;
                let i1 = 0usize;
                let interval = 1.0 - self.keys[i0].time + self.keys[i1].time;
                let t = (time - self.keys[i0].time) / interval;
                return (i0 as i32, i1 as i32, t);
            }
        } else {
            debug_assert!(time >= self.keys[0].time);
            debug_assert!(time <= self.keys[n - 1].time);
        }
        let mut i = 0usize;
        while time > self.keys[i + 1].time {
            i += 1;
        }
        let t = (time - self.keys[i].time) / (self.keys[i + 1].time - self.keys[i].time);
        (i as i32, i as i32 + 1, t)
    }

    /// `Curve1DClass::Save`.
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        let keycount = self.keys.len() as i32;
        csave.begin_chunk(CURVE1D_CHUNK_VARIABLES);
        csave.write_micro_chunk(CURVE1D_VARIABLE_ISLOOPING, &[self.is_looping as u8]);
        csave.write_micro_chunk(CURVE1D_VARIABLE_KEYCOUNT, &keycount.to_le_bytes());
        csave.end_chunk();

        csave.begin_chunk(CURVE1D_CHUNK_KEYS);
        for k in &self.keys {
            csave.write_floats(&[k.point]);
            csave.write_floats(&[k.time]);
            csave.write(&k.extra.to_le_bytes());
        }
        csave.end_chunk();
        true
    }

    /// `Curve1DClass::Load`.
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        let mut keycount: i32 = 0;
        self.keys.clear();
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                CURVE1D_CHUNK_VARIABLES => {
                    while cload.open_micro_chunk().unwrap_or(false) {
                        match cload.cur_micro_chunk_id() {
                            CURVE1D_VARIABLE_ISLOOPING => {
                                let mut b = [0u8; 1];
                                let _ = cload.read_into(&mut b);
                                self.is_looping = b[0] != 0;
                            }
                            CURVE1D_VARIABLE_KEYCOUNT => keycount = read_u32(cload) as i32,
                            _ => {}
                        }
                        let _ = cload.close_micro_chunk();
                    }
                }
                CURVE1D_CHUNK_KEYS => {
                    for _ in 0..keycount {
                        let point = read_f32(cload);
                        let time = read_f32(cload);
                        let extra = read_u32(cload);
                        self.keys.push(Curve1DKey { point, time, extra });
                    }
                }
                _ => {}
            }
            let _ = cload.close_chunk();
        }
        true
    }
}

/// `class LinearCurve1DClass : public Curve1DClass`.
#[derive(Clone, Default)]
pub struct LinearCurve1DClass {
    pub base: Curve1DClass,
}

impl LinearCurve1DClass {
    /// `LinearCurve1DClass::Evaluate` (curve.cpp:530).
    pub fn evaluate(&self, time: f32) -> f32 {
        let keys = &self.base.keys;
        if !self.base.is_looping {
            if time < keys[0].time {
                return keys[0].point;
            }
            if time >= keys[keys.len() - 1].time {
                return keys[keys.len() - 1].point;
            }
        }
        let (i0, i1, t) = self.base.find_interval(time);
        keys[i0 as usize].point + t * (keys[i1 as usize].point - keys[i0 as usize].point)
    }

    /// `PersistClass::Get_Factory` -> the factory's chunk id (`_LinearCurve1DFactory`).
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_LINEARCURVE1D
    }

    /// `LinearCurve1DClass::Save`.
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.begin_chunk(LINEARCURVE1D_CHUNK_CURVE1D);
        self.base.save(csave);
        csave.end_chunk();
        true
    }

    /// `LinearCurve1DClass::Load`.
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        while cload.open_chunk().unwrap_or(false) {
            if cload.cur_chunk_id().unwrap_or(0) == LINEARCURVE1D_CHUNK_CURVE1D {
                self.base.load(cload);
            }
            let _ = cload.close_chunk();
        }
        true
    }
}
