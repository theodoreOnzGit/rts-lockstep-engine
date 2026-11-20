//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//! PARTLY DIFFERENTIAL-TESTED (#202, #215): `UvBufferClass::update_crc` and `is_equal_to` match the upstream C++ (test: tests/differential_draft_meshmatdesc.rs);
//! the `Arc` buffers, `get_material_array`, `get_shader_array` and the `make_*_unique` functions stand in for intrusive reference counts and are not covered.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmatdesc.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmatdesc.h
//!
//! The reference-counted material buffers of `MeshMatDescClass`. Upstream's `ShareBufferClass<T>` is shared by
//! `Add_Ref`; here a shared buffer is `Arc<..>` and "`Num_Refs() > 1`" is `Arc::strong_count(..) > 1`. The element
//! pointers (`VertexMaterialClass*`, `TextureClass*`) are `Option<Arc<T>>`, so `Set_Element`'s `REF_PTR_SET`,
//! `Get_Element`'s `Add_Ref` and `Peek_Element` are `clone`, `clone` and a borrow.

use std::sync::Arc;

use crate::logic::modules::plant;

use crate::ww::realcrc::crc_memory;

/// `MatBufferClass` / `TexBufferClass`: a count of optional shared elements (`Array[i]` may be NULL).
#[derive(Clone, Debug)]
pub struct ElementBuffer<T> {
    pub array: Vec<Option<Arc<T>>>,
}

impl<T> ElementBuffer<T> {
    /// `ShareBufferClass(count, msg)`: all NULL.
    pub fn new(count: usize) -> Self {
        ElementBuffer { array: (0..count).map(|_| None).collect() }
    }

    /// `MatBufferClass::Set_Element` (meshmatdesc.cpp:73) / `TexBufferClass::Set_Element` (:117): `REF_PTR_SET`.
    pub fn set_element(&mut self, index: usize, value: Option<Arc<T>>) {
        self.array[index] = value;
    }

    /// `MatBufferClass::Get_Element` (:78) / `TexBufferClass::Get_Element` (:122): returns a new reference.
    pub fn get_element(&self, index: usize) -> Option<Arc<T>> {
        self.array[index].clone()
    }

    /// `MatBufferClass::Peek_Element` (:86) / `TexBufferClass::Peek_Element` (:130): no reference taken.
    pub fn peek_element(&self, index: usize) -> Option<&Arc<T>> {
        self.array[index].as_ref()
    }

    /// `Get_Count`.
    pub fn get_count(&self) -> usize {
        self.array.len()
    }
}

/// `MatBufferClass` (elements are `VertexMaterialClass`).
pub type MatBufferClass<M> = ElementBuffer<M>;
/// `TexBufferClass` (elements are `TextureClass`).
pub type TexBufferClass<T> = ElementBuffer<T>;

/// `UVBufferClass`: a `Vector2` array and the CRC `Update_CRC` last computed.
#[derive(Clone, Debug, PartialEq)]
pub struct UvBufferClass {
    pub array: Vec<[f32; 2]>,
    /// `CRC`, initially `0xFFFFFFFF`.
    pub crc: u32,
}

impl UvBufferClass {
    /// `UVBufferClass(count, msg)`.
    pub fn new(count: usize) -> Self {
        UvBufferClass { array: vec![[0.0; 2]; count], crc: if plant(17900) { 0 } else { 0xFFFF_FFFF } }
    }

    /// `UVBufferClass::Is_Equal_To` (meshmatdesc.cpp:155): equal CRCs, valid only after `Update_CRC` on both.
    pub fn is_equal_to(&self, that: &UvBufferClass) -> bool {
        if plant(17901) { self.crc <= that.crc } else { self.crc == that.crc }
    }

    /// `UVBufferClass::Update_CRC` (meshmatdesc.cpp:162): `CRC_Memory(Get_Array(), Get_Count() * sizeof(Vector2))`.
    pub fn update_crc(&mut self) {
        let mut bytes = Vec::with_capacity(self.array.len() * 8);
        for v in &self.array {
            if plant(17899) {
                bytes.extend_from_slice(&v[1].to_le_bytes());
                bytes.extend_from_slice(&v[0].to_le_bytes());
            } else if plant(17898) {
                bytes.extend_from_slice(&v[0].to_be_bytes());
                bytes.extend_from_slice(&v[1].to_be_bytes());
            } else {
                bytes.extend_from_slice(&v[0].to_le_bytes());
                bytes.extend_from_slice(&v[1].to_le_bytes());
            }
        }
        self.crc = crc_memory(&bytes, 0);
    }
}

/// `MAX_PASSES`, `MAX_TEX_STAGES`, `MAX_COLOR_ARRAYS`, `MAX_UV_ARRAYS` (meshmatdesc.h).
pub const MAX_PASSES: usize = 4;
pub const MAX_TEX_STAGES: usize = 2;
pub const MAX_COLOR_ARRAYS: usize = 2;
pub const MAX_UV_ARRAYS: usize = MAX_PASSES * MAX_TEX_STAGES;

/// The parts of `MeshMatDescClass` these four functions touch.
#[derive(Clone, Debug)]
pub struct MeshMatDescClass<M, S> {
    pub vertex_count: usize,
    pub poly_count: usize,
    pub material_array: [Option<Arc<MatBufferClass<M>>>; MAX_PASSES],
    /// `ShaderArray[pass]`, a `ShareBufferClass<ShaderClass>` (`S` is the port's `ShaderClass`).
    pub shader_array: [Option<Arc<Vec<S>>>; MAX_PASSES],
    pub uv_source: [[usize; MAX_TEX_STAGES]; MAX_PASSES],
    pub uv: Vec<Option<Arc<UvBufferClass>>>,
    pub color_array: [Option<Arc<Vec<u32>>>; MAX_COLOR_ARRAYS],
}

impl<M: Clone, S: Clone + Default> MeshMatDescClass<M, S> {
    /// `MeshMatDescClass::Get_Material_Array` (meshmatdesc.cpp:559): creates an all-NULL buffer when asked to.
    pub fn get_material_array(&mut self, pass: usize, create: bool) -> Option<&Arc<MatBufferClass<M>>> {
        if create && self.material_array[pass].is_none() {
            self.material_array[pass] = Some(Arc::new(MatBufferClass::new(self.vertex_count)));
        }
        self.material_array[pass].as_ref()
    }

    /// `MeshMatDescClass::Get_Shader_Array` (meshmatdesc.cpp:567): creates a cleared (default shader) array when asked.
    pub fn get_shader_array(&mut self, pass: usize, create: bool) -> Option<&Arc<Vec<S>>> {
        if create && self.shader_array[pass].is_none() {
            self.shader_array[pass] = Some(Arc::new(vec![S::default(); self.poly_count]));
        }
        self.shader_array[pass].as_ref()
    }

    /// `MeshMatDescClass::Make_UV_Array_Unique` (meshmatdesc.cpp:579): copy the UV array if anyone else holds it.
    pub fn make_uv_array_unique(&mut self, pass: usize, stage: usize) {
        let uvindex = self.uv_source[pass][stage];
        if let Some(buf) = &self.uv[uvindex] {
            if Arc::strong_count(buf) > 1 {
                let unique = Arc::new((**buf).clone());
                self.uv[uvindex] = Some(unique);
            }
        }
    }

    /// `MeshMatDescClass::Make_Color_Array_Unique` (meshmatdesc.cpp:589).
    pub fn make_color_array_unique(&mut self, array: usize) {
        if let Some(buf) = &self.color_array[array] {
            if Arc::strong_count(buf) > 1 {
                let unique = Arc::new((**buf).clone());
                self.color_array[array] = Some(unique);
            }
        }
    }
}
