//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshmdl.cpp
//!
//! `MeshModelClass::Make_Geometry_Unique`, `::Make_UV_Array_Unique`, `::Make_Color_Array_Unique` and
//! `::Is_Alternate_Material_Description_Enabled`. The model's shared buffers are `Arc`s ([`ShareBufferClass`] upstream:
//! "make unique" copies the buffer into a new one that only this model holds); `CurMatDesc` is the material description
//! the model currently uses (default or alternate), `meshmatdesc` draft's [`MeshMatDescClass`].
//!
//! `OPTIMIZE_PLANEEQ_RAM` is 1 (meshgeometry.h:80), so the `PlaneEq` copy under `#if (!OPTIMIZE_PLANEEQ_RAM)` in
//! `Make_Geometry_Unique` is not compiled and is not translated.

use std::sync::Arc;

use crate::drafts::sweep_ww3d::meshmatdesc::MeshMatDescClass;

/// Which description `CurMatDesc` points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurMatDesc {
    /// `DefMatDesc`.
    Default,
    /// `AlternateMatDesc`.
    Alternate,
}

/// The shared geometry buffers (`Vertex`, `VertexNorm`), each a `ShareBufferClass<Vector3>`.
#[derive(Clone, Debug, Default)]
pub struct MeshModelGeometry {
    pub vertex: Option<Arc<Vec<[f32; 3]>>>,
    pub vertex_norm: Option<Arc<Vec<[f32; 3]>>>,
}

/// `MeshModelClass::Make_Geometry_Unique()` (meshmdl.cpp:291): "WWASSERT(Vertex)"; the vertex and normal buffers are each
/// replaced by a fresh copy (`NEW_REF(ShareBufferClass<Vector3>, (*Vertex))`, `REF_PTR_SET`), so no other model shares them.
/// Upstream dereferences `VertexNorm` too; a missing buffer panics here.
pub fn make_geometry_unique(geom: &mut MeshModelGeometry) {
    let v = geom.vertex.as_ref().expect("WWASSERT(Vertex)");
    let unique_verts = Arc::new((**v).clone());
    geom.vertex = Some(unique_verts);

    let n = geom.vertex_norm.as_ref().expect("VertexNorm");
    let norms = Arc::new((**n).clone());
    geom.vertex_norm = Some(norms);
}

/// `MeshModelClass::Make_UV_Array_Unique(pass, stage)` (meshmdl.cpp:310): `CurMatDesc->Make_UV_Array_Unique(pass, stage)`.
pub fn make_uv_array_unique<M: Clone, S: Clone + Default>(cur_mat_desc: &mut MeshMatDescClass<M, S>, pass: usize, stage: usize) {
    cur_mat_desc.make_uv_array_unique(pass, stage);
}

/// `MeshModelClass::Make_Color_Array_Unique(array_index)` (meshmdl.cpp:315): `CurMatDesc->Make_Color_Array_Unique(array_index)`.
pub fn make_color_array_unique<M: Clone, S: Clone + Default>(cur_mat_desc: &mut MeshMatDescClass<M, S>, array_index: usize) {
    cur_mat_desc.make_color_array_unique(array_index);
}

/// `MeshModelClass::Is_Alternate_Material_Description_Enabled()` (meshmdl.cpp:351): `CurMatDesc == AlternateMatDesc`. With no
/// alternate description (`AlternateMatDesc == NULL`) `CurMatDesc` is never equal to it.
pub fn is_alternate_material_description_enabled(cur: CurMatDesc, has_alternate: bool) -> bool {
    has_alternate && cur == CurMatDesc::Alternate
}
