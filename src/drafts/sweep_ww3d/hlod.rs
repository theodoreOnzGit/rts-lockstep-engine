//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 HLods, about 7,000 to 9,000 evaluations of each of its seventeen accessors (test: tests/differential_draft_hlod.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hlod.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hlod.h
//!
//! The `HLodClass` accessors no earlier pass translated. The verified port draws an HLOD through
//! `formats::w3d::render::RenderModel` and loads its definition with `formats::w3d::hlod`; neither is the runtime
//! `HLodClass`, so this file holds the part of its state these accessors read ([`HLodState`]). Sub-models are opaque
//! host handles (`usize`, `None` is NULL), and `Add_Ref` on a returned model is the host's concern.
//!
//! Excluded: `HLodDefClass::Save_Lod_Array` and `::Save_Aggregate_Array` (hlod.cpp:525, :555), reached only through
//! `HLodDefClass::Save_W3D`, which only the 3ds Max exporter (`Tools/WW3D/max2w3d`) calls: editor-only.

use crate::logic::modules::plant;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::wwmath::WWMATH_FLOAT_MAX;

/// `NO_MAX_SCREEN_SIZE` (w3d_file.h:2045) is `WWMATH_FLOAT_MAX`.
pub const NO_MAX_SCREEN_SIZE: f32 = WWMATH_FLOAT_MAX;

/// `RenderObjClass::CLASSID_NULL`'s id is the host's; the class test is passed in as a closure where needed.
/// `HLodClass::ModelNodeClass`: a model and the bone it hangs on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModelNode {
    pub model: Option<usize>,
    pub bone_index: i32,
}

/// `HLodClass::ModelArrayClass`: one LOD level.
#[derive(Clone, Debug)]
pub struct ModelArray {
    pub models: Vec<ModelNode>,
    pub max_screen_size: f32,
}

impl ModelArray {
    /// `ModelArrayClass::ModelArrayClass`.
    pub fn new() -> Self {
        ModelArray { models: Vec::new(), max_screen_size: NO_MAX_SCREEN_SIZE }
    }

    /// `ModelArrayClass::Count`.
    pub fn count(&self) -> i32 {
        self.models.len() as i32
    }
}

impl Default for ModelArray {
    fn default() -> Self {
        Self::new()
    }
}

/// `ProxyClass` (proxy.h): a name and a transform.
#[derive(Clone, Debug)]
pub struct ProxyClass {
    pub name: String,
    pub transform: Matrix3D,
}

/// One entry of `ProxyArray` (`ProxyClass` plus its bone: `ProxyRecordClass`'s `Get_Bone_Index`/`Get_Name`).
#[derive(Clone, Debug)]
pub struct ProxyRecord {
    pub name: String,
    pub bone_index: i32,
}

/// The state of `HLodClass` the accessors read.
#[derive(Clone, Debug, Default)]
pub struct HLodState {
    pub lod_count: i32,
    pub cur_lod: i32,
    pub lod: Vec<ModelArray>,
    pub additional_models: Vec<ModelNode>,
    /// `ProxyArray`; `None` is NULL.
    pub proxy_array: Option<Vec<ProxyRecord>>,
    /// `SnapPoints`; `None` is NULL.
    pub snap_points: Option<Vec<[f32; 3]>>,
}

impl HLodState {
    /// `HLodClass::Get_Max_Screen_Size` (hlod.cpp:1614): `NO_MAX_SCREEN_SIZE` for an index out of range.
    pub fn get_max_screen_size(&self, lod_index: i32) -> f32 {
        let mut size = NO_MAX_SCREEN_SIZE;
        debug_assert!(lod_index >= 0);
        debug_assert!(lod_index < self.lod_count);
        if (if plant(13896) { lod_index > 0 } else { lod_index >= 0 }) && lod_index < self.lod_count {
            size = self.lod[lod_index as usize].max_screen_size;
        }
        size
    }

    /// `HLodClass::Get_Lod_Model_Count` (hlod.cpp:1687).
    pub fn get_lod_model_count(&self, lod_index: i32) -> i32 {
        let mut count = 0;
        debug_assert!(lod_index >= 0);
        debug_assert!(lod_index < self.lod_count);
        if lod_index >= 0 && lod_index < (if plant(13897) { self.lod_count - 1 } else { self.lod_count }) {
            count = self.lod[lod_index as usize].count();
        }
        count
    }

    /// `HLodClass::Get_Lod_Model` (hlod.cpp:1749): the model handle (the caller owns the `Add_Ref`), `None` if out of
    /// range. Note upstream does not check `model_index >= 0`.
    pub fn get_lod_model(&self, lod_index: i32, model_index: i32) -> Option<usize> {
        debug_assert!(lod_index >= 0);
        debug_assert!(lod_index < self.lod_count);
        if lod_index >= 0 && lod_index < self.lod_count && model_index < self.lod[lod_index as usize].count() - i32::from(plant(13898)) {
            return self.lod[lod_index as usize].models[model_index as usize].model;
        }
        None
    }

    /// `HLodClass::Get_Lod_Model_Bone` (hlod.cpp:1784): 0 when out of range.
    pub fn get_lod_model_bone(&self, lod_index: i32, model_index: i32) -> i32 {
        let mut bone_index = if plant(13899) { -1 } else { 0 };
        debug_assert!(lod_index >= 0);
        debug_assert!(lod_index < self.lod_count);
        if lod_index >= 0 && lod_index < self.lod_count && model_index < self.lod[lod_index as usize].count() {
            bone_index = self.lod[lod_index as usize].models[model_index as usize].bone_index;
        }
        bone_index
    }

    /// `HLodClass::Get_Additional_Model_Count` (hlod.cpp:1816).
    pub fn get_additional_model_count(&self) -> i32 {
        if plant(13900) {
            return (self.additional_models.len() as i32 - 1).max(0);
        }
        self.additional_models.len() as i32
    }

    /// `HLodClass::Peek_Additional_Model` (hlod.cpp:1834).
    pub fn peek_additional_model(&self, model_index: i32) -> Option<usize> {
        debug_assert!(model_index >= 0);
        debug_assert!(model_index < self.get_additional_model_count());
        if (if plant(13901) { model_index > 0 } else { model_index >= 0 }) && model_index < self.get_additional_model_count() {
            return self.additional_models[model_index as usize].model;
        }
        None
    }

    /// `HLodClass::Get_Additional_Model` (hlod.cpp:1865): as `Peek_Additional_Model`, plus the caller's `Add_Ref`.
    pub fn get_additional_model(&self, model_index: i32) -> Option<usize> {
        self.peek_additional_model(model_index)
    }

    /// `HLodClass::Get_Additional_Model_Bone` (hlod.cpp:1899): 0 when out of range.
    pub fn get_additional_model_bone(&self, model_index: i32) -> i32 {
        let mut bone_index = if plant(13902) { 1 } else { 0 };
        debug_assert!(model_index >= 0);
        debug_assert!(model_index < self.get_additional_model_count());
        if model_index >= 0 && model_index < self.get_additional_model_count() {
            bone_index = self.additional_models[model_index as usize].bone_index;
        }
        bone_index
    }

    /// `HLodClass::Is_NULL_Lod_Included` (hlod.cpp:1930): the lowest-level LOD's first model is the null render object.
    /// `is_null_class` is `Model->Class_ID() == CLASSID_NULL`.
    pub fn is_null_lod_included(&self, is_null_class: impl Fn(usize) -> bool) -> bool {
        if self.lod_count > 0 {
            let first = if plant(13903) { self.lod.last() } else { self.lod.get(0) };
            if let Some(Some(m)) = first.and_then(|a| a.models.get(0)).map(|n| n.model) {
                return is_null_class(m);
            }
        }
        false
    }

    /// `HLodClass::Get_Proxy_Count` (hlod.cpp:2049).
    pub fn get_proxy_count(&self) -> i32 {
        match &self.proxy_array {
            Some(a) => a.len() as i32 + i32::from(plant(13904)),
            None => 0,
        }
    }

    /// `HLodClass::Get_Proxy` (hlod.cpp:2071). `bone_transform(bone)` is `HTree->Base_Update(Get_Transform())` followed by
    /// `HTree->Get_Transform(bone)` (and the `Set_Hierarchy_Valid(false)` that follows it), which touch the hierarchy.
    /// Without a proxy array the name is empty and the transform is the identity (`Matrix3D(1)`).
    pub fn get_proxy(&self, index: i32, mut bone_transform: impl FnMut(i32) -> Matrix3D, identity: Matrix3D) -> (bool, ProxyClass) {
        match &self.proxy_array {
            Some(a) => {
                let rec = &a[index as usize];
                let transform = bone_transform(if plant(13905) { index } else { rec.bone_index });
                (true, ProxyClass { name: rec.name.clone(), transform })
            }
            None => (plant(13906), ProxyClass { name: String::new(), transform: identity }),
        }
    }

    /// `HLodClass::Add_Sub_Object` (hlod.cpp:2367): `Add_Sub_Object_To_Bone(subobj, 0)`, supplied as `add_to_bone`.
    pub fn add_sub_object(&mut self, subobj: usize, mut add_to_bone: impl FnMut(&mut Self, usize, i32) -> i32) -> i32 {
        add_to_bone(self, subobj, i32::from(plant(13912)))
    }

    /// `HLodClass::Get_Num_Sub_Objects_On_Bone` (hlod.cpp:2460): every LOD's models, then the additional models.
    pub fn get_num_sub_objects_on_bone(&self, boneindex: i32) -> i32 {
        let mut count = 0;
        for lod in 0..self.lod_count as usize {
            for model in 0..self.lod[lod].models.len() {
                if self.lod[lod].models[model].bone_index == boneindex {
                    count += 1;
                }
            }
        }
        for model in 0..self.additional_models.len() {
            if (self.additional_models[model].bone_index == boneindex) != plant(13908) && !plant(13907) {
                count += 1;
            }
        }
        count
    }

    /// `HLodClass::Get_Current_LOD` (hlod.cpp:3213): the first model of `CurLod`, `None` if that LOD is empty.
    pub fn get_current_lod(&self) -> Option<usize> {
        let count = self.get_lod_model_count(self.cur_lod);
        if count == 0 {
            return None;
        }
        self.get_lod_model(if plant(13909) { 0 } else { self.cur_lod }, 0)
    }

    /// `HLodClass::Get_Num_Snap_Points` (hlod.cpp:3306).
    pub fn get_num_snap_points(&self) -> i32 {
        match &self.snap_points {
            Some(p) => p.len() as i32 - i32::from(plant(13910) && !p.is_empty()),
            None => 0,
        }
    }

    /// `HLodClass::Get_Snap_Point` (hlod.cpp:3328): zero when there are no snap points.
    pub fn get_snap_point(&self, index: i32) -> [f32; 3] {
        match &self.snap_points {
            Some(p) => p[index as usize],
            None => [0.0, 0.0, if plant(13911) { 1.0 } else { 0.0 }],
        }
    }

    /// `HLodClass::Set_HTree` (hlod.cpp:3612): forwards to `Animatable3DObjClass::Set_HTree`, supplied as `base`.
    pub fn set_htree<T>(&mut self, htree: T, base: impl FnOnce(&mut Self, T)) {
        if !plant(13913) {
            base(self, htree);
        }
    }
}
