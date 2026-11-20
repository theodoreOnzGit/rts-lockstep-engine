//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_ww3d7_misc.rs);
//! not yet wired into the game.
//! `Insert_Inactive_Model` was corrected by this test: `RefRenderObjListClass::Add` links at the head and refuses a model already in the list.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/renderobjectrecycler.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/renderobjectrecycler.h

use super::handles::RenderObjHandle;
use crate::ww::matrix3d::Matrix3D;

/// `RenderObjClass::CLASSID_PARTICLEEMITTER` is what `Reset_Model` tests; the owner answers it.
/// What the recycler asks of the render objects and the asset manager, as a compile-time contract.
pub trait RecyclerHost {
    /// `robj->Get_Name()`.
    fn get_name(&self, robj: RenderObjHandle) -> String;
    /// `obj->Set_Transform(tm)`.
    fn set_transform(&mut self, robj: RenderObjHandle, tm: &Matrix3D);
    /// `WW3DAssetManager::Get_Instance()->Create_Render_Obj(name)`.
    fn create_render_obj(&mut self, name: &str) -> Option<RenderObjHandle>;
    /// `model->Get_Num_Sub_Objects()`.
    fn get_num_sub_objects(&self, robj: RenderObjHandle) -> usize;
    /// `model->Get_Sub_Object(i)` (the returned reference is released by the caller upstream; not modelled).
    fn get_sub_object(&self, robj: RenderObjHandle, i: usize) -> Option<RenderObjHandle>;
    /// `model->Class_ID() == RenderObjClass::CLASSID_PARTICLEEMITTER`.
    fn is_particle_emitter(&self, robj: RenderObjHandle) -> bool;
    /// `((ParticleEmitterClass *)model)->Reset()`.
    fn reset_emitter(&mut self, robj: RenderObjHandle);
    /// `model->Peek_Animation() != NULL`.
    fn has_animation(&self, robj: RenderObjHandle) -> bool;
    /// `model->Set_Animation(model->Peek_Animation(), 0.0f, ANIM_MODE_ONCE)`.
    fn restart_animation_once(&mut self, robj: RenderObjHandle);
}

/// `class RenderObjectRecyclerClass` (renderobjectrecycler.h): a pool of inactive render objects.
#[derive(Clone, Debug, Default)]
pub struct RenderObjectRecyclerClass {
    inactive_models: Vec<RenderObjHandle>,
}

impl RenderObjectRecyclerClass {
    /// `RenderObjectRecyclerClass::Reset` (renderobjectrecycler.cpp:55): `InactiveModels.Reset_List()`.
    pub fn reset(&mut self) {
        self.inactive_models.clear();
    }

    /// `RenderObjectRecyclerClass::Get_Render_Object` (renderobjectrecycler.cpp:72): the first inactive model with
    /// this name (case-insensitive, `stricmp`) is taken out of the pool, placed at `tm` and reset; otherwise a
    /// new one is created through the asset manager and placed at `tm`.
    pub fn get_render_object<H: RecyclerHost>(&mut self, host: &mut H, name: &str, tm: &Matrix3D) -> Option<RenderObjHandle> {
        let mut found: Option<usize> = None;
        for (idx, obj) in self.inactive_models.iter().enumerate() {
            let same = if crate::logic::modules::plant(19466) { host.get_name(*obj) == name } else { host.get_name(*obj).eq_ignore_ascii_case(name) };
            if same {
                found = Some(idx);
                if !crate::logic::modules::plant(19467) {
                    break;
                }
            }
        }

        if let Some(idx) = found {
            let obj = if crate::logic::modules::plant(19473) { self.inactive_models.pop().unwrap_or(0) } else { self.inactive_models.remove(idx) }; // Add_Ref then InactiveModels.Remove
            if !crate::logic::modules::plant(19468) {
                host.set_transform(obj, tm);
            }
            Self::reset_model(host, obj);
            Some(obj)
        } else {
            let new_model = host.create_render_obj(name);
            if let Some(m) = new_model {
                if !crate::logic::modules::plant(19472) {
                    host.set_transform(m, tm);
                }
                return Some(m);
            }
            None
        }
    }

    /// `RenderObjectRecyclerClass::Return_Render_Object` (renderobjectrecycler.cpp:117).
    pub fn return_render_object(&mut self, obj: RenderObjHandle) {
        self.insert_inactive_model(obj);
    }

    /// `RenderObjectRecyclerClass::Insert_Inactive_Model` (renderobjectrecycler.cpp:131).
    ///
    /// ~~Pushed at the end.~~ **CORRECTED 2026-11-20 (differential test #215, 5,469 of 20,000 cases differed):** `InactiveModels` is a
    /// `RefRenderObjListClass`, whose `Add` is `Internal_Add(obj, onlyonce = true)` (multilist.cpp:97): it links the object at the HEAD of
    /// the list (so the newest returned model is found first by `Get_Render_Object`) and REFUSES an object that is already in the list.
    fn insert_inactive_model(&mut self, obj: RenderObjHandle) {
        if self.inactive_models.contains(&obj) {
            return;
        }
        self.inactive_models.insert(0, obj);
    }

    /// `RenderObjectRecyclerClass::Reset_Model` (renderobjectrecycler.cpp:147): recursively resets the
    /// sub-objects, resets a particle emitter, and restarts an animation at frame 0 in ANIM_MODE_ONCE.
    fn reset_model<H: RecyclerHost>(host: &mut H, model: RenderObjHandle) {
        let n = if crate::logic::modules::plant(19469) { 0 } else { host.get_num_sub_objects(model) };
        for i in 0..n {
            if let Some(robj) = host.get_sub_object(model, i) {
                Self::reset_model(host, robj);
            }
            // robj->Release_Ref() — not modelled
        }

        // particle emitters must be reset (this should release their buffers)
        if host.is_particle_emitter(model) && !crate::logic::modules::plant(19470) {
            host.reset_emitter(model);
        }

        // animated models must have their animation reset (if present)
        if host.has_animation(model) && !crate::logic::modules::plant(19471) {
            host.restart_animation_once(model);
        }
    }
}
