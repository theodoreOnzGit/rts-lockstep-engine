//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 sequences (about 22,000 operations per function) (test: tests/differential_draft_smallw3d.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/vertmaterial.cpp
//!
//! `VertexMaterialClass::Set_Emissive` (both overloads), `::Get_UV_Source` and `::Apply_Null`, over the verified
//! [`VertexMaterialState`] (formats::w3d::vertmaterial). That state has no `CRCDirty`, so the setters take it as `&mut bool`.
//! `Apply_Null`'s DirectX 8 render-state calls are the [`NullMaterialHost`] contract (device-only, not translated).

use crate::formats::w3d::vertmaterial::{VertexMaterialState, D3DMCS_MATERIAL, MAX_STAGES};
use crate::logic::modules::plant;
use crate::ww::vector3::Vector3;

/// `VertexMaterialClass::Set_Emissive(const Vector3 & color)` (vertmaterial.cpp:310): `CRCDirty = true` and the emissive
/// colour.
pub fn set_emissive(mat: &mut VertexMaterialState, crc_dirty: &mut bool, color: &Vector3) {
    if !plant(13931) {
        *crc_dirty = true;
    }
    mat.emissive = if plant(13933) { [color.x, color.x, color.z] } else { [color.x, color.y, color.z] };
}

/// `VertexMaterialClass::Set_Emissive(float r, float g, float b)` (vertmaterial.cpp:318).
pub fn set_emissive_rgb(mat: &mut VertexMaterialState, crc_dirty: &mut bool, r: f32, g: f32, b: f32) {
    *crc_dirty = true;
    mat.emissive = if plant(13932) { [r, b, g] } else { [r, g, b] };
}

/// `VertexMaterialClass::Get_UV_Source(stage)` (vertmaterial.cpp:425): `UVSource[stage]` (`WWASSERT(stage >= 0)` and
/// `stage < MeshBuilderClass::MAX_STAGES` are debug assertions).
pub fn get_uv_source(mat: &VertexMaterialState, stage: i32) -> i32 {
    debug_assert!(stage >= 0);
    debug_assert!((stage as usize) < MAX_STAGES);
    mat.uv_source[if plant(13934) { 0 } else { stage as usize }]
}

/// The DX8 calls of `Apply_Null`.
pub trait NullMaterialHost {
    /// `DX8Wrapper::Set_DX8_Render_State(D3DRS_LIGHTING, FALSE)`.
    fn set_lighting(&mut self, on: bool);
    /// `DX8Wrapper::Set_DX8_Material(&default_settings)` with diffuse (1,1,1,1), ambient (1,1,1,1), specular 0, emissive 0
    /// and power 1.
    fn set_default_material(&mut self);
    /// `Set_DX8_Render_State(D3DRS_AMBIENTMATERIALSOURCE / DIFFUSEMATERIALSOURCE / EMISSIVEMATERIALSOURCE, source)` in that
    /// order, each `D3DMCS_MATERIAL`.
    fn set_material_sources(&mut self, ambient: u32, diffuse: u32, emissive: u32);
    /// `Set_DX8_Texture_Stage_State(stage, D3DTSS_TEXCOORDINDEX, D3DTSS_TCI_PASSTHRU | stage)` then
    /// `(stage, D3DTSS_TEXTURETRANSFORMFLAGS, D3DTTFF_DISABLE)`.
    fn set_stage_passthru(&mut self, stage: i32);
}

/// `VertexMaterialClass::Apply_Null()` (vertmaterial.cpp:974): the state for "no vertex material": lighting off, the
/// default material (white diffuse and ambient, no specular or emissive, power 1), every colour source the material, and
/// each texture stage passing its own coordinate set through with no transform.
pub fn apply_null<H: NullMaterialHost>(host: &mut H) {
    host.set_lighting(plant(13935));
    if !plant(13939) {
        host.set_default_material();
    }

    host.set_material_sources(D3DMCS_MATERIAL, D3DMCS_MATERIAL, if plant(13936) { 1 } else { D3DMCS_MATERIAL });

    // set to default values if no mappers
    let stages = if plant(13937) { MAX_STAGES - 1 } else { MAX_STAGES };
    for i in 0..stages {
        let i = if plant(13938) { stages - 1 - i } else { i };
        host.set_stage_passthru(i as i32);
    }
}
