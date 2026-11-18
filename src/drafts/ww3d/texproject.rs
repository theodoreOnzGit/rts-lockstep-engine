//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texproject.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/texproject.h
//!
//! `TexProjectClass`: projects a texture onto the objects in a volume (the shadow and light-projection mechanism).
//! Upstream derives from `ProjectorClass`, `CullableClass` and `MultiListObjectClass`; here it holds a
//! [`ProjectorClass`] (reused from `projector.rs`), the cull box the `CullableClass` base stores, and its own
//! [`MaterialPassClass`] (reused from `matpass.rs`) with the vertex-material settings it makes recorded in
//! [`TexProjMaterial`] (upstream creates the `VertexMaterialClass` itself and sets its colours, so the values are data
//! here; the vertex material handle of the pass stays the owner's).
//!
//! The asset manager, texture and WW3D frame state it reads are the [`TexProjHost`] contract. Device-only, not translated:
//! `Compute_Texture`'s render-to-texture calls (`DX8Wrapper::Set_Render_Target_With_Z`, `WW3D::Begin_Render`/`Render`/
//! `End_Render`, `Activate_Snapshot`, `Set_Render_Target`), which are kept as comments around [`TexProjectClass::
//! configure_camera`] (the camera set-up they use, returned as data). The `#if 0` BW-renderer block is not translated.

use super::coltest::ObBoxClass;
use super::handles::{ObBoxHandle, TextureHandle};
use super::matpass::MaterialPassClass;
use super::matrixmapper::{matrix4x4_multiply, matrix4x4_from_matrix3d, MappingType, MatrixMapperClass};
use super::projector::ProjectorClass;
use crate::client::tactical_view::Matrix4x4;
use crate::formats::w3d::mesh::shader_bits::*;
use crate::formats::w3d::segline::AABox;
use crate::formats::w3d::shader::*;
use crate::formats::w3d::vertmaterial::VertexMaterialState;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `INTENSITY_RATE_OF_CHANGE`: change in intensity per second.
pub const INTENSITY_RATE_OF_CHANGE: f32 = 1.0;

/// What the material pass's cull volume points at: this projector's `WorldBoundingVolume`
/// (`MaterialPass->Set_Cull_Volume(&WorldBoundingVolume)`); the owner resolves it.
pub const WORLD_BOUNDING_VOLUME: ObBoxHandle = 0;

/// `DETAILCOLOR_*` (shader.h) the two shaders use.
pub const DETAILCOLOR_DISABLE: u32 = 0;
pub const DETAILCOLOR_SCALE: u32 = 2;
pub const DETAILCOLOR_ADD: u32 = 4;

/// `TexProjectClass::FlagsType`.
pub mod flags {
    pub const PERSPECTIVE: u32 = 0x0000_0001;
    pub const ADDITIVE: u32 = 0x0000_0002;
    pub const TEXTURE_DIRTY: u32 = 0x0000_0004;
    pub const VOLATILE: u32 = 0x0000_0008;
    pub const ATTENUATE: u32 = 0x0000_0010;
    pub const AFFECT_DYNAMIC_OBJS: u32 = 0x0000_0020;
    pub const AFFECT_STATIC_OBJS: u32 = 0x0000_0040;
    pub const USE_DEPTH_GRADIENT: u32 = 0x0000_0080;
    pub const HAS_RENDER_TARGET: u32 = 0x0000_0100;
    pub const SIZE_MASK: u32 = 0xFFF0_0000;
    pub const SIZE_SHIFT: u32 = 20;
    pub const DEFAULT_FLAGS: u32 = ATTENUATE | AFFECT_DYNAMIC_OBJS | AFFECT_STATIC_OBJS;
}

/// The asset manager / WW3D / texture queries `TexProjectClass` makes.
pub trait TexProjHost {
    /// `WW3DAssetManager::Get_Instance()->Get_Activate_Fog_On_Load()`.
    fn activate_fog_on_load(&self) -> bool;
    /// `WW3DAssetManager::Get_Instance()->Get_Texture(name)` (a new reference, or `None` for NULL).
    fn get_texture(&mut self, name: &str) -> Option<TextureHandle>;
    /// `texture->Get_Filter().Set_U_Addr_Mode(TEXTURE_ADDRESS_CLAMP)` and `Set_V_Addr_Mode(TEXTURE_ADDRESS_CLAMP)`.
    fn set_texture_clamp(&mut self, texture: TextureHandle);
    /// `texture->Get_Width()`.
    fn texture_width(&self, texture: TextureHandle) -> i32;
    /// `WW3D::Get_Frame_Time()`: milliseconds.
    fn frame_time_ms(&self) -> u32;
}

/// The `VertexMaterialClass` this projector creates for its pass: the colours and lighting it sets, and whether a
/// mapper is plugged in at stage 1 (`Set_Mapper(Mapper1, 1)` / `Set_Mapper(NULL, 1)`).
#[derive(Clone, Debug, PartialEq)]
pub struct TexProjMaterial {
    /// Ambient/Diffuse/Specular/Emissive/Opacity/UseLighting.
    pub state: VertexMaterialState,
    /// `Set_Mapper(Mapper1, 1)` is in effect.
    pub mapper1_installed: bool,
}

/// What `Configure_Camera(camera)` sets on the camera (`CameraClass` setters).
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectorCameraConfig {
    /// `Set_Transform(Transform)`.
    pub transform: Matrix3D,
    /// `Set_Clip_Planes(0.01f, ZFar)`.
    pub znear: f32,
    pub zfar: f32,
    /// `Set_Projection_Type(PERSPECTIVE)` (else ORTHO).
    pub perspective: bool,
    /// `Set_View_Plane(HFov, VFov)` (perspective).
    pub hfov: f32,
    pub vfov: f32,
    /// `Set_View_Plane(Vector2(XMin,YMin), Vector2(XMax,YMax))` (ortho).
    pub view_min: Vector2,
    pub view_max: Vector2,
    /// `Set_Viewport(vmin, vmax)`: the one-pixel border.
    pub viewport_min: Vector2,
    pub viewport_max: Vector2,
}

/// `OBBoxClass::Compute_Axis_Aligned_Extent` (obbox.h:217).
pub fn obbox_compute_axis_aligned_extent(b: &ObBoxClass) -> Vector3 {
    let e = [b.extent.x, b.extent.y, b.extent.z];
    let row = |r: usize| wwmath::fabs(e[0] * b.basis[r][0]) + wwmath::fabs(e[1] * b.basis[r][1]) + wwmath::fabs(e[2] * b.basis[r][2]);
    Vector3::new(row(0), row(1), row(2))
}

/// `ShaderClass::Set_Post_Detail_Color_Func(x)`.
fn set_post_detail_color_func(s: &mut ShaderClass, x: u32) {
    s.0 &= !MASK_POSTDETAILCOLORFUNC;
    s.0 |= x << SHIFT_POSTDETAILCOLORFUNC;
}

/// `SHADE_CNST(...)` for the two shaders below (the port keeps `shade_cnst` private).
#[allow(clippy::too_many_arguments)]
const fn shade(
    depth_compare: u32,
    depth_mask: u32,
    color_mask: u32,
    src_blend: u32,
    dst_blend: u32,
    fog: u32,
    pri_grad: u32,
    sec_grad: u32,
    texture: u32,
    alpha_test: u32,
    cullmode: u32,
    post_det_color: u32,
    post_det_alpha: u32,
) -> ShaderClass {
    ShaderClass(
        depth_compare << SHIFT_DEPTHCOMPARE
            | depth_mask << SHIFT_DEPTHMASK
            | color_mask << SHIFT_COLORMASK
            | dst_blend << SHIFT_DSTBLEND
            | fog << SHIFT_FOG
            | pri_grad << SHIFT_PRIGRADIENT
            | sec_grad << SHIFT_SECGRADIENT
            | src_blend << SHIFT_SRCBLEND
            | texture << SHIFT_TEXTURING
            | alpha_test << SHIFT_ALPHATEST
            | cullmode << SHIFT_CULLMODE
            | post_det_color << SHIFT_POSTDETAILCOLORFUNC
            | post_det_alpha << SHIFT_POSTDETAILALPHAFUNC,
    )
}

/// `class TexProjectClass` (texproject.h).
#[derive(Clone, Debug)]
pub struct TexProjectClass {
    /// The `ProjectorClass` base.
    pub projector: ProjectorClass,
    /// The `CullableClass` base's cull box (`Set_Cull_Box`).
    pub cull_box: AABox,
    /// `Flags`.
    pub flags: u32,
    /// `DesiredIntensity`.
    pub desired_intensity: f32,
    /// `Intensity`.
    pub intensity: f32,
    /// `Attenuation`.
    pub attenuation: f32,
    /// `MaterialPass`.
    pub material_pass: MaterialPassClass,
    /// The vertex material the pass uses.
    pub material: TexProjMaterial,
    /// `Mapper1` (NULL until needed).
    pub mapper1: Option<MatrixMapperClass>,
    /// `RenderTarget` / `DepthStencilTarget`.
    pub render_target: Option<TextureHandle>,
    pub depth_stencil_target: Option<usize>,
    /// `HFov`, `VFov`, `XMin`, `XMax`, `YMin`, `YMax`, `ZNear`, `ZFar`.
    pub hfov: f32,
    pub vfov: f32,
    pub xmin: f32,
    pub xmax: f32,
    pub ymin: f32,
    pub ymax: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl TexProjectClass {
    /// `TexProjectClass::TexProjectClass()`: defaults, a material pass whose cull volume is the world bounding volume,
    /// a vertex material using the projector's mapper at stage 0, then `Init_Multiplicative()` (the pass starts
    /// multiplicative, a shadow).
    pub fn new<H: TexProjHost>(host: &mut H) -> Self {
        let mut material_pass = MaterialPassClass::new();
        material_pass.set_cull_volume(Some(WORLD_BOUNDING_VOLUME));
        let mut t = TexProjectClass {
            projector: ProjectorClass::new(),
            cull_box: AABox { center: Vector3::new(0.0, 0.0, 0.0), extent: Vector3::new(0.0, 0.0, 0.0) },
            flags: flags::DEFAULT_FLAGS,
            desired_intensity: 1.0,
            intensity: 1.0,
            attenuation: 1.0,
            material_pass,
            material: TexProjMaterial { state: VertexMaterialState::default(), mapper1_installed: false },
            mapper1: None,
            render_target: None,
            depth_stencil_target: None,
            hfov: 90.0,
            vfov: 90.0,
            xmin: -10.0,
            xmax: 10.0,
            ymin: -10.0,
            ymax: 10.0,
            znear: 1.0,
            zfar: 1000.0,
        };
        // vmtl->Set_Mapper(Mapper): stage 0 is the projector's own mapper; MaterialPass->Set_Material(vmtl)
        t.init_multiplicative(host);
        t
    }

    /// `Set_Texture_Size(size)`: `0 < size <= 512` (asserted), stored in the upper bits of `Flags`.
    pub fn set_texture_size(&mut self, size: i32) {
        debug_assert!(size > 0);
        debug_assert!(size <= 512);
        self.flags &= !flags::SIZE_MASK;
        self.flags |= (size as u32) << flags::SIZE_SHIFT;
    }

    /// `Get_Texture_Size()`.
    pub fn get_texture_size(&self) -> i32 {
        ((self.flags & flags::SIZE_MASK) >> flags::SIZE_SHIFT) as i32
    }

    /// `Set_Flag(flag, onoff)`.
    pub fn set_flag(&mut self, flag: u32, onoff: bool) {
        if onoff {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// `Get_Flag(flag)`: all bits of `flag` set.
    pub fn get_flag(&self, flag: u32) -> bool {
        (self.flags & flag) == flag
    }

    /// `Set_Intensity(intensity, immediate)`: `0 <= intensity <= 1` (asserted).
    pub fn set_intensity(&mut self, intensity: f32, immediate: bool) {
        debug_assert!(intensity <= 1.0);
        debug_assert!(intensity >= 0.0);
        self.desired_intensity = intensity;
        if immediate {
            self.intensity = self.desired_intensity;
        }
    }

    /// `Get_Intensity()`: the desired intensity.
    pub fn get_intensity(&self) -> f32 {
        self.desired_intensity
    }

    /// `Is_Intensity_Zero()`.
    pub fn is_intensity_zero(&self) -> bool {
        self.intensity == 0.0 && self.desired_intensity == 0.0
    }

    /// `Set_Attenuation(attenuation)`: `0 <= attenuation <= 1` (asserted).
    pub fn set_attenuation(&mut self, attenuation: f32) {
        debug_assert!(attenuation >= 0.0);
        debug_assert!(attenuation <= 1.0);
        self.attenuation = attenuation;
    }

    /// `Get_Attenuation()`.
    pub fn get_attenuation(&self) -> f32 {
        self.attenuation
    }

    /// `Enable_Attenuation(onoff)`.
    pub fn enable_attenuation(&mut self, onoff: bool) {
        self.set_flag(flags::ATTENUATE, onoff);
    }

    /// `Is_Attenuation_Enabled()`.
    pub fn is_attenuation_enabled(&self) -> bool {
        self.get_flag(flags::ATTENUATE)
    }

    /// `Enable_Affect_Dynamic_Objects(onoff)`.
    pub fn enable_affect_dynamic_objects(&mut self, onoff: bool) {
        self.set_flag(flags::AFFECT_DYNAMIC_OBJS, onoff);
    }

    /// `Is_Affect_Dynamic_Objects_Enabled()`.
    pub fn is_affect_dynamic_objects_enabled(&self) -> bool {
        self.get_flag(flags::AFFECT_DYNAMIC_OBJS)
    }

    /// `Enable_Affect_Static_Objects(onoff)`.
    pub fn enable_affect_static_objects(&mut self, onoff: bool) {
        self.set_flag(flags::AFFECT_STATIC_OBJS, onoff);
    }

    /// `Is_Affect_Static_Objects_Enabled()`.
    pub fn is_affect_static_objects_enabled(&self) -> bool {
        self.get_flag(flags::AFFECT_STATIC_OBJS)
    }

    /// `Enable_Depth_Gradient(onoff)`: sets the flag and re-runs the matching `Init_*`.
    pub fn enable_depth_gradient<H: TexProjHost>(&mut self, host: &mut H, onoff: bool) {
        self.set_flag(flags::USE_DEPTH_GRADIENT, onoff);
        // re-setup the shader settings
        if self.get_flag(flags::ADDITIVE) {
            self.init_additive(host);
        } else {
            self.init_multiplicative(host);
        }
    }

    /// `Is_Depth_Gradient_Enabled(onoff)` (the argument is unused upstream).
    pub fn is_depth_gradient_enabled(&self, _onoff: bool) -> bool {
        self.get_flag(flags::USE_DEPTH_GRADIENT)
    }

    fn load_gradient<H: TexProjHost>(&mut self, host: &mut H, name: &str) {
        match host.get_texture(name) {
            Some(grad_tex) => {
                host.set_texture_clamp(grad_tex);
                self.material_pass.set_texture(Some(grad_tex), 1);
                // grad_tex->Release_Ref()
            }
            None => {
                // WWDEBUG_SAY(("Could not find texture: %s!\n", name));
            }
        }
    }

    /// `Init_Multiplicative()`: the shadow shader (`ZERO`/`SRC_COLOR`), black material with emissive scaling the
    /// intensity, and, with a depth gradient, the `MultProjectorGradient.tga` second stage.
    pub fn init_multiplicative<H: TexProjHost>(&mut self, host: &mut H) {
        self.set_flag(flags::ADDITIVE, false);

        // Set up the shader
        // mult_shader: PASS_LEQUAL, DEPTH_WRITE_DISABLE, COLOR_WRITE_ENABLE, SRCBLEND_ZERO, DSTBLEND_SRC_COLOR, FOG_DISABLE,
        // GRADIENT_ADD, SECONDARY_GRADIENT_DISABLE, TEXTURING_ENABLE, ALPHATEST_DISABLE, CULL_MODE_ENABLE, 0, 0
        // (upstream's `static` is shared and mutated by the calls below; every call re-sets the same fields, so a fresh
        // copy per call gives the same result except for the fog function, which the static carries over)
        let mut mult_shader = shade(PASS_LEQUAL, 0, 1, SRCBLEND_ZERO, DSTBLEND_SRC_COLOR, FOG_DISABLE, GRADIENT_ADD, 0, 1, 0, 1, 0, 0);
        if host.activate_fog_on_load() {
            mult_shader.enable_fog();
        }
        if self.get_flag(flags::USE_DEPTH_GRADIENT) {
            // enable multi-texturing
            set_post_detail_color_func(&mut mult_shader, DETAILCOLOR_ADD);
            // plug the gradient texture into the second stage
            self.load_gradient(host, "MultProjectorGradient.tga");
        } else {
            // disable multi-texturing
            set_post_detail_color_func(&mut mult_shader, DETAILCOLOR_DISABLE);
            // remove the texture from the second stage
            self.material_pass.set_texture(None, 1);
        }
        // (DEBUG_SHADOW_RENDERING is 0)
        self.material_pass.set_shader(mult_shader);

        // Set up the Vertex Material parameters
        let s = &mut self.material.state;
        s.ambient = [0.0; 3];
        s.diffuse = [0.0; 3];
        s.specular = [0.0; 3];
        s.emissive = [0.0; 3];
        s.opacity = 1.0;
        s.use_lighting = true; // I need the emissive value to scale the intensity of the shadow

        // Set up some mapper settings related to depth gradient
        if self.get_flag(flags::USE_DEPTH_GRADIENT) {
            if self.mapper1.is_none() {
                self.mapper1 = Some(MatrixMapperClass::new(1));
            }
            self.mapper1.as_mut().unwrap().set_type(MappingType::DepthGradient);
            self.material.mapper1_installed = true;
        } else {
            self.material.mapper1_installed = false;
        }
    }

    /// `Init_Additive()`: the light shader (`ONE`/`ONE`), white emissive scaled by the intensity, the
    /// `AddProjectorGradient.tga` second stage, and a normal-gradient mapper at stage 1.
    pub fn init_additive<H: TexProjHost>(&mut self, host: &mut H) {
        self.set_flag(flags::ADDITIVE, true);

        // add_shader: PASS_LEQUAL, DEPTH_WRITE_DISABLE, COLOR_WRITE_ENABLE, SRCBLEND_ONE, DSTBLEND_ONE, FOG_DISABLE,
        // GRADIENT_MODULATE, SECONDARY_GRADIENT_DISABLE, TEXTURING_ENABLE, ALPHATEST_DISABLE, CULL_MODE_ENABLE,
        // DETAILCOLOR_DISABLE, DETAILALPHA_DISABLE
        let mut add_shader = shade(PASS_LEQUAL, 0, 1, SRCBLEND_ONE, DSTBLEND_ONE, FOG_DISABLE, GRADIENT_MODULATE, 0, 1, 0, 1, DETAILCOLOR_DISABLE, 0);
        if host.activate_fog_on_load() {
            add_shader.enable_fog();
        }
        // Additive projectors always use the normal gradient so they need multi-texturing
        set_post_detail_color_func(&mut add_shader, DETAILCOLOR_SCALE);
        // plug in the gradient texture
        self.load_gradient(host, "AddProjectorGradient.tga");
        // (DEBUG_SHADOW_RENDERING is 0)
        self.material_pass.set_shader(add_shader);

        // Set up the Vertex Material parameters
        let s = &mut self.material.state;
        s.ambient = [0.0; 3];
        s.diffuse = [0.0; 3];
        s.specular = [0.0; 3];
        s.emissive = [1.0; 3];
        s.opacity = 1.0;
        s.use_lighting = true; // need emissive to scale the intensity of the projector

        // Additive texture projections always use the normal gradient
        if self.mapper1.is_none() {
            self.mapper1 = Some(MatrixMapperClass::new(1));
        }
        self.mapper1.as_mut().unwrap().set_type(MappingType::NormalGradient);
        self.material.mapper1_installed = true;
    }

    /// `Set_Texture(texture)`: sets clamp addressing on both axes and installs it (nothing for NULL).
    pub fn set_texture<H: TexProjHost>(&mut self, host: &mut H, texture: Option<TextureHandle>) {
        if let Some(t) = texture {
            host.set_texture_clamp(t);
            self.material_pass.set_texture(Some(t), 0);
        }
    }

    /// `Get_Texture()` / `Peek_Texture()`: the pass's stage-0 texture (`Get_` adds a reference).
    pub fn peek_texture(&self) -> Option<TextureHandle> {
        self.material_pass.get_texture(0)
    }

    /// `Peek_Material_Pass()`.
    pub fn peek_material_pass(&self) -> &MaterialPassClass {
        &self.material_pass
    }

    /// `Set_Perspective_Projection(hfov, vfov, znear, zfar)`: remembers the parameters for the camera set-up, projects,
    /// flags `PERSPECTIVE`.
    pub fn set_perspective_projection(&mut self, hfov: f32, vfov: f32, znear: f32, zfar: f32) {
        self.hfov = hfov;
        self.vfov = vfov;
        self.znear = znear;
        self.zfar = zfar;
        self.projector.set_perspective_projection(hfov, vfov, znear, zfar);
        self.update_ws_bounding_volume(); // the virtual Update_WS_Bounding_Volume of this class
        self.set_flag(flags::PERSPECTIVE, true);
    }

    /// `Set_Ortho_Projection(xmin, xmax, ymin, ymax, znear, zfar)`.
    pub fn set_ortho_projection(&mut self, xmin: f32, xmax: f32, ymin: f32, ymax: f32, znear: f32, zfar: f32) {
        self.xmin = xmin;
        self.xmax = xmax;
        self.ymin = ymin;
        self.ymax = ymax;
        self.znear = znear;
        self.zfar = zfar;
        self.projector.set_ortho_projection(xmin, xmax, ymin, ymax, znear, zfar);
        self.update_ws_bounding_volume();
        self.set_flag(flags::PERSPECTIVE, false);
    }

    /// `ProjectorClass::Set_Transform` through this class's virtual `Update_WS_Bounding_Volume`.
    pub fn set_transform(&mut self, tm: &Matrix3D) {
        self.projector.set_transform(tm);
        self.update_ws_bounding_volume();
    }

    /// `Compute_Perspective_Projection(obj_box, tm, lightpos, user_znear, user_zfar)`. Pass `-1.0` for the
    /// defaults (`znear=-1.0f, zfar=-1.0f`). Returns false when the box is behind the viewpoint or the viewpoint is inside
    /// it (the field of view would exceed 180 degrees).
    pub fn compute_perspective_projection(&mut self, obj_box: &AABox, tm: &Matrix3D, lightpos: &Vector3, user_znear: f32, user_zfar: f32) -> bool {
        // Compute the center of the box in world-space
        let wrld_center = Matrix3D::transform_vector(tm, &obj_box.center);

        // Create a camera transform looking at the object. Set this as our transform later in this routine.
        let mut texture_tm = Matrix3D::default();
        texture_tm.look_at(lightpos, &wrld_center, 0.0);
        let texture_tm_inv = texture_tm.get_orthogonal_inverse();

        // Calculate the axis-aligned bounding box of the model in the camera's coordinate system.
        let mut bx = *obj_box;
        let obj_to_world = *tm;
        let obj_to_texture = Matrix3D::multiply(&texture_tm_inv, &obj_to_world);
        let (c, e) = obj_to_texture.transform_center_extent_aabox(&bx.center, &bx.extent); // AABoxClass::Transform
        bx.center = c;
        bx.extent = e;

        // If the box is behind the viewpoint or the viewpoint is inside the box our FOV will be > 180 degrees. Have to give up
        if bx.center.z > 0.0 || bx.extent.z > wwmath::fabs(bx.center.z) {
            return false;
        }

        // Compute the frustum parameters. Remember that our z coordinates are negative but the projection code needs
        // positive z distances.
        let mut znear = -bx.center.z; //-(box.Center.Z + obj_box.Extent.Quick_Length());
        let mut zfar = -(bx.center.z - obj_box.extent.quick_length()) * 2.0;
        if user_znear != -1.0 {
            znear = bx.center.z + user_znear;
        }
        if user_zfar != -1.0 {
            zfar = bx.center.z + user_zfar;
        }
        let tan_hfov2 = wwmath::fabs(bx.extent.x / (bx.center.z + bx.extent.z));
        let tan_vfov2 = wwmath::fabs(bx.extent.y / (bx.center.z + bx.extent.z));
        let hfov = 2.0 * wwmath::atan(tan_hfov2);
        let vfov = 2.0 * wwmath::atan(tan_vfov2);

        // Plug in the results.
        self.set_perspective_projection(hfov, vfov, znear, zfar);
        self.set_transform(&texture_tm);
        true
    }

    /// `Compute_Ortho_Projection(obj_box, tm, lightdir, user_znear, user_zfar)`; `-1.0` for the defaults.
    pub fn compute_ortho_projection(&mut self, obj_box: &AABox, tm: &Matrix3D, lightdir: &Vector3, user_znear: f32, user_zfar: f32) -> bool {
        // Compute the center of the box in world-space
        let mut wrldbox = *obj_box;
        let (c, e) = tm.transform_center_extent_aabox(&wrldbox.center, &wrldbox.extent);
        wrldbox.center = c;
        wrldbox.extent = e;

        // Create a camera transform looking at the object. Set this as our transform later in this routine.
        let camera_target = wrldbox.center;
        let camera_position = camera_target - *lightdir * (2.0 * wrldbox.extent.length());
        let mut texture_tm = Matrix3D::default();
        texture_tm.look_at(&camera_position, &camera_target, 0.0);
        let texture_tm_inv = texture_tm.get_orthogonal_inverse();

        // Calculate the axis-aligned bounding box of the model in the camera's coordinate system.
        let mut bx = *obj_box;
        let obj_to_world = *tm;
        let obj_to_texture = Matrix3D::multiply(&texture_tm_inv, &obj_to_world);
        let (c, e) = obj_to_texture.transform_center_extent_aabox(&bx.center, &bx.extent);
        bx.center = c;
        bx.extent = e;

        // Expand the box to help with bounding box errors
        bx.extent = bx.extent * 1.0;

        // Compute the frustum parameters.  Note that znear and zfar are supposed to be positive distances for the
        // projection code.
        let mut znear = -bx.center.z;
        let mut zfar = -(bx.center.z - obj_box.extent.quick_length()) * 2.0;
        if user_znear != -1.0 {
            znear = -bx.center.z + user_znear;
        }
        if user_zfar != -1.0 {
            zfar = -bx.center.z + user_zfar;
        }

        // All done!
        self.set_ortho_projection(
            bx.center.x - bx.extent.x,
            bx.center.x + bx.extent.x,
            bx.center.y - bx.extent.y,
            bx.center.y + bx.extent.y,
            znear,
            zfar,
        );
        self.set_transform(&texture_tm);
        true
    }

    /// `Compute_Perspective_Projection(RenderObjClass *obj, lightpos, ..)` / `Compute_Ortho_Projection(obj, ..)`: with
    /// `obj_box = obj->Get_Obj_Space_Bounding_Box()` and `tm = obj->Get_Transform()` (a NULL model is `None`: upstream
    /// logs "Attempting to generate projection for a NULL model" and returns false).
    pub fn compute_perspective_projection_obj(&mut self, obj: Option<(&AABox, &Matrix3D)>, lightpos: &Vector3, znear: f32, zfar: f32) -> bool {
        match obj {
            None => false,
            Some((b, tm)) => self.compute_perspective_projection(b, tm, lightpos, znear, zfar),
        }
    }

    /// See [`Self::compute_perspective_projection_obj`].
    pub fn compute_ortho_projection_obj(&mut self, obj: Option<(&AABox, &Matrix3D)>, lightdir: &Vector3, znear: f32, zfar: f32) -> bool {
        match obj {
            None => false,
            Some((b, tm)) => self.compute_ortho_projection(b, tm, lightdir, znear, zfar),
        }
    }

    /// `Compute_Texture(model, context)`: renders the model into the render target through the camera
    /// [`Self::configure_camera`] gives. Returns upstream's result plus what to render: `None` when the model or context
    /// is NULL (`false`); `Some(None)` when there is no render target (returns true, nothing rendered);
    /// `Some(Some((camera, clear_color, zclear)))` otherwise. The device calls are:
    /// upstream: context->Texture_Projector=this; DX8Wrapper::Set_Render_Target_With_Z(rtarget,ztarget);
    /// WW3D::Begin_Render(true,zclear,color); WW3D::Render(*model,*context); WW3D::End_Render(false);
    /// WW3D::Activate_Snapshot(snapshot); DX8Wrapper::Set_Render_Target(NULL) — device-only, not translated
    pub fn compute_texture(&self, have_model: bool, have_context: bool) -> Option<Option<(ProjectorCameraConfig, Vector3, bool)>> {
        if !have_model || !have_context {
            return None;
        }
        if self.render_target.is_some() {
            let mut color = Vector3::new(0.0, 0.0, 0.0);
            if !self.get_flag(flags::ADDITIVE) {
                color.set(1.0, 1.0, 1.0);
            }
            let zclear = self.depth_stencil_target.is_some();
            Some(Some((self.configure_camera(), color, zclear)))
        } else {
            Some(None)
        }
    }

    /// `Needs_Render_Target()`: `TEXTURE_DIRTY`.
    pub fn needs_render_target(&self) -> bool {
        self.get_flag(flags::TEXTURE_DIRTY)
    }

    /// `Set_Render_Target(render_target, zbuffer)`: the target is also the projected texture (`Set_Texture`).
    pub fn set_render_target<H: TexProjHost>(&mut self, host: &mut H, render_target: Option<TextureHandle>, zbuffer: Option<usize>) {
        self.render_target = render_target;
        self.set_texture(host, render_target);
        self.depth_stencil_target = zbuffer;
    }

    /// `Peek_Render_Target(rtarget, ztarget)`: `(render target, depth-stencil target)`.
    pub fn peek_render_target(&self) -> (Option<TextureHandle>, Option<usize>) {
        (self.render_target, self.depth_stencil_target)
    }

    /// `Configure_Camera(camera)`: the camera set-up for render-to-texture, with a one-pixel border to the texture "to
    /// avoid flooding shadows".
    pub fn configure_camera(&self) -> ProjectorCameraConfig {
        let size = self.get_texture_size() as f32;
        let inv_size = 1.0 / size;
        ProjectorCameraConfig {
            transform: *self.projector.get_transform(),
            znear: 0.01,
            zfar: self.zfar,
            perspective: self.get_flag(flags::PERSPECTIVE),
            hfov: self.hfov,
            vfov: self.vfov,
            view_min: Vector2::new(self.xmin, self.ymin),
            view_max: Vector2::new(self.xmax, self.ymax),
            viewport_min: Vector2::new(1.0 * inv_size, 1.0 * inv_size),
            viewport_max: Vector2::new((size - 1.0) * inv_size, (size - 1.0) * inv_size),
        }
    }

    /// `Pre_Render_Update(camera)`: `Mview-texture = PShadow * Mwrld-texture * Mcamera-wrld`; moves `Intensity` toward
    /// `DesiredIntensity` at `INTENSITY_RATE_OF_CHANGE` per second; installs the emissive; sets the mapper types and
    /// texture transforms. The texture size is taken from the texture width when unset (upstream dereferences the pass's
    /// texture unchecked).
    pub fn pre_render_update<H: TexProjHost>(&mut self, host: &H, camera: &Matrix3D) {
        let world_to_texture = self.projector.get_transform().get_orthogonal_inverse();
        let tmp = Matrix3D::multiply(&world_to_texture, camera);
        let view_to_texture: Matrix4x4 = matrix4x4_multiply(&self.projector.projection, &matrix4x4_from_matrix3d(&tmp));

        // update the current intensity by iterating it towards the desired intensity
        let frame_time = host.frame_time_ms() as f32 / 1000.0;
        let intensity_delta = self.desired_intensity - self.intensity;
        let max_intensity_delta = INTENSITY_RATE_OF_CHANGE * frame_time;
        if intensity_delta > max_intensity_delta {
            self.intensity += max_intensity_delta;
        } else if intensity_delta < -max_intensity_delta {
            self.intensity -= max_intensity_delta;
        } else {
            self.intensity = self.desired_intensity;
        }
        let actual_intensity = self.intensity * self.attenuation;

        // install the current intensity
        if self.get_flag(flags::ADDITIVE) {
            self.material.state.emissive = [actual_intensity; 3];
        } else {
            self.material.state.emissive = [1.0 - actual_intensity; 3];
        }

        // update the mappers
        if self.get_flag(flags::PERSPECTIVE) {
            self.projector.mapper.set_type(MappingType::PerspectiveProjection);
        } else {
            self.projector.mapper.set_type(MappingType::OrthoProjection);
        }
        if self.get_texture_size() == 0 {
            let tex = self.material_pass.get_texture(0).unwrap(); // MaterialPass->Peek_Texture()->Get_Width()
            self.set_texture_size(host.texture_width(tex));
            debug_assert!(self.get_texture_size() != 0);
        }
        let size = self.get_texture_size() as f32;
        self.projector.mapper.set_texture_transform(&view_to_texture, size);
        if let Some(m1) = self.mapper1.as_mut() {
            m1.set_texture_transform(&view_to_texture, size);
        }
    }

    /// `Update_WS_Bounding_Volume()`: the base's update, then the culling system's box is the axis-aligned box of
    /// the world volume.
    pub fn update_ws_bounding_volume(&mut self) {
        self.projector.update_ws_bounding_volume();
        // Tell our culling system that we've changed
        let extent = obbox_compute_axis_aligned_extent(&self.projector.world_bounding_volume);
        self.cull_box = AABox { center: self.projector.world_bounding_volume.center, extent };
    }

    /// `Get_Projection_Object_ID()`: NULL (derived shadow classes override it).
    pub fn get_projection_object_id(&self) -> Option<usize> {
        None
    }
}
