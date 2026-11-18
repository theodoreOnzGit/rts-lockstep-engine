//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdhwshader.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdhwshader.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdhw_constants.h
//!
//! Translated: `ShdHWVertexShader::Light` (it computes the vertex shader constant values from the light
//! environment; the `Set_Vertex_Shader_Constant` calls become the returned `(register, value)` list) and the constant
//! register numbers. Not translated (device / OS): `ShdHWShader::Shell_Run` (spawns `rspp.exe` through
//! `CreateProcess`), `Preprocess_And_Assemble_Shader_From_File` (`D3DXAssembleShaderFromFile`),
//! `ShdHWVertexShader::Create/Destroy` and `ShdHWPixelShader::Create/Destroy` (`CreateVertexShader`,
//! `DeleteVertexShader`, `CreatePixelShader`, `DeletePixelShader`).

use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;

/// `shdhw_constants.h`: vertex shader constant registers.
pub const CV_CONST: u32 = 0;
pub const CV_LIGHT_DIRECTION_0: u32 = 16;
pub const CV_LIGHT_DIRECTION_1: u32 = 17;
pub const CV_LIGHT_DIRECTION_2: u32 = 18;
pub const CV_LIGHT_DIRECTION_3: u32 = 19;
pub const CV_LIGHT_COLOR_0: u32 = 20;
pub const CV_LIGHT_COLOR_1: u32 = 21;
pub const CV_LIGHT_COLOR_2: u32 = 22;
pub const CV_LIGHT_COLOR_3: u32 = 23;
pub const CV_AMBIENT: u32 = 24;
pub const CV_DIFFUSE: u32 = 25;
pub const CV_SPECULAR: u32 = 26;

/// `LightEnvironmentClass::Get_Max_Lights()` (4: `MAX_LIGHTS`, lightenvironment.h).
pub const MAX_LIGHTS: usize = 4;

/// What `Light` reads from `rinfo.light_environment` when it is set and has lights.
#[derive(Clone, Debug)]
pub struct LightEnvInput {
    pub equivalent_ambient: Vector3,
    /// `Get_Light_Direction(i)` / `Get_Light_Diffuse(i)` for `i < Get_Light_Count()`.
    pub lights: Vec<(Vector3, Vector3)>,
}

/// One fixed-function light of `DX8Wrapper::Get_Render_State` (`state.LightEnable[i]`, `state.Lights[i]`).
#[derive(Clone, Copy, Debug)]
pub struct FixedFunctionLight {
    pub enabled: bool,
    /// `D3DLIGHT_DIRECTIONAL` (else a point/spot light, whose position is used).
    pub directional: bool,
    pub direction: Vector3,
    pub position: Vector3,
    /// `Diffuse.r/g/b`.
    pub diffuse: Vector3,
}

/// Where the lighting comes from in `Light`.
#[derive(Clone, Debug)]
pub enum LightSource {
    /// `rinfo.light_environment == NULL`: the ambient stays (1,1,1,1) and no light constants are written.
    None,
    /// A light environment with at least one light.
    Environment(LightEnvInput),
    /// A light environment with no lights (or whatever `lenv` holds with `Get_Light_Count() == 0`): the
    /// fixed-function state is used: `DX8Wrapper::Get_Ambient()` and up to four enabled lights.
    FixedFunction { ambient: Vector3, lights: [FixedFunctionLight; MAX_LIGHTS] },
}

/// `ShdHWVertexShader::Light(rinfo, ambient, diffuse, specular)` (shdhwshader.cpp:160): the vertex shader
/// constants to set, in the order upstream sets them. `cdir`/`ccol` are carried across loop iterations as in C++
/// (a light slot past the light count rewrites `ccol` as zero but resends the previous `cdir`; the first `cdir`,
/// uninitialised upstream, is (0,0,0,0) here).
pub fn light(source: &LightSource, ambient: &Vector4, diffuse: &Vector4, specular: &Vector4) -> Vec<(u32, Vector4)> {
    let mut out: Vec<(u32, Vector4)> = Vec::new();
    let mut light_ambient = Vector4 { x: 1.0, y: 1.0, z: 1.0, w: 1.0 };
    let mut cdir = Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
    let mut ccol = Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };

    match source {
        LightSource::None => {}
        LightSource::Environment(env) => {
            let mut num_lights = env.lights.len();
            let amb = env.equivalent_ambient;
            light_ambient = Vector4 { x: amb.x, y: amb.y, z: amb.z, w: 0.0 };
            if num_lights > MAX_LIGHTS {
                num_lights = MAX_LIGHTS;
            }
            for i in 0..MAX_LIGHTS {
                if i < num_lights {
                    let (dir, col) = env.lights[i];
                    cdir = Vector4 { x: dir.x, y: dir.y, z: dir.z, w: 1.0 };
                    ccol = Vector4 { x: col.x, y: col.y, z: col.z, w: 1.0 };
                } else {
                    ccol = Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
                }
                out.push((CV_LIGHT_DIRECTION_0 + i as u32, cdir));
                out.push((CV_LIGHT_COLOR_0 + i as u32, ccol));
            }
        }
        LightSource::FixedFunction { ambient: amb_col, lights } => {
            light_ambient.x = amb_col.x;
            light_ambient.y = amb_col.y;
            light_ambient.z = amb_col.z;
            for i in 0..MAX_LIGHTS {
                if lights[i].enabled {
                    if lights[i].directional {
                        let d = lights[i].direction;
                        cdir = Vector4 { x: -d.x, y: -d.y, z: -d.z, w: 0.0 };
                    } else {
                        let p = lights[i].position;
                        cdir = Vector4 { x: p.x, y: p.y, z: p.z, w: 0.0 };
                    }
                    // Vector4::Normalize()
                    let len2 = cdir.x * cdir.x + cdir.y * cdir.y + cdir.z * cdir.z + cdir.w * cdir.w;
                    if len2 != 0.0 {
                        let oolen = crate::ww::wwmath::inv_sqrt(len2);
                        cdir.x *= oolen;
                        cdir.y *= oolen;
                        cdir.z *= oolen;
                        cdir.w *= oolen;
                    }
                    let c = lights[i].diffuse;
                    ccol = Vector4 { x: c.x, y: c.y, z: c.z, w: 1.0 };
                } else {
                    ccol = Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
                }
                out.push((CV_LIGHT_DIRECTION_0 + i as u32, cdir));
                out.push((CV_LIGHT_COLOR_0 + i as u32, ccol));
            }
        }
    }

    light_ambient.x *= ambient.x;
    light_ambient.y *= ambient.y;
    light_ambient.z *= ambient.z;
    out.push((CV_AMBIENT, light_ambient));
    out.push((CV_DIFFUSE, *diffuse));
    out.push((CV_SPECULAR, *specular));
    out
}

/// `ShdHWVertexShader::Light(rinfo, ambient, diffuse)` (shdhwshader.cpp:249): specular (0,0,0,0).
pub fn light_no_specular(source: &LightSource, ambient: &Vector4, diffuse: &Vector4) -> Vec<(u32, Vector4)> {
    let specular = Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
    light(source, ambient, diffuse, &specular)
}
