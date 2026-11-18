//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/light.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/light.h
//!
//! The `RenderObjClass` base (scene registration, `Set_Force_Visible`, the transform) is not in the verified port;
//! the light keeps its transform and force-visible flag itself. Not translated: `Save_W3D`, `Save`, `Load` and
//! the `PersistFactory` (they need `ChunkSaveClass` and the persist machinery; the game never saves a light) and
//! `Notify_Added`/`Notify_Removed` (scene registration, the scene is not in this area).

use crate::formats::w3d::vertmaterial::convert_color;
use crate::formats::w3d::w3d_file::*;
use crate::formats::w3d::W3dResult;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// `LightClass::LightType` (light.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightType {
    Point = 0,
    Directional,
    Spot,
}

/// `LightClass::FlagsType` (light.h): `NEAR_ATTENUATION = 0`, `FAR_ATTENUATION` (= 1).
/// Upstream quirk, kept: `Set_Flag`/`Get_Flag` use the enum value as the *mask*, so `NEAR_ATTENUATION` (0) can
/// never be set or read: `Get_Flag(NEAR_ATTENUATION)` is always false.
pub const NEAR_ATTENUATION: u32 = 0;
/// See [`NEAR_ATTENUATION`].
pub const FAR_ATTENUATION: u32 = 1;

/// `DEG_TO_RADF(45.0f)` (wwmath.h): `((float)x)*WWMATH_PI/180.0f`, evaluated left to right.
const DEG_TO_RADF_45: f32 = 45.0f32 * wwmath::WWMATH_PI / 180.0f32;

/// `class LightClass` (light.h): a light source; as a render object it has a transform.
#[derive(Clone, Debug)]
pub struct LightClass {
    type_: LightType,
    flags: u32,
    cast_shadows: bool,
    intensity: f32,
    ambient: Vector3,
    diffuse: Vector3,
    specular: Vector3,
    near_atten_start: f32,
    near_atten_end: f32,
    far_atten_start: f32,
    far_atten_end: f32,
    spot_angle: f32,
    spot_angle_cos: f32,
    spot_exponent: f32,
    spot_direction: Vector3,
    /// `RenderObjClass::Transform`.
    pub transform: Matrix3D,
    /// `RenderObjClass::Set_Force_Visible`.
    pub force_visible: bool,
}

impl LightClass {
    /// `LightClass::LightClass(type)` (light.cpp:73): the defaults of the initialiser list; a directional light
    /// has no position so it is forced visible ("culling cant work"). `Matrix3D` default is identity upstream.
    pub fn new(type_: LightType) -> Self {
        let mut identity = Matrix3D::default();
        identity.make_identity();
        LightClass {
            type_,
            flags: 0,
            cast_shadows: false,
            intensity: 1.0,
            ambient: Vector3::new(1.0, 1.0, 1.0),
            diffuse: Vector3::new(1.0, 1.0, 1.0),
            specular: Vector3::new(1.0, 1.0, 1.0),
            near_atten_start: 0.0,
            near_atten_end: 0.0,
            far_atten_start: 50.0,
            far_atten_end: 100.0,
            spot_angle: DEG_TO_RADF_45,
            spot_angle_cos: 0.707,
            spot_exponent: 1.0,
            spot_direction: Vector3::new(0.0, 0.0, 1.0),
            transform: identity,
            force_visible: type_ == LightType::Directional,
        }
    }

    // LightClass(const LightClass &) / operator= (light.cpp:98, 118) are `Clone`; Clone() (light.cpp:150) too.

    /// `Class_ID` is `CLASSID_LIGHT`; `Render` does nothing; `Is_Vertex_Processor` is true (light.h).
    pub fn is_vertex_processor(&self) -> bool {
        true
    }

    /// `LightClass::Get_Obj_Space_Bounding_Sphere` (light.cpp:174): centre 0, radius the attenuation range.
    pub fn get_obj_space_bounding_sphere(&self) -> (Vector3, f32) {
        (Vector3::new(0.0, 0.0, 0.0), self.get_attenuation_range())
    }

    /// `LightClass::Get_Obj_Space_Bounding_Box` (light.cpp:180): centre 0, extent (r, r, r).
    pub fn get_obj_space_bounding_box(&self) -> (Vector3, Vector3) {
        let r = self.get_attenuation_range();
        (Vector3::new(0.0, 0.0, 0.0), Vector3::new(r, r, r))
    }

    /// `Get_Type` (light.h).
    pub fn get_type(&self) -> LightType {
        self.type_
    }

    /// `Set_Intensity` (light.h).
    pub fn set_intensity(&mut self, inten: f32) {
        self.intensity = inten;
    }

    /// `Get_Intensity` (light.h).
    pub fn get_intensity(&self) -> f32 {
        self.intensity
    }

    /// `Set_Ambient` (light.h).
    pub fn set_ambient(&mut self, color: &Vector3) {
        self.ambient = *color;
    }

    /// `Get_Ambient` (light.h).
    pub fn get_ambient(&self) -> Vector3 {
        self.ambient
    }

    /// `Set_Diffuse` (light.h).
    pub fn set_diffuse(&mut self, color: &Vector3) {
        self.diffuse = *color;
    }

    /// `Get_Diffuse` (light.h).
    pub fn get_diffuse(&self) -> Vector3 {
        self.diffuse
    }

    /// `Set_Specular` (light.h).
    pub fn set_specular(&mut self, color: &Vector3) {
        self.specular = *color;
    }

    /// `Get_Specular` (light.h).
    pub fn get_specular(&self) -> Vector3 {
        self.specular
    }

    /// `Set_Far_Attenuation_Range(double, double)` (light.h): stored as floats.
    pub fn set_far_attenuation_range(&mut self, f_start: f64, f_end: f64) {
        self.far_atten_start = f_start as f32;
        self.far_atten_end = f_end as f32;
    }

    /// `Get_Far_Attenuation_Range(double &, double &)` (light.h).
    pub fn get_far_attenuation_range(&self) -> (f64, f64) {
        (f64::from(self.far_atten_start), f64::from(self.far_atten_end))
    }

    /// `Set_Near_Attenuation_Range` (light.h).
    pub fn set_near_attenuation_range(&mut self, n_start: f64, n_end: f64) {
        self.near_atten_start = n_start as f32;
        self.near_atten_end = n_end as f32;
    }

    /// `Get_Near_Attenuation_Range` (light.h).
    pub fn get_near_attenuation_range(&self) -> (f64, f64) {
        (f64::from(self.near_atten_start), f64::from(self.near_atten_end))
    }

    /// `Get_Attenuation_Range` (light.h): `FarAttenEnd`.
    pub fn get_attenuation_range(&self) -> f32 {
        self.far_atten_end
    }

    /// `Set_Flag` (light.h); see the quirk on [`NEAR_ATTENUATION`].
    pub fn set_flag(&mut self, flag: u32, onoff: bool) {
        if onoff {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// `Get_Flag` (light.h).
    pub fn get_flag(&self, flag: u32) -> bool {
        (self.flags & flag) != 0
    }

    /// `Enable_Shadows` (light.h).
    pub fn enable_shadows(&mut self, onoff: bool) {
        self.cast_shadows = onoff;
    }

    /// `Are_Shadows_Enabled` (light.h).
    pub fn are_shadows_enabled(&self) -> bool {
        self.cast_shadows
    }

    /// `Set_Spot_Angle` (light.h): also sets the cosine with `WWMath::Fast_Cos`.
    pub fn set_spot_angle(&mut self, a: f32) {
        self.spot_angle = a;
        self.spot_angle_cos = wwmath::fast_cos(a);
    }

    /// `Get_Spot_Angle` (light.h).
    pub fn get_spot_angle(&self) -> f32 {
        self.spot_angle
    }

    /// `Get_Spot_Angle_Cos` (light.h).
    pub fn get_spot_angle_cos(&self) -> f32 {
        self.spot_angle_cos
    }

    /// `Set_Spot_Direction` (light.h).
    pub fn set_spot_direction(&mut self, dir: &Vector3) {
        self.spot_direction = *dir;
    }

    /// `Get_Spot_Direction` (light.h).
    pub fn get_spot_direction(&self) -> Vector3 {
        self.spot_direction
    }

    /// `Set_Spot_Exponent` (light.h).
    pub fn set_spot_exponent(&mut self, k: f32) {
        self.spot_exponent = k;
    }

    /// `Get_Spot_Exponent` (light.h).
    pub fn get_spot_exponent(&self) -> f32 {
        self.spot_exponent
    }

    /// `LightClass::Load_W3D` (light.cpp:191), with `cload` inside the `W3D_CHUNK_LIGHT` chunk: the
    /// `W3D_CHUNK_LIGHT_INFO` chunk first (upstream asserts it is the first), then the spot, near and far
    /// attenuation chunks in any order. Reads are not checked, as upstream. Always `WW3D_ERROR_OK`.
    pub fn load_w3d(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<()> {
        cload.open_chunk()?;
        debug_assert!(cload.cur_chunk_id()? == W3D_CHUNK_LIGHT_INFO);
        let mut b = [0u8; W3dLightStruct::SIZE];
        cload.read_into(&mut b)?;
        let lightinfo = W3dLightStruct::from_bytes(&b);
        cload.close_chunk()?;

        match lightinfo.attributes & W3D_LIGHT_ATTRIBUTE_TYPE_MASK {
            W3D_LIGHT_ATTRIBUTE_POINT => self.type_ = LightType::Point,
            W3D_LIGHT_ATTRIBUTE_DIRECTIONAL => self.type_ = LightType::Directional,
            W3D_LIGHT_ATTRIBUTE_SPOT => self.type_ = LightType::Spot,
            _ => {}
        }

        self.enable_shadows((lightinfo.attributes & W3D_LIGHT_ATTRIBUTE_CAST_SHADOWS) == W3D_LIGHT_ATTRIBUTE_CAST_SHADOWS);
        self.set_intensity(lightinfo.intensity);

        // W3dUtilityClass::Convert_Color
        let c = convert_color(&lightinfo.ambient);
        self.set_ambient(&Vector3::new(c[0], c[1], c[2]));
        let c = convert_color(&lightinfo.diffuse);
        self.set_diffuse(&Vector3::new(c[0], c[1], c[2]));
        let c = convert_color(&lightinfo.specular);
        self.set_specular(&Vector3::new(c[0], c[1], c[2]));

        while cload.open_chunk()? {
            match cload.cur_chunk_id()? {
                W3D_CHUNK_SPOT_LIGHT_INFO => {
                    let mut sb = [0u8; W3dSpotLightStruct::SIZE];
                    cload.read_into(&mut sb)?;
                    let spotinfo = W3dSpotLightStruct::from_bytes(&sb);
                    self.set_spot_angle(spotinfo.spot_angle);
                    self.set_spot_exponent(spotinfo.spot_exponent);
                    // W3dUtilityClass::Convert_Vector
                    let d = spotinfo.spot_direction;
                    self.set_spot_direction(&Vector3::new(d.x, d.y, d.z));
                }
                W3D_CHUNK_NEAR_ATTENUATION => {
                    let mut ab = [0u8; W3dLightAttenuationStruct::SIZE];
                    cload.read_into(&mut ab)?;
                    let atteninfo = W3dLightAttenuationStruct::from_bytes(&ab);
                    self.set_flag(NEAR_ATTENUATION, true);
                    self.set_near_attenuation_range(f64::from(atteninfo.start), f64::from(atteninfo.end));
                }
                W3D_CHUNK_FAR_ATTENUATION => {
                    let mut ab = [0u8; W3dLightAttenuationStruct::SIZE];
                    cload.read_into(&mut ab)?;
                    let atteninfo = W3dLightAttenuationStruct::from_bytes(&ab);
                    self.set_flag(FAR_ATTENUATION, true);
                    self.set_far_attenuation_range(f64::from(atteninfo.start), f64::from(atteninfo.end));
                }
                _ => {}
            }
            cload.close_chunk()?;
        }
        Ok(())
    }
}
