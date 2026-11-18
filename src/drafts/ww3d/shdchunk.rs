//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Not a translation of one upstream file: the pieces the `shd*.cpp` shaders share, taken from
//! `GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdinterface.h` (`VertexStreamStruct`),
//! `GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/dx8fvf.h` (the three vertex layouts these shaders use; dx8fvf.cpp is
//! a DX8 file the earlier pass skipped) and `WWLib/chunkio.h` (`READ_MICRO_CHUNK`, `READ_MICRO_CHUNK_WWSTRING`).
//! The vertex-stream copy loops in the shaders are identical apart from the layout, so they are written once here
//! and each shader file names the function it uses; every shader's own `Copy_Vertex_Stream` body is the
//! corresponding function below.

use crate::ww::chunkio::ChunkLoad;
use crate::formats::w3d::W3dResult;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;

/// `MAX_TEXTURE_STAGES` (dx8wrapper.h).
pub const MAX_TEXTURE_STAGES: usize = 4;

/// `READ_MICRO_CHUNK_WWSTRING(cload, id, var)`: the micro-chunk's bytes up to the first NUL.
pub fn read_micro_wwstring(cload: &mut ChunkLoad<'_>) -> W3dResult<String> {
    let len = cload.cur_micro_chunk_length();
    let mut buf = vec![0u8; len as usize];
    cload.read(&mut buf, len)?;
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

/// `READ_MICRO_CHUNK(cload, id, Vector3 var)`: three little-endian floats.
pub fn read_micro_vector3(cload: &mut ChunkLoad<'_>) -> W3dResult<Vector3> {
    let mut b = [0u8; 12];
    cload.read_into(&mut b)?;
    let f = |i: usize| f32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    Ok(Vector3::new(f(0), f(4), f(8)))
}

/// `READ_MICRO_CHUNK(cload, id, Vector2 var)`: two little-endian floats.
pub fn read_micro_vector2(cload: &mut ChunkLoad<'_>) -> W3dResult<Vector2> {
    let mut b = [0u8; 8];
    cload.read_into(&mut b)?;
    let f = |i: usize| f32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    Ok(Vector2::new(f(0), f(4)))
}

/// `struct VertexStreamStruct` (shdinterface.h:63): the standard vertex data a mesh offers; a `None` stream is a
/// NULL pointer upstream.
#[derive(Clone, Debug, Default)]
pub struct VertexStreamStruct {
    pub locations: Option<Vec<Vector3>>,
    pub normals: Option<Vec<Vector3>>,
    pub uv: [Option<Vec<Vector2>>; MAX_TEXTURE_STAGES],
    pub diffuse_int: Option<Vec<u32>>,
    pub diffuse_float: Option<Vec<Vector4>>,
    pub s: Option<Vec<Vector3>>,
    pub t: Option<Vec<Vector3>>,
    pub sxt: Option<Vec<Vector3>>,
}

/// `VertexFormatXYZNDUV1` (dx8fvf.h:124).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexFormatXYZNDUV1 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub diffuse: u32,
    pub u1: f32,
    pub v1: f32,
}

/// `VertexFormatXYZNDUV1TG3` (dx8fvf.h:195): plus the S, T and SxT tangent-space basis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexFormatXYZNDUV1TG3 {
    pub base: VertexFormatXYZNDUV1,
    pub sx: f32,
    pub sy: f32,
    pub sz: f32,
    pub tx: f32,
    pub ty: f32,
    pub tz: f32,
    pub sxtx: f32,
    pub sxty: f32,
    pub sxtz: f32,
}

/// `VertexFormatXYZNDCUBEMAP` (dx8fvf.h:236): position, normal, diffuse (the cube-map coordinates are generated).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexFormatXYZNDCUBEMAP {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub diffuse: u32,
}

fn v3_or_zero(a: &Option<Vec<Vector3>>, i: usize) -> Vector3 {
    match a {
        Some(v) => v[i],
        None => Vector3::new(0.0, 0.0, 0.0),
    }
}

/// The position / diffuse / normal / UV0 part shared by every `Copy_Vertex_Stream` below: missing locations and
/// normals are zero, missing diffuse is `0xffffffff`, missing UV is zero.
fn copy_base(vss: &VertexStreamStruct, i: usize, with_uv: bool) -> VertexFormatXYZNDUV1 {
    let l = v3_or_zero(&vss.locations, i);
    let n = v3_or_zero(&vss.normals, i);
    let diffuse = match &vss.diffuse_int {
        Some(d) => d[i],
        None => 0xffff_ffff,
    };
    let (u1, v1) = if with_uv {
        match &vss.uv[0] {
            Some(uv) => (uv[i].x, uv[i].y),
            None => (0.0, 0.0),
        }
    } else {
        (0.0, 0.0)
    };
    VertexFormatXYZNDUV1 { x: l.x, y: l.y, z: l.z, nx: n.x, ny: n.y, nz: n.z, diffuse, u1, v1 }
}

/// `Copy_Vertex_Stream` of `Shd6SimpleClass`, `Shd6GlossMaskClass`, `Shd6BumpDiffClass`, `Shd6BumpSpecClass`
/// (all `VertexFormatXYZNDUV1`).
pub fn copy_vertex_stream_xyzn_duv1(vss: &VertexStreamStruct, vertex_count: usize) -> Vec<VertexFormatXYZNDUV1> {
    (0..vertex_count).map(|i| copy_base(vss, i, true)).collect()
}

/// `Copy_Vertex_Stream` of `Shd7BumpDiffClass`, `Shd7BumpSpecClass`, `Shd8BumpDiffClass`, `Shd8BumpSpecClass`
/// (`VertexFormatXYZNDUV1TG3`: the base plus S, T, SxT; a missing basis vector is zero).
pub fn copy_vertex_stream_xyzn_duv1_tg3(vss: &VertexStreamStruct, vertex_count: usize) -> Vec<VertexFormatXYZNDUV1TG3> {
    (0..vertex_count)
        .map(|i| {
            let s = v3_or_zero(&vss.s, i);
            let t = v3_or_zero(&vss.t, i);
            let sxt = v3_or_zero(&vss.sxt, i);
            VertexFormatXYZNDUV1TG3 {
                base: copy_base(vss, i, true),
                sx: s.x,
                sy: s.y,
                sz: s.z,
                tx: t.x,
                ty: t.y,
                tz: t.z,
                sxtx: sxt.x,
                sxty: sxt.y,
                sxtz: sxt.z,
            }
        })
        .collect()
}

/// `Copy_Vertex_Stream` of `Shd6CubeMapClass` (`VertexFormatXYZNDCUBEMAP`: no UVs).
pub fn copy_vertex_stream_cubemap(vss: &VertexStreamStruct, vertex_count: usize) -> Vec<VertexFormatXYZNDCUBEMAP> {
    (0..vertex_count)
        .map(|i| {
            let b = copy_base(vss, i, false);
            VertexFormatXYZNDCUBEMAP { x: b.x, y: b.y, z: b.z, nx: b.nx, ny: b.ny, nz: b.nz, diffuse: b.diffuse }
        })
        .collect()
}

/// The `D3DMATERIAL8` colours the shaders fill (the rest is zeroed): `Material->Ambient/Diffuse/Specular` and
/// `Power`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D3DMaterial8Colors {
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub specular: [f32; 3],
    pub power: f32,
}

/// The pixel-shader capability test the bump shaders' `Init` does (`ShdBumpDiffDefClass::Init`,
/// `ShdBumpSpecDefClass::Init`): pixel shader 1.1 or later -> `SHDVER_8`, else DOT3 -> `SHDVER_7`, else
/// `SHDVER_6`. Inputs are `DX8Wrapper::Get_Current_Caps()` values.
pub fn select_shader_version(ps_major: u32, ps_minor: u32, support_dot3: bool) -> super::shddef::SHDVER {
    if ps_major == 1 && ps_minor >= 1 {
        super::shddef::SHDVER::Ver8
    } else if support_dot3 {
        super::shddef::SHDVER::Ver7
    } else {
        super::shddef::SHDVER::Ver6
    }
}

/// `SHDDEF_CLASSID_*` (shdclassids.h).
pub const SHDDEF_CLASSID_DUMMY: u32 = 0;
pub const SHDDEF_CLASSID_SIMPLE: u32 = 1;
pub const SHDDEF_CLASSID_GLOSSMASK: u32 = 2;
pub const SHDDEF_CLASSID_BUMPSPEC: u32 = 3;
pub const SHDDEF_CLASSID_BUMPDIFF: u32 = 4;
pub const SHDDEF_CLASSID_CUBEMAP: u32 = 5;
pub const SHDDEF_CLASSID_LEGACYW3D: u32 = 6;
pub const SHDDEF_CLASSID_LAST: u32 = 7;
/// `SHD_CLASSID_DUMMY` / `SHD_CLASSID_LAST`.
pub const SHD_CLASSID_DUMMY: u32 = 0;
pub const SHD_CLASSID_LAST: u32 = 1;
