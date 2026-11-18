//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rinfo.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rinfo.h

use super::handles::{CameraHandle, LightEnvironmentHandle, MaterialPassHandle, TexProjectHandle};

/// `MAX_ADDITIONAL_MATERIAL_PASSES` (rinfo.h).
pub const MAX_ADDITIONAL_MATERIAL_PASSES: usize = 32;
/// `MAX_OVERRIDE_FLAG_LEVEL` (rinfo.h).
pub const MAX_OVERRIDE_FLAG_LEVEL: usize = 32;

/// `RenderInfoClass::RINFO_OVERRIDE_FLAGS` (rinfo.h): an enum used as a bit set (`u32`).
pub type RinfoOverrideFlags = u32;
/// `RINFO_OVERRIDE_DEFAULT`: no overrides.
pub const RINFO_OVERRIDE_DEFAULT: RinfoOverrideFlags = 0x0000;
/// `RINFO_OVERRIDE_FORCE_TWO_SIDED`: force no backface culling.
pub const RINFO_OVERRIDE_FORCE_TWO_SIDED: RinfoOverrideFlags = 0x0001;
/// `RINFO_OVERRIDE_FORCE_SORTING`.
pub const RINFO_OVERRIDE_FORCE_SORTING: RinfoOverrideFlags = 0x0002;
/// `RINFO_OVERRIDE_ADDITIONAL_PASSES_ONLY`.
pub const RINFO_OVERRIDE_ADDITIONAL_PASSES_ONLY: RinfoOverrideFlags = 0x0004;
/// `RINFO_OVERRIDE_SHADOW_RENDERING`.
pub const RINFO_OVERRIDE_SHADOW_RENDERING: RinfoOverrideFlags = 0x0008;

/// `class RenderInfoClass` (rinfo.h): everything the scene needs to render itself.
#[derive(Clone, Debug)]
pub struct RenderInfoClass {
    pub camera: CameraHandle,
    pub fog_scale: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    /// "added for 'Generals' to allow variable alpha -MW".
    pub alpha_override: f32,
    /// "added for 'Generals' to allow variable alpha on additional render passes. -MW".
    pub material_pass_alpha_override: f32,
    /// "added for 'Generals' to allow variable emissive on additional render passes. -MW".
    pub material_pass_emissive_override: f32,
    pub light_environment: Option<LightEnvironmentHandle>,
    pub texture_projector: Option<TexProjectHandle>,
    additional_material_pass_array: [Option<MaterialPassHandle>; MAX_ADDITIONAL_MATERIAL_PASSES],
    additional_material_pass_count: usize,
    rejected_material_passes: usize,
    override_flag: [RinfoOverrideFlags; MAX_OVERRIDE_FLAG_LEVEL],
    override_flag_level: usize,
}

impl RenderInfoClass {
    /// `RenderInfoClass::RenderInfoClass(cam)` (rinfo.cpp:30). One entry on the override stack, the default.
    pub fn new(cam: CameraHandle) -> Self {
        let mut r = RenderInfoClass {
            camera: cam,
            fog_scale: 0.0,
            fog_start: 0.0,
            fog_end: 0.0,
            alpha_override: 1.0,
            material_pass_alpha_override: 1.0,
            material_pass_emissive_override: 1.0,
            light_environment: None,
            texture_projector: None,
            additional_material_pass_array: [None; MAX_ADDITIONAL_MATERIAL_PASSES],
            additional_material_pass_count: 0,
            rejected_material_passes: 0,
            override_flag: [RINFO_OVERRIDE_DEFAULT; MAX_OVERRIDE_FLAG_LEVEL],
            override_flag_level: 0,
        };
        r.override_flag[r.override_flag_level] = RINFO_OVERRIDE_DEFAULT;
        r
    }

    /// `RenderInfoClass::Push_Material_Pass` (rinfo.cpp:48): appends while
    /// `count < MAX_ADDITIONAL_MATERIAL_PASSES - 1` (so 31 fit, as upstream), else counts a rejection.
    pub fn push_material_pass(&mut self, matpass: Option<MaterialPassHandle>) {
        if self.additional_material_pass_count < MAX_ADDITIONAL_MATERIAL_PASSES - 1 {
            // matpass->Add_Ref() — not modelled
            self.additional_material_pass_array[self.additional_material_pass_count] = matpass;
            self.additional_material_pass_count += 1;
        } else {
            self.rejected_material_passes += 1;
        }
    }

    /// `RenderInfoClass::Pop_Material_Pass` (rinfo.cpp:63): pops the last pass, unless passes were rejected, in
    /// which case it only forgets one rejection.
    pub fn pop_material_pass(&mut self) {
        if self.rejected_material_passes == 0 {
            assert!(self.additional_material_pass_count > 0); // WWASSERT
            self.additional_material_pass_count -= 1;
            // mpass->Release_Ref() — not modelled
        } else {
            self.rejected_material_passes -= 1;
        }
    }

    /// `RenderInfoClass::Additional_Pass_Count` (rinfo.cpp:77).
    pub fn additional_pass_count(&self) -> usize {
        self.additional_material_pass_count
    }

    /// `RenderInfoClass::Peek_Additional_Pass` (rinfo.cpp:82): unchecked upstream; indexes the array.
    pub fn peek_additional_pass(&self, i: usize) -> Option<MaterialPassHandle> {
        self.additional_material_pass_array[i]
    }

    /// `RenderInfoClass::Push_Override_Flags` (rinfo.cpp:87).
    pub fn push_override_flags(&mut self, flg: RinfoOverrideFlags) {
        assert!(self.override_flag_level < MAX_OVERRIDE_FLAG_LEVEL); // WWASSERT
        self.override_flag_level += 1;
        self.override_flag[self.override_flag_level] = flg;
    }

    /// `RenderInfoClass::Pop_Override_Flags` (rinfo.cpp:94).
    pub fn pop_override_flags(&mut self) {
        assert!(self.override_flag_level > 0); // WWASSERT
        self.override_flag_level -= 1;
    }

    /// `RenderInfoClass::Current_Override_Flags` (rinfo.cpp:100): upstream returns a reference.
    pub fn current_override_flags(&mut self) -> &mut RinfoOverrideFlags {
        &mut self.override_flag[self.override_flag_level]
    }
}

/// `SpecialRenderInfoClass::RENDER_VIS` / `RENDER_SHADOW` (rinfo.h).
pub const RENDER_VIS: i32 = 0;
/// See [`RENDER_VIS`].
pub const RENDER_SHADOW: i32 = 1;

/// `class SpecialRenderInfoClass : public RenderInfoClass` (rinfo.h, rinfo.cpp:112).
#[derive(Clone, Debug)]
pub struct SpecialRenderInfoClass {
    pub base: RenderInfoClass,
    pub render_type: i32,
    /// `VisRasterizerClass *VisRasterizer` (index into the owner's rasterizers).
    pub vis_rasterizer: Option<usize>,
    /// `BWRenderClass *BWRenderer`.
    pub bw_renderer: Option<usize>,
}

impl SpecialRenderInfoClass {
    /// `SpecialRenderInfoClass::SpecialRenderInfoClass` (rinfo.cpp:112).
    pub fn new(cam: CameraHandle, render_type: i32) -> Self {
        SpecialRenderInfoClass { base: RenderInfoClass::new(cam), render_type, vis_rasterizer: None, bw_renderer: None }
    }
}
