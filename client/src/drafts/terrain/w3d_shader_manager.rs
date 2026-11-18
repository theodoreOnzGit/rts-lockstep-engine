//! UNTESTED DRAFT (#202, #204, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DShaderManager.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DShaderManager.cpp
//!   GeneralsMD/Code/GameEngine/Include/GameClient/CommandXlat.h (`FilterTypes`, `FilterModes`)
//!   GeneralsMD/Code/GameEngine/Include/Common/GameLOD.h (`ChipsetType`, `CpuType`)
//!
//! What this file carries: the data (the shader and filter type enums, the chipset table, the per-shader pass counts the
//! `init` of each shader variant registers, the "first variant that initialises wins" selection through
//! `MasterShaderList`/`MasterFilterList`), the chipset detection from the adapter identifier, the minimum requirement and
//! GPU class tests, the screen filters' state machines (fade, cross fade, motion blur) with the quads they draw as data,
//! the viewport quad, the shroud texture matrix, and the terrain noise texture matrices. The D3D state calls inside each
//! shader's `set(pass)` (texture stage states, render states, pixel and vertex shader constants, `SetTexture`) and
//! `reset()` are device-only: the `set` functions return the upstream success flag and a comment names what they set.
//! The device (D3D adapter, caps, render target surfaces, the shader files `shaders\*.pso`/`*.vso`, the asset manager) is
//! [`ShaderDevice`], and the singletons the filters read (`TheTacticalView`, `TheDisplay`, `TheGameLogic`) are parameters.
//!
//! Not translated: the `CPUDetectClass` and `Benchmark.h` internals (`testMinimumRequirements` reads them through
//! [`ShaderDevice`]; `GetCPUBenchTime`'s loop is translated, its timer is a closure) and the `DISABLE_PIXEL_SHADERS`
//! branch (the macro is `do_not_DISABLE_PIXEL_SHADERS`, so undefined).
//!
//! Notes on upstream behaviour kept as it is:
//! - `ScreenBWFilterDOT3` derives from `ScreenBWFilter` and shares its static fade state (`m_fadeFrames`,
//!   `m_fadeDirection`, `m_curFadeFrame`, `m_curFadeValue`).
//! - `ScreenCrossFadeFilter::updateFadeLevel` increments the frame after computing the value when turning off, unlike the
//!   black and white filter, which increments first.
//! - `ScreenMotionBlurFilter::postRender` uses `sqrt` (the double function) on a float expression and converts back.
//! - In `W3DShaderManager::getChipset`, a vendor match that finds no device id falls through to the pixel shader version
//!   test; the cached `m_currentVendor` is only assigned by a match.
//! - The ATI device id `0x4e44` is `DC_RADEON_9700`.
//! - `m_currentShaderPass` is not initialised by the constructor.

use super::w3d_common::TextureHandle;
use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D};
use rts_lockstep_engine::logic::terrain_visual::MAP_XY_FACTOR;

/// `W3DShaderManager::ShaderTypes`: any custom shaders (not going through W3D) go in here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ShaderTypes {
    /// `ST_INVALID`: invalid shader type.
    Invalid = 0,
    /// `ST_TERRAIN_BASE`: shader to apply base terrain texture only.
    TerrainBase,
    /// `ST_TERRAIN_BASE_NOISE1`: shader to apply base texture and cloud/noise 1.
    TerrainBaseNoise1,
    /// `ST_TERRAIN_BASE_NOISE2`: shader to apply base texture and cloud/noise 2.
    TerrainBaseNoise2,
    /// `ST_TERRAIN_BASE_NOISE12`: shader to apply base texture and both cloud/noise.
    TerrainBaseNoise12,
    /// `ST_SHROUD_TEXTURE`: shader to apply shroud texture projection.
    ShroudTexture,
    /// `ST_MASK_TEXTURE`: shader to apply alpha mask texture projection.
    MaskTexture,
    /// `ST_ROAD_BASE`.
    RoadBase,
    /// `ST_ROAD_BASE_NOISE1`.
    RoadBaseNoise1,
    /// `ST_ROAD_BASE_NOISE2`.
    RoadBaseNoise2,
    /// `ST_ROAD_BASE_NOISE12`.
    RoadBaseNoise12,
    /// `ST_CLOUD_TEXTURE`: shader to project clouds.
    CloudTexture,
    /// `ST_FLAT_TERRAIN_BASE`.
    FlatTerrainBase,
    /// `ST_FLAT_TERRAIN_BASE_NOISE1`.
    FlatTerrainBaseNoise1,
    /// `ST_FLAT_TERRAIN_BASE_NOISE2`.
    FlatTerrainBaseNoise2,
    /// `ST_FLAT_TERRAIN_BASE_NOISE12`.
    FlatTerrainBaseNoise12,
    /// `ST_FLAT_SHROUD_TEXTURE`.
    FlatShroudTexture,
    /// `ST_MAX`.
    Max,
}

/// `FilterTypes` (CommandXlat.h): the screen filter shaders that modify the rendered viewport after it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum FilterTypes {
    /// `FT_NULL_FILTER`.
    NullFilter = 0,
    /// `FT_VIEW_BW_FILTER`: filter to apply a black & white filter to the screen.
    ViewBwFilter,
    /// `FT_VIEW_MOTION_BLUR_FILTER`: filter to apply motion blur filter to screen.
    ViewMotionBlurFilter,
    /// `FT_VIEW_CROSSFADE`: filter to apply a cross blend between previous/current views.
    ViewCrossfade,
    /// `FT_VIEW_DEFAULT`: default filter mode for default filter.
    ViewDefault,
    /// `FT_MAX`.
    Max,
}

/// `FilterModes` (CommandXlat.h), as `Int`s (a pan factor is added to `FM_VIEW_MB_PAN_ALPHA`).
pub mod filter_modes {
    /// `FM_NULL_MODE`.
    pub const FM_NULL_MODE: i32 = 0;
    /// `FM_VIEW_BW_BLACK_AND_WHITE`: BW Filter to black & white.
    pub const FM_VIEW_BW_BLACK_AND_WHITE: i32 = 1;
    /// `FM_VIEW_BW_RED_AND_WHITE`.
    pub const FM_VIEW_BW_RED_AND_WHITE: i32 = 2;
    /// `FM_VIEW_BW_GREEN_AND_WHITE`.
    pub const FM_VIEW_BW_GREEN_AND_WHITE: i32 = 3;
    /// `FM_VIEW_CROSSFADE_CIRCLE`: fades from previous to current view using expanding circle.
    pub const FM_VIEW_CROSSFADE_CIRCLE: i32 = 4;
    /// `FM_VIEW_CROSSFADE_FB_MASK`: fades from previous to current using mask stored in framebuffer alpha.
    pub const FM_VIEW_CROSSFADE_FB_MASK: i32 = 5;
    /// `FM_VIEW_MB_IN_AND_OUT_ALPHA`.
    pub const FM_VIEW_MB_IN_AND_OUT_ALPHA: i32 = 6;
    /// `FM_VIEW_MB_IN_AND_OUT_SATURATE`.
    pub const FM_VIEW_MB_IN_AND_OUT_SATURATE: i32 = 7;
    /// `FM_VIEW_MB_IN_ALPHA`.
    pub const FM_VIEW_MB_IN_ALPHA: i32 = 8;
    /// `FM_VIEW_MB_OUT_ALPHA`.
    pub const FM_VIEW_MB_OUT_ALPHA: i32 = 9;
    /// `FM_VIEW_MB_IN_SATURATE`.
    pub const FM_VIEW_MB_IN_SATURATE: i32 = 10;
    /// `FM_VIEW_MB_OUT_SATURATE`.
    pub const FM_VIEW_MB_OUT_SATURATE: i32 = 11;
    /// `FM_VIEW_MB_END_PAN_ALPHA`: motion blur on screen pan (for camera tracks object mode).
    pub const FM_VIEW_MB_END_PAN_ALPHA: i32 = 12;
    /// `FM_VIEW_DEFAULT`: default filter that's enabled when all others are off.
    pub const FM_VIEW_DEFAULT: i32 = 13;
    /// `FM_VIEW_MB_PAN_ALPHA`: NOTE: this has to be the last entry in the enum.
    pub const FM_VIEW_MB_PAN_ALPHA: i32 = 14;
}
use filter_modes::*;

/// `CustomScenePassModes` (W3DCustomScene.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomScenePassModes {
    /// `SCENE_PASS_DEFAULT`.
    Default,
    /// `SCENE_PASS_ALPHA_MASK`.
    AlphaMask,
}

/// `CpuType` (GameLOD.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuType {
    /// `XX`: unknown type.
    Xx,
    /// `P3`.
    P3,
    /// `P4`.
    P4,
    /// `K7`.
    K7,
}

/// `ChipsetType` (GameLOD.h). Keep this in sync with `VideoNames` in Gamelod.cpp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum ChipsetType {
    /// `DC_UNKNOWN`.
    Unknown = 0,
    /// `DC_VOODOO2`.
    Voodoo2,
    /// `DC_VOODOO3`.
    Voodoo3,
    /// `DC_VOODOO4`.
    Voodoo4,
    /// `DC_VOODOO5`.
    Voodoo5,
    /// `DC_TNT`.
    Tnt,
    /// `DC_TNT2`.
    Tnt2,
    /// `DC_GEFORCE2`.
    GeForce2,
    /// `DC_RADEON`.
    Radeon,
    /// `DC_GENERIC_PIXEL_SHADER_1_1`.
    GenericPixelShader1_1,
    /// `DC_GEFORCE3`.
    GeForce3,
    /// `DC_GEFORCE4`.
    GeForce4,
    /// `DC_GENERIC_PIXEL_SHADER_1_4`.
    GenericPixelShader1_4,
    /// `DC_RADEON_8500`.
    Radeon8500,
    /// `DC_GENERIC_PIXEL_SHADER_2_0`.
    GenericPixelShader2_0,
    /// `DC_RADEON_9700`.
    Radeon9700,
    /// `DC_MAX`.
    Max,
}

/// `StaticGameLODLevel` as `getGPUPerformanceIndex` returns it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticGameLodLevel {
    /// `STATIC_GAME_LOD_LOW`.
    Low,
    /// `STATIC_GAME_LOD_HIGH`.
    High,
}

/// `GraphicsVenderID` (W3DShaderManager.cpp:2907).
pub const DC_NVIDIA_VENDOR_ID: u32 = 0x10DE;
/// `DC_3DFX_VENDOR_ID`.
pub const DC_3DFX_VENDOR_ID: u32 = 0x121A;
/// `DC_ATI_VENDOR_ID`.
pub const DC_ATI_VENDOR_ID: u32 = 0x1002;

/// `MIN_INTEL_CPU_FREQ`.
pub const MIN_INTEL_CPU_FREQ: i32 = 1300;
/// `MIN_AMD_CPU_FREQ`.
pub const MIN_AMD_CPU_FREQ: i32 = 1100;
/// `MIN_ACCEPTED_FREQUENCY`.
pub const MIN_ACCEPTED_FREQUENCY: i32 = 1300;
/// `MIN_ACCEPTED_MEMORY`: 256 MB.
pub const MIN_ACCEPTED_MEMORY: i32 = 1024 * 1024 * 256;
/// `MIN_ACCEPTED_TEXTURE_MEMORY`: 30 MB.
pub const MIN_ACCEPTED_TEXTURE_MEMORY: i32 = 1024 * 1024 * 30;

/// `D3DADAPTER_IDENTIFIER8` as `getChipset` reads it.
#[derive(Clone, Copy, Debug, Default)]
pub struct AdapterIdentifier {
    /// `VendorId`.
    pub vendor_id: u32,
    /// `DeviceId`.
    pub device_id: u32,
    /// `DriverVersion` (a `LARGE_INTEGER`).
    pub driver_version: i64,
}

/// What `CPUDetectClass` answers for `testMinimumRequirements`.
#[derive(Clone, Copy, Debug, Default)]
pub struct CpuInfo {
    /// `Get_Processor_Manufacturer() == MANUFACTURER_AMD`.
    pub is_amd: bool,
    /// `Get_AMD_Processor() >= AMD_PROCESSOR_ATHLON_025`.
    pub amd_at_least_athlon: bool,
    /// `Get_Processor_Manufacturer() == MANUFACTURER_INTEL`.
    pub is_intel: bool,
    /// `Get_Intel_Processor() >= INTEL_PROCESSOR_PENTIUM_III_MODEL_7`.
    pub intel_at_least_p3: bool,
    /// `Get_Intel_Processor() >= INTEL_PROCESSOR_PENTIUM4`.
    pub intel_at_least_p4: bool,
    /// `Get_Processor_Speed()`.
    pub speed: i32,
    /// `Get_Total_Physical_Memory()`.
    pub total_physical_memory: i32,
}

/// What the shaders and filters ask of the device and the game.
pub trait ShaderDevice {
    /// `TheGlobalData->m_chipSetType` (`DC_UNKNOWN` when `TheGlobalData` is NULL or it is unset).
    fn global_chipset_override(&self) -> ChipsetType;
    /// `d3d8Interface && _Get_D3D_Device8()` and `GetAdapterIdentifier(0, ..)`; `None` without an interface or device.
    fn adapter_identifier(&self) -> Option<AdapterIdentifier>;
    /// `Get_Current_Caps()->Get_Max_Simultaneous_Textures()`.
    fn max_simultaneous_textures(&self) -> i32;
    /// `Get_Pixel_Shader_Major_Version()` and `Get_Pixel_Shader_Minor_Version()`.
    fn pixel_shader_version(&self) -> (i32, i32);
    /// `LoadAndCreateD3DShader`'s device part (`CreateVertexShader` when `vertex`, else `CreatePixelShader`) of the file's
    /// bytes read through `TheFileSystem`; false on any failure.
    fn create_shader_from_file(&mut self, path: &[u8], vertex: bool) -> bool;
    /// `WW3DAssetManager::Get_Texture(name)` with clamp U/V and no mip mapping set.
    fn get_texture(&mut self, name: &[u8]) -> Option<TextureHandle>;
    /// `TheDisplay` exists.
    fn display_present(&self) -> bool;
    /// `TheTacticalView->getOrigin`, `getWidth`, `getHeight`: `(xpos, ypos, width, height)`.
    fn tactical_view(&self) -> (i32, i32, i32, i32);
    /// `TheDisplay->getWidth()`, `getHeight()`.
    fn display_size(&self) -> (i32, i32);
    /// `TheGlobalData->m_showSoftWaterEdge`.
    fn show_soft_water_edge(&self) -> bool;
    /// `TheWaterTransparency->m_minWaterOpacity`.
    fn min_water_opacity(&self) -> f32;
    /// `CPUDetectClass` and the physical memory.
    fn cpu_info(&self) -> CpuInfo;
    /// `RunBenchmark(0, NULL, &float, &int, &mem)`: `(floatBenchIndex, intBenchIndex, memBenchIndex)`.
    fn run_benchmark(&mut self) -> (f32, f32, f32);
}

/// `m_renderTexture` and the surfaces around it (`init` makes them, `shutdown` releases them).
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderTargets {
    /// `m_oldRenderSurface` is set.
    pub old_render_surface: bool,
    /// `m_renderTexture` is set.
    pub render_texture: bool,
    /// `m_newRenderSurface` is set.
    pub new_render_surface: bool,
    /// `m_oldDepthSurface` is set.
    pub old_depth_surface: bool,
    /// `m_renderTexture`'s level 0 description `(Width, Height)` (the back buffer's size).
    pub texture_size: (i32, i32),
}

/// `_TRANS_LIT_TEX_VERTEX` of the filter quads: `D3DFVF_XYZRHW | D3DFVF_DIFFUSE | D3DFVF_TEX1` (or `TEX2`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FilterQuadVertex {
    /// `p` (x, y, 0, 1) when the format has `XYZRHW` positions (the `ScreenBWFilterDOT3` and cross fade quads have none).
    pub p: Option<[f32; 4]>,
    /// `color`.
    pub color: u32,
    /// `u`.
    pub u: f32,
    /// `v`.
    pub v: f32,
    /// `u1` (the cross fade's second texture).
    pub u1: f32,
    /// `v1`.
    pub v1: f32,
}

/// One `DrawPrimitiveUP(D3DPT_TRIANGLESTRIP, 2, v, ..)` of a filter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterQuad {
    /// The four vertices.
    pub v: [FilterQuadVertex; 4],
    /// `D3DRS_ALPHABLENDENABLE` during this draw (motion blur toggles it), `None` when not set around the draw.
    pub alpha_blend: Option<bool>,
}

/// The result of a filter's `postRender`, with everything it asks the device to draw, in order.
#[derive(Clone, Debug, Default)]
pub struct FilterPostRender {
    /// The returned `Bool`.
    pub result: bool,
    /// `doExtraRender`.
    pub do_extra_render: bool,
    /// The quads drawn.
    pub quads: Vec<FilterQuad>,
    /// `TheTacticalView->setViewFilterMode(FM_NULL_MODE)` and `setViewFilter(FT_NULL_FILTER)` were called.
    pub reset_view_filter: bool,
    /// `TheTacticalView->lookAt(&m_zoomToPos)` was called with this position.
    pub look_at: Option<Coord3D>,
    /// For the black and white pixel shader: the constants set (`c1` colour tint, `c2` fade value).
    pub bw_constants: Option<([f32; 4], f32)>,
}

/// The four view corners of the filter quads: `(xpos+width, ypos+height)`, `(xpos+width, ypos)`, `(xpos, ypos+height)`,
/// `(xpos, ypos)` as `(position, uv)`, with the half texel shift `0.5f` of the `XYZRHW` format when `half_texel`.
fn view_quad_corners(device: &impl ShaderDevice, half_texel: bool) -> [([f32; 4], f32, f32); 4] {
    let (xpos, ypos, width, height) = device.tactical_view();
    let (dw, dh) = device.display_size();
    let (dw, dh) = (dw as f32, dh as f32);
    let h = if half_texel { 0.5 } else { 0.0 };
    [
        // bottom right
        ([xpos as f32 + width as f32 - h, ypos as f32 + height as f32 - h, 0.0, 1.0], (xpos + width) as f32 / dw, (ypos + height) as f32 / dh),
        // top right
        ([xpos as f32 + width as f32 - h, ypos as f32 - h, 0.0, 1.0], (xpos + width) as f32 / dw, ypos as f32 / dh),
        // bottom left
        ([xpos as f32 - h, ypos as f32 + height as f32 - h, 0.0, 1.0], xpos as f32 / dw, (ypos + height) as f32 / dh),
        // top left
        ([xpos as f32 - h, ypos as f32 - h, 0.0, 1.0], xpos as f32 / dw, ypos as f32 / dh),
    ]
}

/// The viewport quad most filters draw with colour `color` (`ScreenDefaultFilter::postRender` and the black and white and
/// motion blur filters use white).
fn viewport_quad(device: &impl ShaderDevice, color: u32, with_position: bool) -> FilterQuad {
    let c = view_quad_corners(device, true);
    let mut v = [FilterQuadVertex::default(); 4];
    for i in 0..4 {
        v[i] = FilterQuadVertex { p: if with_position { Some(c[i].0) } else { None }, color, u: c[i].1, v: c[i].2, u1: 0.0, v1: 0.0 };
    }
    FilterQuad { v, alpha_blend: None }
}

/// The `ScreenBWFilter` fade state (statics shared with `ScreenBWFilterDOT3`).
#[derive(Clone, Copy, Debug, Default)]
pub struct BwFadeState {
    /// `m_fadeFrames`.
    pub fade_frames: i32,
    /// `m_fadeDirection`.
    pub fade_direction: i32,
    /// `m_curFadeFrame`.
    pub cur_fade_frame: i32,
    /// `m_curFadeValue`.
    pub cur_fade_value: f32,
}

/// The `ScreenCrossFadeFilter` state (statics).
#[derive(Clone, Debug, Default)]
pub struct CrossFadeState {
    /// `m_fadeFrames`.
    pub fade_frames: i32,
    /// `m_fadeDirection`.
    pub fade_direction: i32,
    /// `m_curFadeFrame`.
    pub cur_fade_frame: i32,
    /// `m_curFadeValue`.
    pub cur_fade_value: f32,
    /// `m_skipRender`.
    pub skip_render: bool,
    /// `m_fadePatternTexture` ("exmask_g.tga").
    pub fade_pattern_texture: Option<TextureHandle>,
}

/// `ScreenMotionBlurFilter`'s constants.
pub const MB_MAX_COUNT: i32 = 60;
/// `MAX_LIMIT`.
pub const MB_MAX_LIMIT: i32 = 30;
/// `COUNT_STEP`.
pub const MB_COUNT_STEP: i32 = 5;
/// `DEFAULT_PAN_FACTOR`.
pub const MB_DEFAULT_PAN_FACTOR: i32 = 30;

/// `ScreenMotionBlurFilter`'s members and statics.
#[derive(Clone, Debug, Default)]
pub struct MotionBlurState {
    /// `m_maxCount`.
    pub max_count: i32,
    /// `m_lastFrame`.
    pub last_frame: i32,
    /// `m_decrement`.
    pub decrement: bool,
    /// `m_skipRender`.
    pub skip_render: bool,
    /// `m_additive`.
    pub additive: bool,
    /// `m_doZoomTo`.
    pub do_zoom_to: bool,
    /// `m_priorDelta`.
    pub prior_delta: Coord2D,
    /// `m_panFactor`.
    pub pan_factor: i32,
    /// `m_zoomToPos` (static).
    pub zoom_to_pos: Coord3D,
    /// `m_zoomToValid` (static).
    pub zoom_to_valid: bool,
}

/// The terrain 2 stage shader's cloud scroll state (`TerrainShader2Stage`).
#[derive(Clone, Copy, Debug, Default)]
pub struct TerrainShader2StageState {
    /// `m_xSlidePerSecond`: how far the clouds move per second.
    pub x_slide_per_second: f32,
    /// `m_ySlidePerSecond`.
    pub y_slide_per_second: f32,
    /// `m_curTick`.
    pub cur_tick: i32,
    /// `m_xOffset`.
    pub x_offset: f32,
    /// `m_yOffset`.
    pub y_offset: f32,
}

/// A `D3DXMATRIX` (row major).
pub type D3dxMatrix = [[f32; 4]; 4];

/// `D3DXMatrixMultiply(&out, &a, &b)` (`a * b`): `out[i][j] = sum_k a[i][k] * b[k][j]`.
pub fn d3dx_matrix_multiply(a: &D3dxMatrix, b: &D3dxMatrix) -> D3dxMatrix {
    let mut out = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            out[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
        }
    }
    out
}

/// `D3DXMatrixScaling(&out, sx, sy, sz)`.
pub fn d3dx_matrix_scaling(sx: f32, sy: f32, sz: f32) -> D3dxMatrix {
    [[sx, 0.0, 0.0, 0.0], [0.0, sy, 0.0, 0.0], [0.0, 0.0, sz, 0.0], [0.0, 0.0, 0.0, 1.0]]
}

/// `D3DXMatrixTranslation(&out, x, y, z)`.
pub fn d3dx_matrix_translation(x: f32, y: f32, z: f32) -> D3dxMatrix {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [x, y, z, 1.0]]
}

/// `STRETCH_FACTOR`: `(float)(1/(63.0*MAP_XY_FACTOR/2))`, covers 63/2 tiles.
fn stretch_factor() -> f32 {
    (1.0f64 / (63.0 * MAP_XY_FACTOR as f64 / 2.0)) as f32
}

impl TerrainShader2StageState {
    /// `TerrainShader2Stage::init` (W3DShaderManager.cpp:1566): the cloud scroll speeds and start time (`WW3D::Get_Sync_Time()`).
    pub fn new(sync_time: i32) -> Self {
        let x = -0.02f32;
        TerrainShader2StageState { x_slide_per_second: x, y_slide_per_second: 1.50 * x, cur_tick: sync_time, x_offset: 0.0, y_offset: 0.0 }
    }

    /// `TerrainShader2Stage::updateNoise1` (W3DShaderManager.cpp:1605): generate the uv coordinates for Noise1 (i.e
    /// clouds): the view inverse scaled, then offset by the scrolling cloud position.
    pub fn update_noise1(&mut self, cur_view_inverse: &D3dxMatrix, sync_time: i32) -> D3dxMatrix {
        let scale = d3dx_matrix_scaling(stretch_factor(), stretch_factor(), 1.0);
        let mut dest = d3dx_matrix_multiply(cur_view_inverse, &scale);

        let mut delta = self.cur_tick;
        self.cur_tick = sync_time;
        delta = self.cur_tick - delta;
        self.x_offset += self.x_slide_per_second * delta as f32 / 1000.0;
        self.y_offset += self.y_slide_per_second * delta as f32 / 1000.0;

        while self.x_offset > 1.0 {
            self.x_offset -= 1.0;
        }
        while self.y_offset > 1.0 {
            self.y_offset -= 1.0;
        }
        while self.x_offset < -1.0 {
            self.x_offset += 1.0;
        }
        while self.y_offset < -1.0 {
            self.y_offset += 1.0;
        }

        let offset = d3dx_matrix_translation(self.x_offset, self.y_offset, 0.0);
        dest = d3dx_matrix_multiply(&dest, &offset);
        dest
    }
}

/// `TerrainShader2Stage::updateNoise2` (W3DShaderManager.cpp:1639): generate the uv coordinates for Noise2 (i.e lightmap).
pub fn terrain_shader_update_noise2(cur_view_inverse: &D3dxMatrix) -> D3dxMatrix {
    let scale = d3dx_matrix_scaling(stretch_factor(), stretch_factor(), 1.0);
    d3dx_matrix_multiply(cur_view_inverse, &scale)
}

/// The shroud texture coordinate transform of `W3DShaderManager::setShroudTex` (and the two shroud shaders): the view
/// inverse, translated by the draw origin (shifted by one cell for the unused border texels), scaled to texture space.
/// `shroud` is `(cellWidth, cellHeight, drawOriginX, drawOriginY, textureWidth, textureHeight)`; `map_present` is
/// `TheTerrainRenderObject->getMap() != NULL`.
pub fn shroud_texture_transform(view_inverse: &D3dxMatrix, shroud: (f32, f32, f32, f32, f32, f32), map_present: bool) -> D3dxMatrix {
    let (mut width, mut height, origin_x, origin_y, tex_w, tex_h) = shroud;
    let mut xoffset: f32 = 0.0;
    let mut yoffset: f32 = 0.0;
    if map_present {
        // subtract origin position from all coordinates. Origin is shifted by 1 cell width/height to allow for unused
        // border texels.
        xoffset = -origin_x + width;
        yoffset = -origin_y + height;
    }
    let offset = d3dx_matrix_translation(xoffset, yoffset, 0.0);
    width = 1.0 / (width * tex_w);
    height = 1.0 / (height * tex_h);
    let scale = d3dx_matrix_scaling(width, height, 1.0);
    d3dx_matrix_multiply(&d3dx_matrix_multiply(view_inverse, &offset), &scale)
}

/// The custom shader implementations (`W3DShaderInterface` subclasses), by variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderImpl {
    /// `TerrainShaderPixelShader`: pixel shader based terrain shader - fastest method for the newest cards.
    TerrainPixelShader,
    /// `TerrainShader8Stage`: 8 stage terrain shader which only works on certain Nvidia cards.
    Terrain8Stage,
    /// `TerrainShader2Stage`: regular terrain shader that should work on all multi-texture video cards (slowest version).
    Terrain2Stage,
    /// `ShroudTextureShader`.
    ShroudTexture,
    /// `FlatShroudTextureShader`.
    FlatShroudTexture,
    /// `MaskTextureShader`.
    MaskTexture,
    /// `RoadShaderPixelShader`.
    RoadPixelShader,
    /// `RoadShader2Stage`.
    Road2Stage,
    /// `CloudTextureShader`.
    CloudTexture,
    /// `FlatTerrainShaderPixelShader`.
    FlatTerrainPixelShader,
    /// `FlatTerrainShader2Stage`.
    FlatTerrain2Stage,
}

/// The custom filter implementations (`W3DFilterInterface` subclasses), by variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterImpl {
    /// `ScreenDefaultFilter`.
    Default,
    /// `ScreenBWFilter`.
    Bw,
    /// `ScreenBWFilterDOT3`: slower version for older cards without pixel shaders.
    BwDot3,
    /// `ScreenCrossFadeFilter`.
    CrossFade,
    /// `ScreenMotionBlurFilter`.
    MotionBlur,
}

/// The lists in `MasterShaderList` order: each list is the variants of one shader in order of preference.
pub const MASTER_SHADER_LIST: [&[ShaderImpl]; 7] = [
    &[ShaderImpl::TerrainPixelShader, ShaderImpl::Terrain8Stage, ShaderImpl::Terrain2Stage], // TerrainShaderList
    &[ShaderImpl::ShroudTexture],                                                            // ShroudShaderList
    &[ShaderImpl::FlatShroudTexture],                                                        // FlatShroudShaderList
    &[ShaderImpl::RoadPixelShader, ShaderImpl::Road2Stage],                                  // RoadShaderList
    &[ShaderImpl::MaskTexture],                                                              // MaskShaderList
    &[ShaderImpl::CloudTexture],                                                             // CloudShaderList
    &[ShaderImpl::FlatTerrainPixelShader, ShaderImpl::FlatTerrain2Stage],                    // FlatTerrainShaderList
];

/// The lists in `MasterFilterList` order.
pub const MASTER_FILTER_LIST: [&[FilterImpl]; 4] = [
    &[FilterImpl::Default],                      // ScreenDefaultFilterList
    &[FilterImpl::Bw, FilterImpl::BwDot3],       // ScreenBWFilterList
    &[FilterImpl::MotionBlur],                   // ScreenMotionBlurFilterList
    &[FilterImpl::CrossFade],                    // ScreenCrossFadeFilterList
];

/// `class W3DShaderManager` with the file-static tables (`W3DShaders`, `W3DShadersPassCount`, `W3DFilters`) and the
/// filters' and shaders' own statics.
pub struct W3DShaderManager {
    /// `W3DShaders[ST_MAX]`.
    pub w3d_shaders: Vec<Option<ShaderImpl>>,
    /// `W3DShadersPassCount[ST_MAX]`: number of passes for each of the above shaders.
    pub w3d_shaders_pass_count: Vec<i32>,
    /// `W3DFilters[FT_MAX]`.
    pub w3d_filters: Vec<Option<FilterImpl>>,
    /// `m_Textures[8]`: textures assigned to each of the possible stages.
    pub textures: [Option<TextureHandle>; 8],
    /// `m_currentChipset`: last video card chipset that was detected.
    pub current_chipset: ChipsetType,
    /// `m_currentVendor`: last video card vendor.
    pub current_vendor: u32,
    /// `m_driverVersion`: driver version of last chipset.
    pub driver_version: i64,
    /// `m_currentShader`: last shader that was set.
    pub current_shader: ShaderTypes,
    /// `m_currentShaderPass`: pass of last shader that was set.
    pub current_shader_pass: i32,
    /// `m_currentFilter`: last filter that was set.
    pub current_filter: FilterTypes,
    /// `m_renderingToTexture`.
    pub rendering_to_texture: bool,
    /// The render target surfaces.
    pub render_targets: RenderTargets,
    /// The pixel shader handles each shader variant created (`m_dw*PixelShader`), by variant.
    pub pixel_shaders_created: Vec<ShaderImpl>,
    /// `ScreenBWFilter`'s pixel shader (`m_dwBWPixelShader`) exists.
    pub bw_pixel_shader: bool,
    /// The black and white fade (statics shared by both black and white filters).
    pub bw: BwFadeState,
    /// The cross fade filter state.
    pub cross_fade: CrossFadeState,
    /// The motion blur filter state.
    pub motion_blur: MotionBlurState,
    /// The terrain 2 stage shader state.
    pub terrain_2stage: TerrainShader2StageState,
}

impl Default for W3DShaderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DShaderManager {
    /// `W3DShaderManager::W3DShaderManager` (W3DShaderManager.cpp:2583): just clears some variables.
    pub fn new() -> Self {
        W3DShaderManager {
            w3d_shaders: vec![None; ShaderTypes::Max as usize],
            w3d_shaders_pass_count: vec![0; ShaderTypes::Max as usize],
            w3d_filters: vec![None; FilterTypes::Max as usize],
            textures: [None; 8],
            current_chipset: ChipsetType::Unknown,
            current_vendor: 0,
            driver_version: 0,
            current_shader: ShaderTypes::Invalid, // (then assigned -1 upstream: `(ShaderTypes)-1`; both mean "none set")
            current_shader_pass: 0,
            current_filter: FilterTypes::NullFilter,
            rendering_to_texture: false,
            render_targets: RenderTargets::default(),
            pixel_shaders_created: Vec::new(),
            bw_pixel_shader: false,
            bw: BwFadeState::default(),
            cross_fade: CrossFadeState::default(),
            motion_blur: MotionBlurState::default(),
            terrain_2stage: TerrainShader2StageState::default(),
        }
    }

    /// `W3DShaderManager::setTexture` (W3DShaderManager.h): specify all textures (up to 8) which can be accessed by the
    /// shaders.
    pub fn set_texture(&mut self, stage: usize, texture: Option<TextureHandle>) {
        self.textures[stage] = texture;
    }

    /// `W3DShaderManager::getShaderTexture`: returns currently selected texture for given stage.
    pub fn get_shader_texture(&self, stage: usize) -> Option<TextureHandle> {
        self.textures[stage]
    }

    /// `W3DShaderManager::getCurrentShader`: last activated shader.
    pub fn get_current_shader(&self) -> ShaderTypes {
        self.current_shader
    }

    /// `W3DShaderManager::getCurrentVendor`.
    pub fn get_current_vendor(&self) -> u32 {
        self.current_vendor
    }

    /// `W3DShaderManager::getCurrentDriverVersion`.
    pub fn get_current_driver_version(&self) -> i64 {
        self.driver_version
    }

    /// `W3DShaderManager::canRenderToTexture` (W3DShaderManager.h): `m_oldRenderSurface && m_newRenderSurface`.
    pub fn can_render_to_texture(&self) -> bool {
        self.render_targets.old_render_surface && self.render_targets.new_render_surface
    }

    /// `W3DShaderManager::isRenderingToTexture`.
    pub fn is_rendering_to_texture(&self) -> bool {
        self.rendering_to_texture
    }

    /// `W3DShaderManager::getRenderTexture` (W3DShaderManager.cpp:2902): returns the texture containing the image that was
    /// last rendered using any of the effects requiring render target textures. NOTE: it does not survive device reset.
    pub fn get_render_texture(&self) -> bool {
        self.render_targets.render_texture
    }

    /// `W3DShaderManager::getChipset` (W3DShaderManager.cpp:2919): returns the chipset used by the currently active
    /// rendering device. Can be useful for coding around specific driver bugs.
    pub fn get_chipset(&mut self, device: &impl ShaderDevice) -> ChipsetType {
        if device.global_chipset_override() != ChipsetType::Unknown {
            return device.global_chipset_override();
        }

        let mut chip = ChipsetType::Unknown;

        if let Some(did) = device.adapter_identifier() {
            self.driver_version = did.driver_version;

            if did.vendor_id == DC_NVIDIA_VENDOR_ID {
                self.current_vendor = DC_NVIDIA_VENDOR_ID;

                if did.device_id == 0x20 {
                    return ChipsetType::Tnt;
                }
                if did.device_id >= 0x28 && did.device_id < 0x100 {
                    return ChipsetType::Tnt2;
                }
                if (did.device_id >= 0x100 && did.device_id <= 0x103) // GeForce
                    || (did.device_id >= 0x110 && did.device_id <= 0x113) // GeForce2 MX
                    || (did.device_id >= 0x150 && did.device_id <= 0x153)
                // GeForce2
                {
                    return ChipsetType::GeForce2;
                }
                if did.device_id >= 0x200 && did.device_id < 0x250 {
                    return ChipsetType::GeForce3;
                }
                if did.device_id >= 0x250 {
                    return ChipsetType::GeForce4;
                }
            } else if did.vendor_id == DC_3DFX_VENDOR_ID {
                self.current_vendor = DC_3DFX_VENDOR_ID;

                if did.device_id == 0x0002 {
                    return ChipsetType::Voodoo2;
                }
                if did.device_id == 0x0005 {
                    return ChipsetType::Voodoo3;
                }
                if did.device_id == 0x0008 {
                    // @todo: Just guessing on this one - find actual Voodoo4 deviceID.
                    return ChipsetType::Voodoo4;
                }
                if did.device_id == 0x0009 {
                    return ChipsetType::Voodoo5;
                }
            } else if did.vendor_id == DC_ATI_VENDOR_ID {
                self.current_vendor = DC_ATI_VENDOR_ID;

                if did.device_id == 0x5144 {
                    return ChipsetType::Radeon;
                }
                if did.device_id == 0x514C {
                    return ChipsetType::Radeon8500;
                }
                if did.device_id == 0x4e44 {
                    return ChipsetType::Radeon9700;
                }
            }

            let max_textures = device.max_simultaneous_textures();
            // sprintf "%d.%d" then sscanf "%f"
            let (major, minor) = device.pixel_shader_version();
            let pixel_shader_version: f32 = format!("{}.{}", major, minor).parse::<f32>().unwrap_or(0.0);

            if max_textures >= 4 {
                if pixel_shader_version >= 1.1 {
                    chip = ChipsetType::GenericPixelShader1_1;
                }
                if pixel_shader_version >= 1.4 {
                    chip = ChipsetType::GenericPixelShader1_4;
                }
                if max_textures >= 8 && pixel_shader_version >= 2.0 {
                    chip = ChipsetType::GenericPixelShader2_0;
                }
            }
        } // D3D8 interface and device exist.

        chip
    }

    /// `W3DShaderManager::LoadAndCreateD3DShader` (W3DShaderManager.cpp:3012): loads and creates a D3D pixel or vertex
    /// shader. `true` is `S_OK`. Hardware that can't handle shaders (below `DC_GENERIC_PIXEL_SHADER_1_1`) fails.
    pub fn load_and_create_d3d_shader(&mut self, device: &mut impl ShaderDevice, path: &[u8], vertex: bool) -> bool {
        if self.get_chipset(device) < ChipsetType::GenericPixelShader1_1 {
            return false; // don't allow loading any shaders if hardware can't handle it.
        }
        device.create_shader_from_file(path, vertex)
    }

    /// One shader variant's `init()` (the per-class `init` functions of W3DShaderManager.cpp): registers the shader and its
    /// pass count in the tables and returns whether it initialised. `sync_time` is `WW3D::Get_Sync_Time()`.
    fn init_shader(&mut self, which: ShaderImpl, device: &mut impl ShaderDevice, sync_time: i32) -> bool {
        use ShaderTypes as S;
        let mut reg = |m: &mut W3DShaderManager, t: ShaderTypes, s: ShaderImpl, passes: i32| {
            m.w3d_shaders[t as usize] = Some(s);
            m.w3d_shaders_pass_count[t as usize] = passes;
        };
        match which {
            ShaderImpl::Terrain2Stage => {
                // TerrainShader2Stage::init (W3DShaderManager.cpp:1566)
                self.terrain_2stage = TerrainShader2StageState::new(sync_time);
                reg(self, S::TerrainBase, which, 2);
                reg(self, S::TerrainBaseNoise1, which, 3);
                reg(self, S::TerrainBaseNoise2, which, 3);
                reg(self, S::TerrainBaseNoise12, which, 3);
                true
            }
            ShaderImpl::Terrain8Stage => {
                // TerrainShader8Stage::init (W3DShaderManager.cpp:1796)
                let ok2 = self.init_shader(ShaderImpl::Terrain2Stage, device, sync_time);
                let res = self.get_chipset(device);
                if ok2 && res >= ChipsetType::Tnt && res <= ChipsetType::GeForce2 {
                    reg(self, S::TerrainBase, which, 1);
                    reg(self, S::TerrainBaseNoise1, which, 2);
                    reg(self, S::TerrainBaseNoise2, which, 2);
                    reg(self, S::TerrainBaseNoise12, which, 2);
                    return true;
                }
                false
            }
            ShaderImpl::TerrainPixelShader => {
                // TerrainShaderPixelShader::init (W3DShaderManager.cpp:1966)
                let ok2 = self.init_shader(ShaderImpl::Terrain2Stage, device, sync_time);
                let res = self.get_chipset(device);
                if ok2 && res >= ChipsetType::GenericPixelShader1_1 {
                    // declaration: stream 0, position FLOAT3, diffuse D3DCOLOR, 2 x texture coordinates FLOAT2
                    if !self.load_and_create_d3d_shader(device, b"shaders\\terrain.pso", false) {
                        return false;
                    }
                    if !self.load_and_create_d3d_shader(device, b"shaders\\terrainnoise.pso", false) {
                        return false;
                    }
                    if !self.load_and_create_d3d_shader(device, b"shaders\\terrainnoise2.pso", false) {
                        return false;
                    }
                    for t in [S::TerrainBase, S::TerrainBaseNoise1, S::TerrainBaseNoise2, S::TerrainBaseNoise12] {
                        self.w3d_shaders[t as usize] = Some(which);
                    }
                    for t in [S::TerrainBase, S::TerrainBaseNoise1, S::TerrainBaseNoise2, S::TerrainBaseNoise12] {
                        self.w3d_shaders_pass_count[t as usize] = 1;
                    }
                    self.pixel_shaders_created.push(which);
                    return true;
                }
                false
            }
            ShaderImpl::ShroudTexture => {
                // ShroudTextureShader::init (W3DShaderManager.cpp:1202)
                reg(self, S::ShroudTexture, which, 1);
                true
            }
            ShaderImpl::FlatShroudTexture => {
                // FlatShroudTextureShader::init (W3DShaderManager.cpp:1303)
                reg(self, S::FlatShroudTexture, which, 1);
                true
            }
            ShaderImpl::MaskTexture => {
                // MaskTextureShader::init (W3DShaderManager.cpp:1394)
                reg(self, S::MaskTexture, which, 1);
                true
            }
            ShaderImpl::CloudTexture => {
                // CloudTextureShader::init (W3DShaderManager.cpp:2166)
                reg(self, S::CloudTexture, which, 1);
                true
            }
            ShaderImpl::Road2Stage => {
                // RoadShader2Stage::init (W3DShaderManager.cpp:2383)
                reg(self, S::RoadBase, which, 1);
                reg(self, S::RoadBaseNoise1, which, 1);
                reg(self, S::RoadBaseNoise2, which, 1);
                reg(self, S::RoadBaseNoise12, which, 2);
                true
            }
            ShaderImpl::RoadPixelShader => {
                // RoadShaderPixelShader::init (W3DShaderManager.cpp:2260)
                let ok2 = self.init_shader(ShaderImpl::Road2Stage, device, sync_time);
                let res = self.get_chipset(device);
                if ok2 && res >= ChipsetType::GenericPixelShader1_1 {
                    // declaration: stream 0, position FLOAT3, diffuse D3DCOLOR, texture coordinates FLOAT2
                    if !self.load_and_create_d3d_shader(device, b"shaders\\roadnoise2.pso", false) {
                        return false;
                    }
                    reg(self, S::RoadBaseNoise12, which, 1);
                    self.pixel_shaders_created.push(which);
                    return true;
                }
                false
            }
            ShaderImpl::FlatTerrain2Stage => {
                // FlatTerrainShader2Stage::init (W3DShaderManager.cpp:3247)
                reg(self, S::FlatTerrainBase, which, 1);
                reg(self, S::FlatTerrainBaseNoise1, which, 2);
                reg(self, S::FlatTerrainBaseNoise2, which, 2);
                reg(self, S::FlatTerrainBaseNoise12, which, 2);
                true
            }
            ShaderImpl::FlatTerrainPixelShader => {
                // FlatTerrainShaderPixelShader::init (W3DShaderManager.cpp:3494)
                let res = self.get_chipset(device);
                if res >= ChipsetType::GenericPixelShader1_1 {
                    // declaration: stream 0, position FLOAT3, diffuse D3DCOLOR, 2 x texture coordinates FLOAT2
                    if !self.load_and_create_d3d_shader(device, b"shaders\\fterrain.pso", false) {
                        return false;
                    }
                    if !self.load_and_create_d3d_shader(device, b"shaders\\fterrain0.pso", false) {
                        return false;
                    }
                    if !self.load_and_create_d3d_shader(device, b"shaders\\fterrainnoise.pso", false) {
                        return false;
                    }
                    if !self.load_and_create_d3d_shader(device, b"shaders\\fterrainnoise2.pso", false) {
                        return false;
                    }
                    for t in [S::FlatTerrainBase, S::FlatTerrainBaseNoise1, S::FlatTerrainBaseNoise2, S::FlatTerrainBaseNoise12] {
                        self.w3d_shaders[t as usize] = Some(which);
                        self.w3d_shaders_pass_count[t as usize] = 1;
                    }
                    self.pixel_shaders_created.push(which);
                    return true;
                }
                false
            }
        }
    }

    /// One filter variant's `init()`: registers the filter in `W3DFilters` and returns whether it initialised.
    fn init_filter(&mut self, which: FilterImpl, device: &mut impl ShaderDevice) -> bool {
        match which {
            FilterImpl::Default => {
                // ScreenDefaultFilter::init (W3DShaderManager.cpp:150)
                if !self.can_render_to_texture() {
                    // Have to be able to render to texture.
                    return false;
                }

                // Can render to texture, but we don't know if it can read and write to the same texture. Since there is no
                // D3D caps bit to tell you this, we will just hard-code some specific cards that we know should work.
                // (The driver version check against TheGameLODManager is commented out upstream.)
                let _res = self.get_chipset(device);
                self.w3d_filters[FilterTypes::ViewDefault as usize] = Some(which);
                true
            }
            FilterImpl::Bw => {
                // ScreenBWFilter::init (W3DShaderManager.cpp:283)
                self.bw_pixel_shader = false;
                self.bw.cur_fade_frame = 0;

                if !self.can_render_to_texture() {
                    // Have to be able to render to texture.
                    return false;
                }

                let res = self.get_chipset(device);
                if res != ChipsetType::Unknown {
                    if res >= ChipsetType::GenericPixelShader1_1 {
                        // this shader needs some assets that need to be loaded: the monochrome pixel shader
                        // (declaration: stream 0, position FLOAT3, diffuse D3DCOLOR, texture coordinates FLOAT2)
                        if !self.load_and_create_d3d_shader(device, b"shaders\\monochrome.pso", false) {
                            return false;
                        }
                        self.bw_pixel_shader = true;
                        self.w3d_filters[FilterTypes::ViewBwFilter as usize] = Some(which);
                        return true;
                    }
                }
                false
            }
            FilterImpl::BwDot3 => {
                // ScreenBWFilterDOT3::init (W3DShaderManager.cpp:494)
                self.bw.cur_fade_frame = 0;

                if !self.can_render_to_texture() {
                    return false;
                }

                let res = self.get_chipset(device);
                if res != ChipsetType::Unknown {
                    self.w3d_filters[FilterTypes::ViewBwFilter as usize] = Some(which);
                    return true;
                }
                false
            }
            FilterImpl::CrossFade => {
                // ScreenCrossFadeFilter::init (W3DShaderManager.cpp:691)
                if !device.display_present() {
                    return false; // effect is useless without a view so no point initializing for the WB, etc.
                }

                self.cross_fade.cur_fade_frame = 0;

                if !self.can_render_to_texture() {
                    return false;
                }

                self.cross_fade.fade_pattern_texture = device.get_texture(b"exmask_g.tga");
                if self.cross_fade.fade_pattern_texture.is_none() {
                    return false;
                }

                // (the pattern texture is set to clamp U/V with no mip mapping by the asset manager)
                self.w3d_filters[FilterTypes::ViewCrossfade as usize] = Some(which);
                true
            }
            FilterImpl::MotionBlur => {
                // ScreenMotionBlurFilter::init (W3DShaderManager.cpp:934)
                if !self.can_render_to_texture() {
                    return false;
                }
                self.w3d_filters[FilterTypes::ViewMotionBlurFilter as usize] = Some(which);
                true
            }
        }
    }

    /// `W3DShaderManager::init` (W3DShaderManager.cpp:2610): walk through all shaders and find versions suitable for
    /// current hardware. `render_targets` is what `GetRenderTarget`, `CreateTexture(.., D3DUSAGE_RENDERTARGET, ..)`,
    /// `GetSurfaceLevel` and `GetDepthStencilSurface` gave (each stage releases what the earlier ones made on failure,
    /// which is how the flags below are cut: a failed stage leaves later flags unset).
    pub fn init(&mut self, device: &mut impl ShaderDevice, sync_time: i32, render_targets: RenderTargets) {
        // For now, check & see if we are gf3 or higher on the food chain.
        let res = self.get_chipset(device);
        if res != ChipsetType::Unknown {
            self.current_chipset = res; // cache the current chipset.

            // Some of our effects require an offscreen render target, so try creating it here.
            let rt = render_targets;
            if !rt.render_texture {
                // CreateTexture failed
                self.render_targets = RenderTargets::default();
            } else if !rt.new_render_surface {
                // GetSurfaceLevel failed
                self.render_targets = RenderTargets { old_render_surface: rt.old_render_surface, render_texture: false, new_render_surface: false, old_depth_surface: false, texture_size: rt.texture_size };
            } else if !rt.old_depth_surface {
                // GetDepthStencilSurface failed
                self.render_targets = RenderTargets { old_render_surface: rt.old_render_surface, render_texture: false, new_render_surface: false, old_depth_surface: false, texture_size: rt.texture_size };
            } else {
                self.render_targets = rt;
            }
        }

        for list in MASTER_SHADER_LIST.iter() {
            for s in list.iter() {
                if self.init_shader(*s, device, sync_time) {
                    break; // found a working shader
                }
            }
        }

        for list in MASTER_FILTER_LIST.iter() {
            for f in list.iter() {
                if self.init_filter(*f, device) {
                    break; // found a working shader
                }
            }
        }

        // DEBUG_LOG(("ShaderManager ChipsetID %d\n", res));
    }

    /// `W3DShaderManager::shutdown` (W3DShaderManager.cpp:2684): any shaders which allocate resources will be allowed to
    /// free them.
    pub fn shutdown(&mut self) {
        self.render_targets = RenderTargets::default();
        self.current_shader = ShaderTypes::Invalid;
        self.current_filter = FilterTypes::NullFilter;

        // release any assets associated with a shader (vertex/pixel shaders, textures, etc.)
        self.pixel_shaders_created.clear();
        // for each W3DFilters[i]->shutdown(): ScreenBWFilter::shutdown deletes its pixel shader,
        // ScreenCrossFadeFilter::shutdown releases the fade pattern texture
        self.bw_pixel_shader = false;
        self.cross_fade.fade_pattern_texture = None;
    }

    /// `W3DShaderManager::getShaderPasses` (W3DShaderManager.cpp:2719): return number of rendering passes required to
    /// perform the desired shader on current hardware. App will need to re-render the polygons this many times to
    /// complete the effect.
    pub fn get_shader_passes(&self, shader: ShaderTypes) -> i32 {
        self.w3d_shaders_pass_count[shader as usize]
    }

    /// `W3DShaderManager::setShader` (W3DShaderManager.cpp:2729): must call this method before each rendering pass in
    /// order to perform proper D3D setup for each shader. Returns `None` when the shader is already set (upstream returns
    /// TRUE without doing anything), else `Some(true)` for a set-up shader (whose `set(pass)` is device-only) and
    /// `Some(false)` when there is no shader of that type.
    pub fn set_shader(&mut self, shader: ShaderTypes, pass: i32) -> Option<bool> {
        if shader == self.current_shader && pass == self.current_shader_pass {
            return None; // shader is already set
        }

        self.current_shader = shader;
        self.current_shader_pass = pass;

        if self.w3d_shaders[shader as usize].is_some() {
            // W3DShaders[shader]->set(pass): the terrain, road, shroud, mask and cloud D3D state set-up — device-only
            return Some(true);
        }
        Some(false)
    }

    /// `W3DShaderManager::resetShader` (W3DShaderManager.cpp:2746): must call this method after all polygons and rendering
    /// passes have been submitted. This method allows D3D to reset itself to a default state that doesn't conflict with the
    /// WW3D2 Shader system. Returns the shader whose `reset()` the device must run, if any.
    pub fn reset_shader(&mut self, shader: ShaderTypes) -> Option<ShaderImpl> {
        if self.current_shader == ShaderTypes::Invalid {
            return None; // last shader is already reset.
        }

        let r = self.w3d_shaders[shader as usize];
        self.current_shader = ShaderTypes::Invalid;
        r
    }

    /// `W3DShaderManager::filterPreRender` (W3DShaderManager.cpp:2758): call to view filter shaders before rendering
    /// starts. Returns `(result, skipRender, scenePassMode)`.
    pub fn filter_pre_render(&mut self, filter: FilterTypes, device: &impl ShaderDevice, game_logic_frame: i32) -> (bool, bool, CustomScenePassModes) {
        let _ = game_logic_frame;
        let mut skip_render = false;
        let mut scene_pass_mode = CustomScenePassModes::Default;
        if let Some(f) = self.w3d_filters[filter as usize] {
            let result = match f {
                FilterImpl::Default => {
                    // Right now this filter is only used for smudges, so don't bother if none are present (the caller
                    // checks `getSmudgeCountLastFrame() == 0` and then does not call this).
                    self.start_render_to_texture(device);
                    true
                }
                FilterImpl::Bw | FilterImpl::BwDot3 => {
                    skip_render = false;
                    self.start_render_to_texture(device);
                    true
                }
                FilterImpl::CrossFade => {
                    if self.cross_fade_update_fade_level() {
                        // if fade has not completed
                        self.start_render_to_texture(device);
                        scene_pass_mode = CustomScenePassModes::AlphaMask;
                        skip_render = false;
                        self.cross_fade.skip_render = true; // tell the postRender function not to draw into framebuffer yet.
                        true
                    } else {
                        true
                    }
                }
                FilterImpl::MotionBlur => {
                    skip_render = self.motion_blur.skip_render;
                    self.start_render_to_texture(device);
                    true
                }
            };
            if result {
                self.current_filter = filter;
            }
            return (result, skip_render, scene_pass_mode);
        }
        (false, skip_render, scene_pass_mode)
    }

    /// `W3DShaderManager::filterSetup` (W3DShaderManager.cpp:2787).
    pub fn filter_setup(&mut self, filter: FilterTypes, mode: i32) -> bool {
        if let Some(f) = self.w3d_filters[filter as usize] {
            return match f {
                FilterImpl::Bw | FilterImpl::BwDot3 => true,
                FilterImpl::CrossFade => true,
                FilterImpl::MotionBlur => self.motion_blur_setup(mode),
                FilterImpl::Default => true,
            };
        }
        false
    }

    /// `ScreenBWFilter::setFadeParameters` / `ScreenCrossFadeFilter::setFadeParameters` (W3DShaderManager.h).
    pub fn set_bw_fade_parameters(&mut self, fade_frames: i32, direction: i32) {
        self.bw.cur_fade_frame = 0;
        self.bw.fade_frames = fade_frames;
        self.bw.fade_direction = direction;
    }

    /// `ScreenCrossFadeFilter::setFadeParameters` (W3DShaderManager.h).
    pub fn set_cross_fade_parameters(&mut self, fade_frames: i32, direction: i32) {
        self.cross_fade.cur_fade_frame = 0;
        self.cross_fade.fade_frames = fade_frames;
        self.cross_fade.fade_direction = direction;
    }

    /// `ScreenCrossFadeFilter::getCurrentFadeValue`.
    pub fn get_cross_fade_current_fade_value(&self) -> f32 {
        self.cross_fade.cur_fade_value
    }

    /// `ScreenMotionBlurFilter::setZoomToPos` (W3DShaderManager.h).
    pub fn set_zoom_to_pos(&mut self, pos: &Coord3D) {
        self.motion_blur.zoom_to_pos = *pos;
        self.motion_blur.zoom_to_valid = true;
    }

    /// The fade update both black and white filters' `set` do (`ScreenBWFilter::set` / `ScreenBWFilterDOT3::set`): advances
    /// the fade. Returns true when the fade finished turning off (the view filter is then reset by the caller).
    fn bw_update_fade(&mut self) -> bool {
        let mut reset_view_filter = false;
        if self.bw.fade_direction > 0 {
            // turning effect on
            self.bw.cur_fade_frame += 1;
            let fade = self.bw.cur_fade_frame;
            if fade < self.bw.fade_frames {
                self.bw.cur_fade_value = fade as f32 / self.bw.fade_frames as f32;
            } else {
                self.bw.cur_fade_frame = 0;
                self.bw.cur_fade_value = 1.0;
                self.bw.fade_direction = 0;
            }
        } else if self.bw.fade_direction < 0 {
            // turning effect off
            self.bw.cur_fade_frame += 1;
            let fade = self.bw.cur_fade_frame;
            if fade < self.bw.fade_frames {
                self.bw.cur_fade_value = 1.0 - fade as f32 / self.bw.fade_frames as f32;
            } else {
                self.bw.cur_fade_value = 0.0;
                // TheTacticalView->setViewFilterMode(FM_NULL_MODE); setViewFilter(FT_NULL_FILTER)
                reset_view_filter = true;
                self.bw.cur_fade_frame = 0;
                self.bw.fade_direction = 0;
            }
        }
        reset_view_filter
    }

    /// `ScreenCrossFadeFilter::updateFadeLevel` (W3DShaderManager.cpp:715): updates the current state of the fade and
    /// returns true if not finished.
    pub fn cross_fade_update_fade_level(&mut self) -> bool {
        let c = &mut self.cross_fade;
        if c.fade_direction > 0 {
            // turning effect on
            c.cur_fade_frame += 1;
            let fade = c.cur_fade_frame;
            if fade < c.fade_frames {
                c.cur_fade_value = fade as f32 / c.fade_frames as f32;
            } else {
                c.cur_fade_frame = 0;
                c.cur_fade_value = 1.0;
                c.fade_direction = 0;
                return false;
            }
        } else if c.fade_direction < 0 {
            // turning effect off
            let fade = c.cur_fade_frame;
            if fade < c.fade_frames {
                c.cur_fade_value = 1.0 - fade as f32 / c.fade_frames as f32;
                c.cur_fade_frame += 1;
            } else {
                c.cur_fade_value = 0.0;
                // TheTacticalView->setViewFilterMode(FM_NULL_MODE); setViewFilter(FT_NULL_FILTER): see
                // `cross_fade_reset_view_filter`
                c.cur_fade_frame = 0;
                c.fade_direction = 0;
                return false;
            }
        }
        true
    }

    /// `ScreenMotionBlurFilter::setup` (W3DShaderManager.cpp:1114): called when the filter is started, one time before the
    /// first prerender.
    pub fn motion_blur_setup(&mut self, mode: i32) -> bool {
        let m = &mut self.motion_blur;
        m.additive = false;
        if mode == FM_VIEW_MB_IN_AND_OUT_SATURATE || mode == FM_VIEW_MB_IN_SATURATE || mode == FM_VIEW_MB_OUT_SATURATE {
            m.additive = true;
        }
        m.do_zoom_to = false;
        if mode == FM_VIEW_MB_IN_AND_OUT_SATURATE || mode == FM_VIEW_MB_IN_AND_OUT_ALPHA {
            m.do_zoom_to = true;
        }
        if mode >= FM_VIEW_MB_PAN_ALPHA {
            m.pan_factor = mode - FM_VIEW_MB_PAN_ALPHA;
            if m.pan_factor < 1 {
                m.pan_factor = MB_DEFAULT_PAN_FACTOR;
            }
        }
        m.skip_render = false;
        if mode != FM_VIEW_MB_END_PAN_ALPHA {
            m.max_count = 0;
        }
        m.decrement = false;
        m.skip_render = false;
        if mode == FM_VIEW_MB_OUT_SATURATE || mode == FM_VIEW_MB_OUT_ALPHA {
            m.max_count = MB_MAX_COUNT;
            m.decrement = true;
        }
        true
    }

    /// `W3DShaderManager::filterPostRender` (W3DShaderManager.cpp:2773) with the filters' `postRender` bodies: returns the
    /// quads each draws. `scroll_delta` is the camera scroll delta, `game_logic_frame` is `TheGameLogic->getFrame()`.
    pub fn filter_post_render(
        &mut self,
        filter: FilterTypes,
        mode: i32,
        scroll_delta: &Coord2D,
        game_logic_frame: i32,
        device: &impl ShaderDevice,
    ) -> FilterPostRender {
        let mut out = FilterPostRender::default();
        let f = match self.w3d_filters[filter as usize] {
            Some(f) => f,
            None => {
                self.current_filter = FilterTypes::NullFilter;
                return out;
            }
        };
        match f {
            FilterImpl::Default => {
                // ScreenDefaultFilter::postRender (W3DShaderManager.cpp:192)
                if !self.end_render_to_texture() {
                    return out;
                }
                // set(mode): preset PRELIT_DIFFUSE material, opaque shader, no texture, ZFUNC ALWAYS, ZWRITE off — device-only
                out.quads.push(viewport_quad(device, 0xffff_ffff, true));
                out.result = true;
            }
            FilterImpl::Bw => {
                // ScreenBWFilter::postRender (W3DShaderManager.cpp:331)
                if !self.end_render_to_texture() {
                    return out;
                }
                // set(mode) (ScreenBWFilter::set, W3DShaderManager.cpp:381)
                if mode <= FM_NULL_MODE {
                    return out;
                }
                out.reset_view_filter = self.bw_update_fade();
                // pixel shader constants: c0 = (0.3, 0.59, 0.11, 1), c1 = the tint, c2 = (fade, fade, fade, 1)
                let mut color = [1.0f32, 1.0, 1.0, 1.0]; // multiply color
                if mode == FM_VIEW_BW_BLACK_AND_WHITE {
                    // back & white mode
                    color[0] = 1.0;
                    color[1] = 1.0;
                    color[2] = 1.0;
                }
                if mode == FM_VIEW_BW_RED_AND_WHITE {
                    // red is on
                    color[0] = 1.0;
                    color[1] = 0.0;
                    color[2] = 0.0;
                }
                if mode == FM_VIEW_BW_GREEN_AND_WHITE {
                    color[0] = 0.0;
                    color[1] = 1.0;
                    color[2] = 0.0;
                }
                out.bw_constants = Some((color, self.bw.cur_fade_value));
                out.quads.push(viewport_quad(device, 0xffff_ffff, true));
                out.result = true;
            }
            FilterImpl::BwDot3 => {
                // ScreenBWFilterDOT3::postRender (W3DShaderManager.cpp:520)
                if !self.end_render_to_texture() {
                    return out;
                }
                if mode <= FM_NULL_MODE {
                    return out;
                }
                out.reset_view_filter = self.bw_update_fade();
                // vertices have no position (D3DFVF_XYZRHW in the vertex shader flags, but the struct holds only
                // color, u, v: upstream's struct omits `p`)
                let current_fade: u32 = ((((1.0 - self.bw.cur_fade_value) * 255.0) as i32) << 24) as u32 | 0x00ff_ffff; // store alpha value
                let q = viewport_quad(device, current_fade, false);
                // texture factor 0x80A5CA8E when the device supports DOT3, else 0x60606060; drawn twice: the DOT3 pass,
                // then the alpha shader pass with ALWAYS depth compare — device-only
                out.quads.push(q);
                out.quads.push(q);
                out.result = true;
            }
            FilterImpl::CrossFade => {
                // ScreenCrossFadeFilter::postRender (W3DShaderManager.cpp:769)
                if self.cross_fade.skip_render {
                    self.cross_fade.skip_render = false;
                    out.do_extra_render = true;
                    let _ = self.end_render_to_texture();
                    out.result = true;
                    return out;
                }
                if !self.render_targets.render_texture {
                    return out; // DEBUG_ASSERTCRASH(tex, ("Require last rendered texture."))
                }
                // set(mode): fade shader states for the circle or frame buffer mask mode — device-only
                let mut radius: f32 = 0.0;
                if mode == FM_VIEW_CROSSFADE_CIRCLE {
                    // stage 1 = the fade pattern texture
                    radius = (1.0 - self.cross_fade.cur_fade_value) * 2.0;
                    if radius <= 0.0 {
                        radius = 0.01;
                    }
                    radius = 0.5 / radius;
                }
                let c = view_quad_corners(device, false);
                let diffuse: u32 = 0xffff_ffff;
                let uv1 = [(0.5 + radius, 0.5 + radius), (0.5 + radius, 0.5 - radius), (0.5 - radius, 0.5 + radius), (0.5 - radius, 0.5 - radius)];
                let mut v = [FilterQuadVertex::default(); 4];
                for i in 0..4 {
                    v[i] = FilterQuadVertex { p: None, color: diffuse, u: c[i].1, v: c[i].2, u1: uv1[i].0, v1: uv1[i].1 };
                }
                out.quads.push(FilterQuad { v, alpha_blend: None });
                out.result = true;
            }
            FilterImpl::MotionBlur => {
                out = self.motion_blur_post_render(mode, scroll_delta, game_logic_frame, device);
            }
        }
        out
    }

    /// `ScreenMotionBlurFilter::postRender` (W3DShaderManager.cpp:951).
    fn motion_blur_post_render(&mut self, mode: i32, scroll_delta: &Coord2D, game_logic_frame: i32, device: &impl ShaderDevice) -> FilterPostRender {
        let mut out = FilterPostRender::default();
        if !self.end_render_to_texture() {
            return out;
        }
        // set(mode): preset material, opaque shader, no textures, ZFUNC ALWAYS, ZWRITE off — device-only

        let mut continue_effect = true;

        let c = view_quad_corners(device, true);
        let mut v = [FilterQuadVertex::default(); 4];
        for i in 0..4 {
            v[i] = FilterQuadVertex { p: Some(c[i].0), color: 0xffff_ffff, u: c[i].1, v: c[i].2, u1: 0.0, v1: 0.0 };
        }

        // blend: SRCALPHA/ONE when additive, else SRCALPHA/INVSRCALPHA; ALPHABLENDENABLE false for the first draw

        let m = &mut self.motion_blur;
        let mut center = Coord2D { x: 0.5, y: 0.5 };

        let mut pan = false;
        if mode >= FM_VIEW_MB_PAN_ALPHA {
            // sqrt is the double function
            let len = super::double_math::c_sqrt(scroll_delta.x * scroll_delta.x + scroll_delta.y * scroll_delta.y);
            center.y -= 0.5; // * (scrollDelta.y/len);
            m.decrement = false;
            m.max_count = (len * 200.0 * m.pan_factor as f32 / MB_DEFAULT_PAN_FACTOR as f32) as i32;
            if m.max_count < m.pan_factor / 2 {
                m.max_count = m.pan_factor / 2;
            }
            if m.max_count > m.pan_factor {
                m.max_count = m.pan_factor;
            }
            pan = true;
            m.prior_delta = *scroll_delta;
        } else if mode == FM_VIEW_MB_END_PAN_ALPHA {
            let len = super::double_math::c_sqrt(m.prior_delta.x * m.prior_delta.x + m.prior_delta.y * m.prior_delta.y);
            center.x += 0.5 * (m.prior_delta.x / len);
            center.y -= 0.5 * (m.prior_delta.y / len);
            m.decrement = false;
            m.max_count -= 1;
            if m.max_count < 2 {
                continue_effect = false;
            }
            pan = true;
        }

        m.skip_render = false;
        if !pan && m.last_frame != game_logic_frame {
            if m.decrement {
                m.max_count -= MB_COUNT_STEP;
                if m.max_count < 1 {
                    m.decrement = false;
                    continue_effect = false;
                } else {
                    m.skip_render = true;
                }
            } else {
                m.max_count += MB_COUNT_STEP;
                if m.max_count >= MB_MAX_COUNT {
                    m.decrement = true;
                    if m.do_zoom_to && m.zoom_to_valid {
                        out.look_at = Some(m.zoom_to_pos);
                    } else {
                        continue_effect = false;
                    }
                } else {
                    m.skip_render = true;
                }
            }
        }

        if !pan {
            for i in 0..4 {
                let mut factor: f32 = 1.0 - (m.max_count as f32 / MB_MAX_COUNT as f32) * 0.90;
                factor = super::double_math::c_sqrt(factor);
                v[i].u = ((v[i].u - center.x) * factor) + center.x;
                v[i].v = ((v[i].v - center.y) * factor) + center.y;
            }
        }

        // alpha arg1 = current, arg2 = texture, op SELECTARG1 — device-only
        out.quads.push(FilterQuad { v, alpha_blend: Some(false) });

        {
            let mut limit = m.max_count;
            if m.max_count > 30 {
                limit = 30;
            }
            for _j in 0..limit {
                for i in 0..4 {
                    let mut factor: f32 = 0.99;
                    if m.additive {
                        factor = 0.98;
                    }
                    let mut alpha: i32 = 0x15;
                    if m.additive {
                        alpha = 0x09;
                        if m.max_count > limit {
                            alpha += (m.max_count - limit) / 5;
                        }
                        if m.max_count == MB_MAX_COUNT {
                            alpha += 60;
                        }
                    }
                    v[i].color = ((alpha << 24) | 0x00ff_ffff) as u32;
                    if pan {
                        // (factor+.006): the double sum
                        v[i].u = ((v[i].u - center.x) * ((factor as f64 + 0.006) as f32)) + center.x;
                        v[i].v = ((v[i].v - center.y) * factor) + center.y;
                    } else {
                        v[i].u = ((v[i].u - center.x) * factor) + center.x;
                        v[i].v = ((v[i].v - center.y) * factor) + center.y;
                    }
                }
                out.quads.push(FilterQuad { v, alpha_blend: Some(true) });
            }
        }

        m.last_frame = game_logic_frame;
        if pan {
            m.skip_render = false;
        }

        if !continue_effect {
            m.zoom_to_valid = false;
        }
        out.result = continue_effect;
        out
    }

    /// `W3DShaderManager::drawViewport` (W3DShaderManager.cpp:2795): draws 2 triangles covering the viewport given the
    /// current render states, with `color` for all four vertices.
    pub fn draw_viewport(&self, color: u32, device: &impl ShaderDevice) -> FilterQuad {
        viewport_quad(device, color, true)
    }

    /// `W3DShaderManager::startRenderToTexture` (W3DShaderManager.cpp:2840): starts rendering to a texture. Returns what
    /// the clear does, as `Some(clear_alpha_only)` : with soft water edges the destination alpha is cleared to
    /// `m_minWaterOpacity` (`true`: only the alpha, by a viewport quad coloured `0x00ffffff | (minWaterOpacity*255)<<24`,
    /// for the filters that rely on the previous frame; `false`: a full clear).
    pub fn start_render_to_texture(&mut self, device: &impl ShaderDevice) -> Option<bool> {
        // DEBUG_ASSERTCRASH(!m_renderingToTexture, ("Already rendering to texture - cannot nest calls."));
        if self.rendering_to_texture || !self.render_targets.new_render_surface || !self.render_targets.old_depth_surface {
            return None;
        }

        // SetRenderTarget(m_newRenderSurface, m_oldDepthSurface)
        self.rendering_to_texture = true;

        if device.show_soft_water_edge() {
            // Soft water edges use frame buffer destination alpha so we must clear it to a known value.
            if self.current_filter == FilterTypes::ViewMotionBlurFilter || self.current_filter == FilterTypes::ViewCrossfade {
                // these filters rely on the previous frame being visible so we must be careful about clearing: only
                // clear alpha with drawViewport(0x00ffffff | (((Int)(minWaterOpacity*255.0f)) << 24))
                return Some(true);
            }
            // normal clear that overwrites everything: Clear(true, false, (0,0,0), minWaterOpacity)
            return Some(false);
        }
        None
    }

    /// The colour `startRenderToTexture` clears the alpha with for the filters that rely on the previous frame.
    pub fn clear_alpha_viewport_color(device: &impl ShaderDevice) -> u32 {
        (0x00ff_ffffu32) | ((((device.min_water_opacity() * 255.0) as i32) as u32) << 24)
    }

    /// `W3DShaderManager::endRenderToTexture` (W3DShaderManager.cpp:2877): ends rendering to a texture (restores the
    /// original render target and sets the texture stage 0 to clamp / linear / no mip). Returns whether it was rendering
    /// (upstream returns `m_renderTexture`, or NULL when not rendering).
    pub fn end_render_to_texture(&mut self) -> bool {
        // DEBUG_ASSERTCRASH(m_renderingToTexture, ("Not rendering to texture."));
        if !self.rendering_to_texture {
            return false;
        }

        self.rendering_to_texture = false;
        self.render_targets.render_texture
    }

    /// `W3DShaderManager::testMinimumRequirements` (W3DShaderManager.cpp:3084).
    pub fn test_minimum_requirements(
        &mut self,
        device: &mut impl ShaderDevice,
        want_video: bool,
        want_cpu_type: bool,
        want_cpu_freq: bool,
        want_ram: bool,
        want_bench: bool,
    ) -> (Option<ChipsetType>, Option<CpuType>, Option<i32>, Option<i32>, Option<(f32, f32, f32)>) {
        let video = if want_video { Some(self.get_chipset(device)) } else { None };
        let cpu = device.cpu_info();
        let cpu_type = if want_cpu_type {
            let mut t = CpuType::Xx; // unknown
            if cpu.is_amd && cpu.amd_at_least_athlon {
                t = CpuType::K7;
            }
            if cpu.is_intel && cpu.intel_at_least_p3 {
                t = CpuType::P3;
            }
            if cpu.is_intel && cpu.intel_at_least_p4 {
                t = CpuType::P4;
            }
            Some(t)
        } else {
            None
        };
        let freq = if want_cpu_freq { Some(cpu.speed) } else { None };
        let ram = if want_ram { Some(cpu.total_physical_memory) } else { None };
        let bench = if want_bench {
            // RunBenchmark(0, NULL, floatBenchIndex, intBenchIndex, memBenchIndex)
            Some(device.run_benchmark())
        } else {
            None
        };
        (video, cpu_type, freq, ram, bench)
    }

    /// `W3DShaderManager::getGPUPerformanceIndex` (W3DShaderManager.cpp:3123): try to guess how well the video card will
    /// handle the game assuming very fast CPU.
    pub fn get_gpu_performance_index(&mut self, device: &impl ShaderDevice) -> StaticGameLodLevel {
        let mut detail_setting = StaticGameLodLevel::Low; // assume lowest settings for now.

        let chip_type = self.get_chipset(device);
        if chip_type != ChipsetType::Unknown {
            // a known video card so we can make some assumptions
            if chip_type >= ChipsetType::GeForce2 {
                detail_setting = StaticGameLodLevel::Low; // these cards need multiple terrain passes.
            }
            if chip_type >= ChipsetType::GenericPixelShader1_1 {
                // these cards can do terrain in single pass.
                detail_setting = StaticGameLodLevel::High;
            }
        }
        detail_setting
    }

    /// `W3DShaderManager::GetCPUBenchTime` (W3DShaderManager.cpp:3149): returns seconds needed to run the test. We need a
    /// hardware independent method to compare different CPUs; for lack of anything better, time a calculation of PI using
    /// a slow random number algorithm. `elapsed_seconds` times the closure it is given (`QueryPerformanceCounter`).
    pub fn get_cpu_bench_time(elapsed_seconds: &mut impl FnMut(&mut dyn FnMut()) -> f64) -> f32 {
        let mut result_pi: f32 = 0.0;
        let secs = elapsed_seconds(&mut || {
            let mut ztot: f32 = 0.0;
            let mut low: i32 = 1;
            let mut ixran: i32 = 1907;
            let mut yran: f32 = 5813.0;
            let ymult: f32 = 1307.0;
            let ymod: f32 = 5471.0;
            let itot: i32 = 560000; // total iterations. This value ends up running at ~30 fps on our P4-2.2Ghz.

            for _j in 1..=itot {
                let iprod: i32 = 27611i32.wrapping_mul(ixran);
                ixran = iprod - 74383 * (iprod / 74383);
                let x: f32 = (ixran as f64 / 74383.0) as f32;

                let prod: f32 = ymult * yran;
                yran = prod - ymod * ((prod / ymod) as i32 as f32);
                let y: f32 = (yran as f64 / ymod as f64) as f32;

                let z: f32 = x * x + y * y;
                ztot = ztot + z; // add(&ztot, &z)
                if (z as f64) <= 1.0 {
                    low += 1;
                }
            }
            let _ = ztot;
            result_pi = (4.0f64 * low as f64 / itot as f64) as f32;
        });
        let _ = result_pi;
        secs as f32
    }

    /// `W3DShaderManager::setShroudTex` (W3DShaderManager.cpp:3192): puts the shroud texture into a texture stage. Returns
    /// the texture transform to set for `D3DTS_TEXTURE0 + stage` (`None` when there is no shroud: upstream returns FALSE).
    /// `view_inverse` is the inverse of the current view transform (`D3DXMatrixInverse`).
    pub fn set_shroud_tex(&self, shroud: Option<(f32, f32, f32, f32, f32, f32)>, view_inverse: &D3dxMatrix, map_present: bool) -> Option<D3dxMatrix> {
        // texture stage: shroud texture, TEXCOORDINDEX CAMERASPACEPOSITION, TEXTURETRANSFORMFLAGS COUNT2, COLOROP MODULATE,
        // ALPHAOP SELECTARG2 — device-only
        shroud.map(|s| shroud_texture_transform(view_inverse, s, map_present))
    }
}
