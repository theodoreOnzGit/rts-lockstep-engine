//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/tcbspline.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/tcbspline.cpp
//!
//! Upstream's `Update_Tangents` has no `return` for fewer than two keys (UB afterwards); this draft returns.

use super::chunk_save::ChunkSaveClass;
use super::curve::read_f32;
use super::hermite_spline::HermiteSpline3DClass;
use super::saveload_ids::*;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::vector3::Vector3;

/// ID's used by TCBSpline3D (tcbspline.cpp:46).
pub const TCB3D_CHUNK_HERMITE3D: u32 = 0x0207_1009;
pub const TCB3D_CHUNK_PARAMS: u32 = 0x0207_100A;

/// `TCBSpline3DClass::TCBClass`.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct TcbClass {
    pub tension: f32,
    pub continuity: f32,
    pub bias: f32,
}

/// `class TCBSpline3DClass : public HermiteSpline3DClass` (Kochanek-Bartels).
#[derive(Clone, Default)]
pub struct TCBSpline3DClass {
    pub base: HermiteSpline3DClass,
    pub params: Vec<TcbClass>,
}

impl TCBSpline3DClass {
    /// `TCBSpline3DClass::Add_Key`.
    pub fn add_key(&mut self, point: &Vector3, t: f32) -> i32 {
        let index = self.base.add_key(point, t);
        self.params.insert(index as usize, TcbClass { tension: 0.0, continuity: 0.0, bias: 0.0 });
        index
    }

    /// `TCBSpline3DClass::Remove_Key`.
    pub fn remove_key(&mut self, i: i32) {
        self.base.remove_key(i);
        self.params.remove(i as usize);
    }

    /// `TCBSpline3DClass::Clear_Keys`.
    pub fn clear_keys(&mut self) {
        self.base.clear_keys();
        self.params.clear();
    }

    /// `TCBSpline3DClass::Set_TCB_Params`.
    pub fn set_tcb_params(&mut self, i: i32, tension: f32, continuity: f32, bias: f32) {
        debug_assert!(i >= 0);
        debug_assert!((i as usize) < self.params.len());
        self.params[i as usize] = TcbClass { tension, continuity, bias };
        self.base.tangents_dirty = true;
    }

    /// `TCBSpline3DClass::Get_TCB_Params`: (tension, continuity, bias).
    pub fn get_tcb_params(&self, i: i32) -> (f32, f32, f32) {
        let p = &self.params[i as usize];
        (p.tension, p.continuity, p.bias)
    }

    /// `TCBSpline3DClass::Update_Tangents` (tcbspline.cpp:118).
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
        let pr = &self.params;
        // First and Last Key:
        // Only need to compute the OutTangent for key[0] and the InTangent for key[end]
        let end = n - 1;
        tan[0].in_tangent.set(0.0, 0.0, 0.0);
        tan[end].out_tangent.set(0.0, 0.0, 0.0);

        if self.base.base.is_looping {
            // This really only works if the start and end points have the same position...
            // Also just using the TCB params from p0 for both
            let k0 = 0.5 * ((1.0 - pr[0].tension) * (1.0 - pr[0].continuity) * (1.0 - pr[0].bias));
            let k1 = 0.5 * ((1.0 - pr[0].tension) * (1.0 + pr[0].continuity) * (1.0 + pr[0].bias));
            let k2 = 0.5 * ((1.0 - pr[0].tension) * (1.0 + pr[0].continuity) * (1.0 - pr[0].bias));
            let k3 = 0.5 * ((1.0 - pr[0].tension) * (1.0 - pr[0].continuity) * (1.0 + pr[0].bias));

            let dp_in = keys[0].point - keys[end - 1].point;
            let dp_out = keys[1].point - keys[0].point;
            tan[end].in_tangent = (k0 * dp_in) + (k1 * dp_out);
            tan[0].out_tangent = (k2 * dp_out) + (k3 * dp_in);
        } else {
            let k2 = 0.25 * ((1.0 - pr[0].tension) * (1.0 + pr[0].continuity) * (1.0 - pr[0].bias));
            let k3 = 0.25 * ((1.0 - pr[0].tension) * (1.0 - pr[0].continuity) * (1.0 + pr[0].bias));

            let mut dp_out = keys[1].point - keys[0].point;
            let mut dp_in = dp_out;
            tan[0].out_tangent = (k2 * dp_out) + (k3 * dp_in);

            let k0 = 0.25 * ((1.0 - pr[0].tension) * (1.0 - pr[0].continuity) * (1.0 - pr[0].bias));
            let k1 = 0.25 * ((1.0 - pr[0].tension) * (1.0 + pr[0].continuity) * (1.0 + pr[0].bias));

            dp_in = keys[end].point - keys[end - 1].point;
            dp_out = dp_in;
            tan[end].in_tangent = (k0 * dp_out) + (k1 * dp_in);
        }

        let total_time = (keys[1].time - keys[0].time) + (keys[end].time - keys[end - 1].time);
        let in_factor = 2.0 * (keys[end].time - keys[end - 1].time) / total_time;
        let out_factor = 2.0 * (keys[1].time - keys[0].time) / total_time;
        tan[end].in_tangent *= in_factor;
        tan[0].out_tangent *= out_factor;

        // Now compute the tangents of all of the normal keys...
        for pi in 1..n - 1 {
            let k0 = 0.5 * ((1.0 - pr[pi].tension) * (1.0 - pr[pi].continuity) * (1.0 - pr[pi].bias));
            let k1 = 0.5 * ((1.0 - pr[pi].tension) * (1.0 + pr[pi].continuity) * (1.0 + pr[pi].bias));
            let k2 = 0.5 * ((1.0 - pr[pi].tension) * (1.0 + pr[pi].continuity) * (1.0 - pr[pi].bias));
            let k3 = 0.5 * ((1.0 - pr[pi].tension) * (1.0 - pr[pi].continuity) * (1.0 + pr[pi].bias));

            let dp_in = keys[pi].point - keys[pi - 1].point;
            let dp_out = keys[pi + 1].point - keys[pi].point;
            tan[pi].in_tangent = (k0 * dp_out) + (k1 * dp_in);
            tan[pi].out_tangent = (k2 * dp_out) + (k3 * dp_in);

            let total_time = keys[pi + 1].time - keys[pi - 1].time;
            let in_factor = 2.0 * (keys[pi].time - keys[pi - 1].time) / total_time;
            let out_factor = 2.0 * (keys[pi + 1].time - keys[pi].time) / total_time;
            tan[pi].in_tangent *= in_factor; // compensating for un-even keys
            tan[pi].out_tangent *= out_factor;
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

    /// `PersistClass::Get_Factory` -> `_TCBSpline3DFactory`'s chunk id.
    pub fn get_factory_id(&self) -> u32 {
        WWMATH_CHUNKID_TCBSPLINE3D
    }

    /// `TCBSpline3DClass::Save`.
    pub fn save(&self, csave: &mut ChunkSaveClass) -> bool {
        csave.begin_chunk(TCB3D_CHUNK_HERMITE3D);
        self.base.save(csave);
        csave.end_chunk();

        csave.begin_chunk(TCB3D_CHUNK_PARAMS);
        for p in &self.params {
            csave.write_floats(&[p.tension]);
            csave.write_floats(&[p.continuity]);
            csave.write_floats(&[p.bias]);
        }
        csave.end_chunk();
        true
    }

    /// `TCBSpline3DClass::Load`.
    pub fn load(&mut self, cload: &mut ChunkLoad) -> bool {
        self.params.clear();
        while cload.open_chunk().unwrap_or(false) {
            match cload.cur_chunk_id().unwrap_or(0) {
                TCB3D_CHUNK_HERMITE3D => {
                    self.base.load(cload);
                }
                TCB3D_CHUNK_PARAMS => {
                    for _ in 0..self.base.base.keys.len() {
                        let tension = read_f32(cload);
                        let continuity = read_f32(cload);
                        let bias = read_f32(cload);
                        self.params.push(TcbClass { tension, continuity, bias });
                    }
                }
                _ => {}
            }
            let _ = cload.close_chunk();
        }
        true
    }
}
