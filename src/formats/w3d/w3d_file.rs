// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3d_file.h
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3d_obsolete.h
//   (and the IOVector3Struct / IOQuaternionStruct they use, from
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/iostruct.h)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, "Commando / G 3D Library", w3d_file.h by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The W3D file-format definitions: chunk ids, flags and the on-disk
//! structs, which upstream reads by copying the bytes into the C struct
//! (`cload.Read(&s, sizeof(s))`).
//!
//! Each struct here carries its exact C layout on the Win32 target: `SIZE`
//! (`sizeof`, padding included) and [`W3dField`]s with each member's
//! upstream name, `offsetof` and `sizeof`; [`W3dPod::read_le`] reads the
//! members from those offsets (little-endian), so it is what upstream's
//! `memcpy` of the same bytes gives. Every layout is checked against the
//! real header compiled by the C++ compiler (`tests/differential_w3d.rs`).
//!
//! Upstream types: `uint32` is `unsigned long`, 32 bits on Win32; `char`
//! arrays are kept as raw bytes (`[u8; N]`, not necessarily NUL-terminated);
//! `bool` operators and helper functions of the header that only build or
//! compare structs are not translated (except `W3D_MAKE_VERSION` and the
//! `Get` helpers as `const fn`s).
//!
//! Structs that end in `Data[1]` (`W3dAnimChannelStruct`, ...) are the
//! fixed part of a variable-length chunk: `sizeof` includes the first
//! element, which is how the loaders read them.

/// A member of a W3D struct: its upstream name, `offsetof` and `sizeof`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct W3dField {
    pub name: &'static str,
    pub offset: usize,
    pub size: usize,
}

/// A plain on-disk value: read from and written to its little-endian bytes.
pub trait W3dPod: Sized + Copy {
    /// `sizeof` on the Win32 target.
    const SIZE: usize;
    /// The value from the first `SIZE` bytes of `b` (as `memcpy` gives it).
    fn read_le(b: &[u8]) -> Self;
    /// Writes the value into the first `SIZE` bytes of `dst` (padding bytes
    /// of a struct are written as 0).
    fn write_le(&self, dst: &mut [u8]);
    /// The `SIZE` bytes of the value.
    fn to_bytes(&self) -> Vec<u8> {
        let mut v = vec![0u8; Self::SIZE];
        self.write_le(&mut v);
        v
    }
}

macro_rules! pod_int {
    ($($t:ty),*) => {$(
        impl W3dPod for $t {
            const SIZE: usize = std::mem::size_of::<$t>();
            fn read_le(b: &[u8]) -> Self {
                let mut a = [0u8; std::mem::size_of::<$t>()];
                a.copy_from_slice(&b[..Self::SIZE]);
                <$t>::from_le_bytes(a)
            }
            fn write_le(&self, dst: &mut [u8]) {
                dst[..Self::SIZE].copy_from_slice(&self.to_le_bytes());
            }
        }
    )*};
}
pod_int!(u8, i8, u16, i16, u32, i32, f32);

impl<T: W3dPod, const N: usize> W3dPod for [T; N] {
    const SIZE: usize = T::SIZE * N;
    fn read_le(b: &[u8]) -> Self {
        std::array::from_fn(|i| T::read_le(&b[i * T::SIZE..]))
    }
    fn write_le(&self, dst: &mut [u8]) {
        for (i, v) in self.iter().enumerate() {
            v.write_le(&mut dst[i * T::SIZE..]);
        }
    }
}

macro_rules! w3d_struct {
    (
        $(#[$m:meta])*
        $name:ident = $cname:literal, $size:expr, {
            $( $(#[$fm:meta])* $field:ident : $ty:ty = $fname:literal @ $off:expr ),* $(,)?
        }
    ) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $name {
            $( $(#[$fm])* pub $field: $ty, )*
        }

        impl W3dPod for $name {
            const SIZE: usize = $size;
            fn read_le(b: &[u8]) -> Self {
                $name { $( $field: <$ty as W3dPod>::read_le(&b[$off..]), )* }
            }
            fn write_le(&self, dst: &mut [u8]) {
                dst[..$size].fill(0);
                $( self.$field.write_le(&mut dst[$off..]); )*
            }
        }

        impl $name {
            /// The upstream struct name.
            pub const C_NAME: &'static str = $cname;
            /// The members: upstream name, `offsetof`, `sizeof`.
            pub const FIELDS: &'static [W3dField] = &[
                $( W3dField { name: $fname, offset: $off, size: <$ty as W3dPod>::SIZE }, )*
            ];
            /// The struct as upstream's `memcpy` of `b` gives it.
            pub fn from_bytes(b: &[u8; $size]) -> Self {
                <Self as W3dPod>::read_le(b)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Versions, names
// ---------------------------------------------------------------------------

/// `W3D_MAKE_VERSION(major, minor)`.
pub const fn w3d_make_version(major: u32, minor: u32) -> u32 {
    (major << 16) | minor
}
/// `W3D_GET_MAJOR_VERSION(ver)`.
pub const fn w3d_get_major_version(ver: u32) -> u32 {
    ver >> 16
}
/// `W3D_GET_MINOR_VERSION(ver)`.
pub const fn w3d_get_minor_version(ver: u32) -> u32 {
    ver & 0xFFFF
}
/// `W3D_NAME_LEN`.
pub const W3D_NAME_LEN: usize = 16;

pub const W3D_CURRENT_MESH_VERSION: u32 = w3d_make_version(4, 2);
pub const W3D_CURRENT_HTREE_VERSION: u32 = w3d_make_version(4, 1);
pub const W3D_CURRENT_HANIM_VERSION: u32 = w3d_make_version(4, 1);
pub const W3D_CURRENT_COMPRESSED_HANIM_VERSION: u32 = w3d_make_version(0, 1);
pub const W3D_CURRENT_MORPH_HANIM_VERSION: u32 = w3d_make_version(0, 1);
pub const W3D_CURRENT_HMODEL_VERSION: u32 = w3d_make_version(4, 2);
pub const W3D_CURRENT_COLLECTION_VERSION: u32 = w3d_make_version(4, 2);
pub const W3D_CURRENT_PLACEHOLDER_VERSION: u32 = w3d_make_version(1, 0);
pub const W3D_CURRENT_TRANSFORM_VERSION: u32 = w3d_make_version(1, 0);
pub const W3D_CURRENT_LIGHT_VERSION: u32 = w3d_make_version(1, 0);
pub const W3D_CURRENT_EMITTER_VERSION: u32 = 0x0002_0000;
pub const W3D_CURRENT_AGGREGATE_VERSION: u32 = 0x0001_0003;
pub const W3D_CURRENT_HLOD_VERSION: u32 = w3d_make_version(1, 0);
pub const W3D_BOX_CURRENT_VERSION: u32 = w3d_make_version(1, 0);
pub const W3D_NULL_OBJECT_CURRENT_VERSION: u32 = w3d_make_version(1, 0);
pub const W3D_CURRENT_SOUNDROBJ_VERSION: u32 = 0x0001_0000;
pub const W3D_CURRENT_SHDMESH_VERSION: u32 = 0x0001_0000;

// ---------------------------------------------------------------------------
// Chunk ids (w3d_file.h, then the obsolete ones of w3d_obsolete.h)
// ---------------------------------------------------------------------------

pub const W3D_CHUNK_MESH: u32 = 0x0000_0000;
pub const W3D_CHUNK_VERTICES: u32 = 0x0000_0002;
pub const W3D_CHUNK_VERTEX_NORMALS: u32 = 0x0000_0003;
pub const W3D_CHUNK_MESH_USER_TEXT: u32 = 0x0000_000C;
pub const W3D_CHUNK_VERTEX_INFLUENCES: u32 = 0x0000_000E;
pub const W3D_CHUNK_MESH_HEADER3: u32 = 0x0000_001F;
pub const W3D_CHUNK_TRIANGLES: u32 = 0x0000_0020;
pub const W3D_CHUNK_VERTEX_SHADE_INDICES: u32 = 0x0000_0022;
pub const W3D_CHUNK_PRELIT_UNLIT: u32 = 0x0000_0023;
pub const W3D_CHUNK_PRELIT_VERTEX: u32 = 0x0000_0024;
pub const W3D_CHUNK_PRELIT_LIGHTMAP_MULTI_PASS: u32 = 0x0000_0025;
pub const W3D_CHUNK_PRELIT_LIGHTMAP_MULTI_TEXTURE: u32 = 0x0000_0026;
pub const W3D_CHUNK_MATERIAL_INFO: u32 = 0x0000_0028;
pub const W3D_CHUNK_SHADERS: u32 = 0x0000_0029;
pub const W3D_CHUNK_VERTEX_MATERIALS: u32 = 0x0000_002A;
pub const W3D_CHUNK_VERTEX_MATERIAL: u32 = 0x0000_002B;
pub const W3D_CHUNK_VERTEX_MATERIAL_NAME: u32 = 0x0000_002C;
pub const W3D_CHUNK_VERTEX_MATERIAL_INFO: u32 = 0x0000_002D;
pub const W3D_CHUNK_VERTEX_MAPPER_ARGS0: u32 = 0x0000_002E;
pub const W3D_CHUNK_VERTEX_MAPPER_ARGS1: u32 = 0x0000_002F;
pub const W3D_CHUNK_TEXTURES: u32 = 0x0000_0030;
pub const W3D_CHUNK_TEXTURE: u32 = 0x0000_0031;
pub const W3D_CHUNK_TEXTURE_NAME: u32 = 0x0000_0032;
pub const W3D_CHUNK_TEXTURE_INFO: u32 = 0x0000_0033;
pub const W3D_CHUNK_MATERIAL_PASS: u32 = 0x0000_0038;
pub const W3D_CHUNK_VERTEX_MATERIAL_IDS: u32 = 0x0000_0039;
pub const W3D_CHUNK_SHADER_IDS: u32 = 0x0000_003A;
pub const W3D_CHUNK_DCG: u32 = 0x0000_003B;
pub const W3D_CHUNK_DIG: u32 = 0x0000_003C;
pub const W3D_CHUNK_SCG: u32 = 0x0000_003E;
pub const W3D_CHUNK_TEXTURE_STAGE: u32 = 0x0000_0048;
pub const W3D_CHUNK_TEXTURE_IDS: u32 = 0x0000_0049;
pub const W3D_CHUNK_STAGE_TEXCOORDS: u32 = 0x0000_004A;
pub const W3D_CHUNK_PER_FACE_TEXCOORD_IDS: u32 = 0x0000_004B;
pub const W3D_CHUNK_DEFORM: u32 = 0x0000_0058;
pub const W3D_CHUNK_DEFORM_SET: u32 = 0x0000_0059;
pub const W3D_CHUNK_DEFORM_KEYFRAME: u32 = 0x0000_005A;
pub const W3D_CHUNK_DEFORM_DATA: u32 = 0x0000_005B;
pub const W3D_CHUNK_PS2_SHADERS: u32 = 0x0000_0080;
pub const W3D_CHUNK_AABTREE: u32 = 0x0000_0090;
pub const W3D_CHUNK_AABTREE_HEADER: u32 = 0x0000_0091;
pub const W3D_CHUNK_AABTREE_POLYINDICES: u32 = 0x0000_0092;
pub const W3D_CHUNK_AABTREE_NODES: u32 = 0x0000_0093;
pub const W3D_CHUNK_HIERARCHY: u32 = 0x0000_0100;
pub const W3D_CHUNK_HIERARCHY_HEADER: u32 = 0x0000_0101;
pub const W3D_CHUNK_PIVOTS: u32 = 0x0000_0102;
pub const W3D_CHUNK_PIVOT_FIXUPS: u32 = 0x0000_0103;
pub const W3D_CHUNK_ANIMATION: u32 = 0x0000_0200;
pub const W3D_CHUNK_ANIMATION_HEADER: u32 = 0x0000_0201;
pub const W3D_CHUNK_ANIMATION_CHANNEL: u32 = 0x0000_0202;
pub const W3D_CHUNK_BIT_CHANNEL: u32 = 0x0000_0203;
pub const W3D_CHUNK_COMPRESSED_ANIMATION: u32 = 0x0000_0280;
pub const W3D_CHUNK_COMPRESSED_ANIMATION_HEADER: u32 = 0x0000_0281;
pub const W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL: u32 = 0x0000_0282;
pub const W3D_CHUNK_COMPRESSED_BIT_CHANNEL: u32 = 0x0000_0283;
pub const W3D_CHUNK_MORPH_ANIMATION: u32 = 0x0000_02C0;
pub const W3D_CHUNK_MORPHANIM_HEADER: u32 = 0x0000_02C1;
pub const W3D_CHUNK_MORPHANIM_CHANNEL: u32 = 0x0000_02C2;
pub const W3D_CHUNK_MORPHANIM_POSENAME: u32 = 0x0000_02C3;
pub const W3D_CHUNK_MORPHANIM_KEYDATA: u32 = 0x0000_02C4;
pub const W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA: u32 = 0x0000_02C5;
pub const W3D_CHUNK_HMODEL: u32 = 0x0000_0300;
pub const W3D_CHUNK_HMODEL_HEADER: u32 = 0x0000_0301;
pub const W3D_CHUNK_NODE: u32 = 0x0000_0302;
pub const W3D_CHUNK_COLLISION_NODE: u32 = 0x0000_0303;
pub const W3D_CHUNK_SKIN_NODE: u32 = 0x0000_0304;
pub const OBSOLETE_W3D_CHUNK_HMODEL_AUX_DATA: u32 = 0x0000_0305;
pub const OBSOLETE_W3D_CHUNK_SHADOW_NODE: u32 = 0x0000_0306;
pub const W3D_CHUNK_LODMODEL: u32 = 0x0000_0400;
pub const W3D_CHUNK_LODMODEL_HEADER: u32 = 0x0000_0401;
pub const W3D_CHUNK_LOD: u32 = 0x0000_0402;
pub const W3D_CHUNK_COLLECTION: u32 = 0x0000_0420;
pub const W3D_CHUNK_COLLECTION_HEADER: u32 = 0x0000_0421;
pub const W3D_CHUNK_COLLECTION_OBJ_NAME: u32 = 0x0000_0422;
pub const W3D_CHUNK_PLACEHOLDER: u32 = 0x0000_0423;
pub const W3D_CHUNK_TRANSFORM_NODE: u32 = 0x0000_0424;
pub const W3D_CHUNK_POINTS: u32 = 0x0000_0440;
pub const W3D_CHUNK_LIGHT: u32 = 0x0000_0460;
pub const W3D_CHUNK_LIGHT_INFO: u32 = 0x0000_0461;
pub const W3D_CHUNK_SPOT_LIGHT_INFO: u32 = 0x0000_0462;
pub const W3D_CHUNK_NEAR_ATTENUATION: u32 = 0x0000_0463;
pub const W3D_CHUNK_FAR_ATTENUATION: u32 = 0x0000_0464;
pub const W3D_CHUNK_EMITTER: u32 = 0x0000_0500;
pub const W3D_CHUNK_EMITTER_HEADER: u32 = 0x0000_0501;
pub const W3D_CHUNK_EMITTER_USER_DATA: u32 = 0x0000_0502;
pub const W3D_CHUNK_EMITTER_INFO: u32 = 0x0000_0503;
pub const W3D_CHUNK_EMITTER_INFOV2: u32 = 0x0000_0504;
pub const W3D_CHUNK_EMITTER_PROPS: u32 = 0x0000_0505;
pub const OBSOLETE_W3D_CHUNK_EMITTER_COLOR_KEYFRAME: u32 = 0x0000_0506;
pub const OBSOLETE_W3D_CHUNK_EMITTER_OPACITY_KEYFRAME: u32 = 0x0000_0507;
pub const OBSOLETE_W3D_CHUNK_EMITTER_SIZE_KEYFRAME: u32 = 0x0000_0508;
pub const W3D_CHUNK_EMITTER_LINE_PROPERTIES: u32 = 0x0000_0509;
pub const W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES: u32 = 0x0000_050A;
pub const W3D_CHUNK_EMITTER_FRAME_KEYFRAMES: u32 = 0x0000_050B;
pub const W3D_CHUNK_EMITTER_BLUR_TIME_KEYFRAMES: u32 = 0x0000_050C;
pub const W3D_CHUNK_EMITTER_EXTRA_INFO: u32 = 0x0000_050D;
pub const W3D_CHUNK_AGGREGATE: u32 = 0x0000_0600;
pub const W3D_CHUNK_AGGREGATE_HEADER: u32 = 0x0000_0601;
pub const W3D_CHUNK_AGGREGATE_INFO: u32 = 0x0000_0602;
pub const W3D_CHUNK_TEXTURE_REPLACER_INFO: u32 = 0x0000_0603;
pub const W3D_CHUNK_AGGREGATE_CLASS_INFO: u32 = 0x0000_0604;
pub const W3D_CHUNK_HLOD: u32 = 0x0000_0700;
pub const W3D_CHUNK_HLOD_HEADER: u32 = 0x0000_0701;
pub const W3D_CHUNK_HLOD_LOD_ARRAY: u32 = 0x0000_0702;
pub const W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER: u32 = 0x0000_0703;
pub const W3D_CHUNK_HLOD_SUB_OBJECT: u32 = 0x0000_0704;
pub const W3D_CHUNK_HLOD_AGGREGATE_ARRAY: u32 = 0x0000_0705;
pub const W3D_CHUNK_HLOD_PROXY_ARRAY: u32 = 0x0000_0706;
pub const W3D_CHUNK_BOX: u32 = 0x0000_0740;
pub const W3D_CHUNK_SPHERE: u32 = 0x0000_0741;
pub const W3D_CHUNK_RING: u32 = 0x0000_0742;
pub const W3D_CHUNK_NULL_OBJECT: u32 = 0x0000_0750;
pub const W3D_CHUNK_LIGHTSCAPE: u32 = 0x0000_0800;
pub const W3D_CHUNK_LIGHTSCAPE_LIGHT: u32 = 0x0000_0801;
pub const W3D_CHUNK_LIGHT_TRANSFORM: u32 = 0x0000_0802;
pub const W3D_CHUNK_DAZZLE: u32 = 0x0000_0900;
pub const W3D_CHUNK_DAZZLE_NAME: u32 = 0x0000_0901;
pub const W3D_CHUNK_DAZZLE_TYPENAME: u32 = 0x0000_0902;
pub const W3D_CHUNK_SOUNDROBJ: u32 = 0x0000_0A00;
pub const W3D_CHUNK_SOUNDROBJ_HEADER: u32 = 0x0000_0A01;
pub const W3D_CHUNK_SOUNDROBJ_DEFINITION: u32 = 0x0000_0A02;
pub const W3D_CHUNK_SHDMESH: u32 = 0x0000_0B00;
pub const W3D_CHUNK_SHDMESH_NAME: u32 = 0x0000_0B01;
pub const W3D_CHUNK_SHDMESH_HEADER: u32 = 0x0000_0B02;
pub const W3D_CHUNK_SHDMESH_USER_TEXT: u32 = 0x0000_0B03;
pub const W3D_CHUNK_SHDSUBMESH: u32 = 0x0000_0B20;
pub const W3D_CHUNK_SHDSUBMESH_HEADER: u32 = 0x0000_0B21;
pub const W3D_CHUNK_SHDSUBMESH_SHADER: u32 = 0x0000_0B40;
pub const W3D_CHUNK_SHDSUBMESH_SHADER_CLASSID: u32 = 0x0000_0B41;
pub const W3D_CHUNK_SHDSUBMESH_SHADER_DEF: u32 = 0x0000_0B42;
pub const W3D_CHUNK_SHDSUBMESH_VERTICES: u32 = 0x0000_0B43;
pub const W3D_CHUNK_SHDSUBMESH_VERTEX_NORMALS: u32 = 0x0000_0B44;
pub const W3D_CHUNK_SHDSUBMESH_TRIANGLES: u32 = 0x0000_0B45;
pub const W3D_CHUNK_SHDSUBMESH_VERTEX_SHADE_INDICES: u32 = 0x0000_0B46;
pub const W3D_CHUNK_SHDSUBMESH_UV0: u32 = 0x0000_0B47;
pub const W3D_CHUNK_SHDSUBMESH_UV1: u32 = 0x0000_0B48;
pub const W3D_CHUNK_SHDSUBMESH_TANGENT_BASIS_S: u32 = 0x0000_0B49;
pub const W3D_CHUNK_SHDSUBMESH_TANGENT_BASIS_T: u32 = 0x0000_0B4A;
pub const W3D_CHUNK_SHDSUBMESH_TANGENT_BASIS_SXT: u32 = 0x0000_0B4B;
pub const W3D_CHUNK_SHDSUBMESH_VERTEX_COLOR: u32 = 0x0000_0B4C;
pub const W3D_CHUNK_SHDSUBMESH_VERTEX_INFLUENCES: u32 = 0x0000_0B4D;

// w3d_obsolete.h
pub const W3D_CHUNK_MESH_HEADER: u32 = 0x0000_0001;
pub const W3D_CHUNK_SURRENDER_NORMALS: u32 = 0x0000_0004;
pub const W3D_CHUNK_TEXCOORDS: u32 = 0x0000_0005;
pub const O_W3D_CHUNK_MATERIALS: u32 = 0x0000_0006;
pub const O_W3D_CHUNK_TRIANGLES: u32 = 0x0000_0007;
pub const O_W3D_CHUNK_QUADRANGLES: u32 = 0x0000_0008;
pub const O_W3D_CHUNK_SURRENDER_TRIANGLES: u32 = 0x0000_0009;
pub const O_W3D_CHUNK_POV_TRIANGLES: u32 = 0x0000_000A;
pub const O_W3D_CHUNK_POV_QUADRANGLES: u32 = 0x0000_000B;
pub const W3D_CHUNK_VERTEX_COLORS: u32 = 0x0000_000D;
pub const W3D_CHUNK_DAMAGE: u32 = 0x0000_000F;
pub const W3D_CHUNK_DAMAGE_HEADER: u32 = 0x0000_0010;
pub const W3D_CHUNK_DAMAGE_VERTICES: u32 = 0x0000_0011;
pub const W3D_CHUNK_DAMAGE_COLORS: u32 = 0x0000_0012;
pub const W3D_CHUNK_DAMAGE_MATERIALS: u32 = 0x0000_0013;
pub const O_W3D_CHUNK_MATERIALS2: u32 = 0x0000_0014;
pub const W3D_CHUNK_MATERIALS3: u32 = 0x0000_0015;
pub const W3D_CHUNK_MATERIAL3: u32 = 0x0000_0016;
pub const W3D_CHUNK_MATERIAL3_NAME: u32 = 0x0000_0017;
pub const W3D_CHUNK_MATERIAL3_INFO: u32 = 0x0000_0018;
pub const W3D_CHUNK_MATERIAL3_DC_MAP: u32 = 0x0000_0019;
pub const W3D_CHUNK_MAP3_FILENAME: u32 = 0x0000_001A;
pub const W3D_CHUNK_MAP3_INFO: u32 = 0x0000_001B;
pub const W3D_CHUNK_MATERIAL3_DI_MAP: u32 = 0x0000_001C;
pub const W3D_CHUNK_MATERIAL3_SC_MAP: u32 = 0x0000_001D;
pub const W3D_CHUNK_MATERIAL3_SI_MAP: u32 = 0x0000_001E;
pub const W3D_CHUNK_PER_TRI_MATERIALS: u32 = 0x0000_0021;

// ---------------------------------------------------------------------------
// Flags and enumerations
// ---------------------------------------------------------------------------

// W3dVertexMaterialStruct::Attributes
pub const W3DVERTMAT_USE_DEPTH_CUE: u32 = 0x0000_0001;
pub const W3DVERTMAT_ARGB_EMISSIVE_ONLY: u32 = 0x0000_0002;
pub const W3DVERTMAT_COPY_SPECULAR_TO_DIFFUSE: u32 = 0x0000_0004;
pub const W3DVERTMAT_DEPTH_CUE_TO_ALPHA: u32 = 0x0000_0008;
pub const W3DVERTMAT_STAGE0_MAPPING_MASK: u32 = 0x00FF_0000;
pub const W3DVERTMAT_STAGE1_MAPPING_MASK: u32 = 0x0000_FF00;
pub const W3DVERTMAT_PSX_MASK: u32 = 0xFF00_0000;

// W3dShaderStruct values
pub const W3DSHADER_DEPTHCOMPARE_PASS_LEQUAL: u8 = 3;
pub const W3DSHADER_DEPTHMASK_WRITE_ENABLE: u8 = 1;
pub const W3DSHADER_ALPHATEST_DISABLE: u8 = 0;
pub const W3DSHADER_ALPHATEST_ENABLE: u8 = 1;
pub const W3DSHADER_DESTBLENDFUNC_ZERO: u8 = 0;
pub const W3DSHADER_DESTBLENDFUNC_ONE: u8 = 1;
pub const W3DSHADER_PRIGRADIENT_MODULATE: u8 = 1;
pub const W3DSHADER_SECGRADIENT_DISABLE: u8 = 0;
pub const W3DSHADER_SRCBLENDFUNC_ONE: u8 = 1;
pub const W3DSHADER_TEXTURING_DISABLE: u8 = 0;
pub const W3DSHADER_TEXTURING_ENABLE: u8 = 1;
pub const W3DSHADER_DETAILCOLORFUNC_DISABLE: u8 = 0;
pub const W3DSHADER_DETAILALPHAFUNC_DISABLE: u8 = 0;

// W3dTextureInfoStruct::Attributes, AnimType
pub const W3DTEXTURE_PUBLISH: u16 = 0x0001;
pub const W3DTEXTURE_RESIZE_OBSOLETE: u16 = 0x0002;
pub const W3DTEXTURE_NO_LOD: u16 = 0x0004;
pub const W3DTEXTURE_CLAMP_U: u16 = 0x0008;
pub const W3DTEXTURE_CLAMP_V: u16 = 0x0010;
pub const W3DTEXTURE_ALPHA_BITMAP: u16 = 0x0020;
pub const W3DTEXTURE_MIP_LEVELS_MASK: u16 = 0x00c0;
pub const W3DTEXTURE_HINT_SHIFT: u16 = 8;
pub const W3DTEXTURE_HINT_MASK: u16 = 0xff00;
pub const W3DTEXTURE_TYPE_MASK: u16 = 0x1000;
pub const W3DTEXTURE_ANIM_LOOP: u16 = 0x0000;
pub const W3DTEXTURE_ANIM_PINGPONG: u16 = 0x0001;
pub const W3DTEXTURE_ANIM_ONCE: u16 = 0x0002;
pub const W3DTEXTURE_ANIM_MANUAL: u16 = 0x0003;

// W3dMeshHeader3Struct::Attributes
pub const W3D_MESH_FLAG_NONE: u32 = 0x0000_0000;
pub const W3D_MESH_FLAG_COLLISION_BOX: u32 = 0x0000_0001;
pub const W3D_MESH_FLAG_SKIN: u32 = 0x0000_0002;
pub const W3D_MESH_FLAG_SHADOW: u32 = 0x0000_0004;
pub const W3D_MESH_FLAG_ALIGNED: u32 = 0x0000_0008;
pub const W3D_MESH_FLAG_COLLISION_TYPE_MASK: u32 = 0x0000_0FF0;
pub const W3D_MESH_FLAG_COLLISION_TYPE_SHIFT: u32 = 4;
pub const W3D_MESH_FLAG_COLLISION_TYPE_PHYSICAL: u32 = 0x0000_0010;
pub const W3D_MESH_FLAG_COLLISION_TYPE_PROJECTILE: u32 = 0x0000_0020;
pub const W3D_MESH_FLAG_COLLISION_TYPE_VIS: u32 = 0x0000_0040;
pub const W3D_MESH_FLAG_COLLISION_TYPE_CAMERA: u32 = 0x0000_0080;
pub const W3D_MESH_FLAG_COLLISION_TYPE_VEHICLE: u32 = 0x0000_0100;
pub const W3D_MESH_FLAG_HIDDEN: u32 = 0x0000_1000;
pub const W3D_MESH_FLAG_TWO_SIDED: u32 = 0x0000_2000;
pub const OBSOLETE_W3D_MESH_FLAG_LIGHTMAPPED: u32 = 0x0000_4000;
pub const W3D_MESH_FLAG_CAST_SHADOW: u32 = 0x0000_8000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_MASK: u32 = 0x00FF_0000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_NORMAL: u32 = 0x0000_0000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED: u32 = 0x0001_0000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN: u32 = 0x0002_0000;
pub const OBSOLETE_W3D_MESH_FLAG_GEOMETRY_TYPE_SHADOW: u32 = 0x0003_0000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_AABOX: u32 = 0x0004_0000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_OBBOX: u32 = 0x0005_0000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ORIENTED: u32 = 0x0006_0000;
pub const W3D_MESH_FLAG_PRELIT_MASK: u32 = 0x0F00_0000;
pub const W3D_MESH_FLAG_PRELIT_UNLIT: u32 = 0x0100_0000;
pub const W3D_MESH_FLAG_PRELIT_VERTEX: u32 = 0x0200_0000;
pub const W3D_MESH_FLAG_PRELIT_LIGHTMAP_MULTI_PASS: u32 = 0x0400_0000;
pub const W3D_MESH_FLAG_PRELIT_LIGHTMAP_MULTI_TEXTURE: u32 = 0x0800_0000;
pub const W3D_MESH_FLAG_SHATTERABLE: u32 = 0x1000_0000;
pub const W3D_MESH_FLAG_NPATCHABLE: u32 = 0x2000_0000;

// W3dMeshHeader3Struct::VertexChannels, FaceChannels
pub const W3D_VERTEX_CHANNEL_LOCATION: u32 = 0x0000_0001;
pub const W3D_VERTEX_CHANNEL_NORMAL: u32 = 0x0000_0002;
pub const W3D_VERTEX_CHANNEL_TEXCOORD: u32 = 0x0000_0004;
pub const W3D_VERTEX_CHANNEL_COLOR: u32 = 0x0000_0008;
pub const W3D_VERTEX_CHANNEL_BONEID: u32 = 0x0000_0010;
pub const W3D_FACE_CHANNEL_FACE: u32 = 0x0000_0001;

// W3dMeshHeader3Struct::SortLevel
pub const SORT_LEVEL_NONE: i32 = 0;
pub const MAX_SORT_LEVEL: i32 = 32;
pub const SORT_LEVEL_BIN1: i32 = 20;
pub const SORT_LEVEL_BIN2: i32 = 15;
pub const SORT_LEVEL_BIN3: i32 = 10;

// W3dTriStruct::Attributes: W3D_SURFACE_TYPES
pub const SURFACE_TYPE_MAX: u32 = 32;

// Animation channel types (W3dAnimChannelStruct::Flags and friends)
pub const ANIM_CHANNEL_X: u32 = 0;
pub const ANIM_CHANNEL_Y: u32 = 1;
pub const ANIM_CHANNEL_Z: u32 = 2;
pub const ANIM_CHANNEL_XR: u32 = 3;
pub const ANIM_CHANNEL_YR: u32 = 4;
pub const ANIM_CHANNEL_ZR: u32 = 5;
pub const ANIM_CHANNEL_Q: u32 = 6;
pub const ANIM_CHANNEL_TIMECODED_X: u32 = 7;
pub const ANIM_CHANNEL_TIMECODED_Y: u32 = 8;
pub const ANIM_CHANNEL_TIMECODED_Z: u32 = 9;
pub const ANIM_CHANNEL_TIMECODED_Q: u32 = 10;
pub const ANIM_CHANNEL_ADAPTIVEDELTA_X: u32 = 11;
pub const ANIM_CHANNEL_ADAPTIVEDELTA_Y: u32 = 12;
pub const ANIM_CHANNEL_ADAPTIVEDELTA_Z: u32 = 13;
pub const ANIM_CHANNEL_ADAPTIVEDELTA_Q: u32 = 14;
pub const ANIM_FLAVOR_TIMECODED: u32 = 0;
pub const ANIM_FLAVOR_ADAPTIVE_DELTA: u32 = 1;
pub const ANIM_FLAVOR_VALID: u32 = 2;
pub const BIT_CHANNEL_VIS: u32 = 0;
pub const BIT_CHANNEL_TIMECODED_VIS: u32 = 1;
pub const W3D_TIMECODED_BINARY_MOVEMENT_FLAG: u32 = 0x8000_0000;
pub const W3D_TIMECODED_BIT_MASK: u32 = 0x8000_0000;

// Lights, boxes
pub const W3D_LIGHT_ATTRIBUTE_TYPE_MASK: u32 = 0x0000_00FF;
pub const W3D_LIGHT_ATTRIBUTE_POINT: u32 = 0x0000_0001;
pub const W3D_LIGHT_ATTRIBUTE_DIRECTIONAL: u32 = 0x0000_0002;
pub const W3D_LIGHT_ATTRIBUTE_SPOT: u32 = 0x0000_0003;
pub const W3D_LIGHT_ATTRIBUTE_CAST_SHADOWS: u32 = 0x0000_0100;
pub const W3D_BOX_ATTRIBUTE_ORIENTED: u32 = 0x0000_0001;
pub const W3D_BOX_ATTRIBUTE_ALIGNED: u32 = 0x0000_0002;
pub const W3D_BOX_ATTRIBUTE_COLLISION_TYPE_MASK: u32 = 0x0000_0FF0;
pub const W3D_BOX_ATTRIBUTE_COLLISION_TYPE_SHIFT: u32 = 4;

// Obsolete material mapping types (W3dMap3Struct::MappingType)
pub const W3DMAPPING_UV: u16 = 0;
pub const W3DMAPPING_ENVIRONMENT: u16 = 1;

/// `MESH_PATH_ENTRIES`.
pub const MESH_PATH_ENTRIES: usize = 15;
/// `MESH_PATH_ENTRY_LEN`.
pub const MESH_PATH_ENTRY_LEN: usize = W3D_NAME_LEN * 2;

// ---------------------------------------------------------------------------
// Structs (w3d_file.h order)
// ---------------------------------------------------------------------------

type Name = [u8; W3D_NAME_LEN];
type Name2 = [u8; 2 * W3D_NAME_LEN];

w3d_struct! {
    /// `struct W3dChunkHeader`.
    W3dChunkHeader = "W3dChunkHeader", 8, {
        chunk_type: u32 = "ChunkType" @ 0,
        chunk_size: u32 = "ChunkSize" @ 4,
    }
}
w3d_struct! {
    /// `W3dVectorStruct` (`IOVector3Struct`).
    W3dVectorStruct = "W3dVectorStruct", 12, {
        x: f32 = "X" @ 0,
        y: f32 = "Y" @ 4,
        z: f32 = "Z" @ 8,
    }
}
w3d_struct! {
    /// `W3dQuaternionStruct` (`IOQuaternionStruct`).
    W3dQuaternionStruct = "W3dQuaternionStruct", 16, {
        q: [f32; 4] = "Q" @ 0,
    }
}
w3d_struct! {
    /// `struct W3dTexCoordStruct`.
    W3dTexCoordStruct = "W3dTexCoordStruct", 8, {
        u: f32 = "U" @ 0,
        v: f32 = "V" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dRGBStruct` (padded to 4 bytes).
    W3dRGBStruct = "W3dRGBStruct", 4, {
        r: u8 = "R" @ 0,
        g: u8 = "G" @ 1,
        b: u8 = "B" @ 2,
        pad: u8 = "pad" @ 3,
    }
}
w3d_struct! {
    /// `struct W3dRGBAStruct`.
    W3dRGBAStruct = "W3dRGBAStruct", 4, {
        r: u8 = "R" @ 0,
        g: u8 = "G" @ 1,
        b: u8 = "B" @ 2,
        a: u8 = "A" @ 3,
    }
}
w3d_struct! {
    /// `struct W3dMaterialInfoStruct`.
    W3dMaterialInfoStruct = "W3dMaterialInfoStruct", 16, {
        pass_count: u32 = "PassCount" @ 0,
        vertex_material_count: u32 = "VertexMaterialCount" @ 4,
        shader_count: u32 = "ShaderCount" @ 8,
        texture_count: u32 = "TextureCount" @ 12,
    }
}
w3d_struct! {
    /// `struct W3dVertexMaterialStruct`.
    W3dVertexMaterialStruct = "W3dVertexMaterialStruct", 32, {
        attributes: u32 = "Attributes" @ 0,
        ambient: W3dRGBStruct = "Ambient" @ 4,
        diffuse: W3dRGBStruct = "Diffuse" @ 8,
        specular: W3dRGBStruct = "Specular" @ 12,
        emissive: W3dRGBStruct = "Emissive" @ 16,
        shininess: f32 = "Shininess" @ 20,
        opacity: f32 = "Opacity" @ 24,
        translucency: f32 = "Translucency" @ 28,
    }
}
w3d_struct! {
    /// `struct W3dShaderStruct`.
    W3dShaderStruct = "W3dShaderStruct", 16, {
        depth_compare: u8 = "DepthCompare" @ 0,
        depth_mask: u8 = "DepthMask" @ 1,
        color_mask: u8 = "ColorMask" @ 2,
        dest_blend: u8 = "DestBlend" @ 3,
        fog_func: u8 = "FogFunc" @ 4,
        pri_gradient: u8 = "PriGradient" @ 5,
        sec_gradient: u8 = "SecGradient" @ 6,
        src_blend: u8 = "SrcBlend" @ 7,
        texturing: u8 = "Texturing" @ 8,
        detail_color_func: u8 = "DetailColorFunc" @ 9,
        detail_alpha_func: u8 = "DetailAlphaFunc" @ 10,
        shader_preset: u8 = "ShaderPreset" @ 11,
        alpha_test: u8 = "AlphaTest" @ 12,
        post_detail_color_func: u8 = "PostDetailColorFunc" @ 13,
        post_detail_alpha_func: u8 = "PostDetailAlphaFunc" @ 14,
        pad: [u8; 1] = "pad" @ 15,
    }
}
w3d_struct! {
    /// `struct W3dPS2ShaderStruct`.
    W3dPS2ShaderStruct = "W3dPS2ShaderStruct", 12, {
        depth_compare: u8 = "DepthCompare" @ 0,
        depth_mask: u8 = "DepthMask" @ 1,
        pri_gradient: u8 = "PriGradient" @ 2,
        texturing: u8 = "Texturing" @ 3,
        alpha_test: u8 = "AlphaTest" @ 4,
        a_param: u8 = "AParam" @ 5,
        b_param: u8 = "BParam" @ 6,
        c_param: u8 = "CParam" @ 7,
        d_param: u8 = "DParam" @ 8,
        pad: [u8; 3] = "pad" @ 9,
    }
}
w3d_struct! {
    /// `struct W3dTextureInfoStruct`.
    W3dTextureInfoStruct = "W3dTextureInfoStruct", 12, {
        attributes: u16 = "Attributes" @ 0,
        anim_type: u16 = "AnimType" @ 2,
        frame_count: u32 = "FrameCount" @ 4,
        frame_rate: f32 = "FrameRate" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dTriStruct`.
    W3dTriStruct = "W3dTriStruct", 32, {
        vindex: [u32; 3] = "Vindex" @ 0,
        attributes: u32 = "Attributes" @ 12,
        normal: W3dVectorStruct = "Normal" @ 16,
        dist: f32 = "Dist" @ 28,
    }
}
w3d_struct! {
    /// `struct W3dMeshHeader3Struct`.
    W3dMeshHeader3Struct = "W3dMeshHeader3Struct", 116, {
        version: u32 = "Version" @ 0,
        attributes: u32 = "Attributes" @ 4,
        mesh_name: Name = "MeshName" @ 8,
        container_name: Name = "ContainerName" @ 24,
        num_tris: u32 = "NumTris" @ 40,
        num_vertices: u32 = "NumVertices" @ 44,
        num_materials: u32 = "NumMaterials" @ 48,
        num_damage_stages: u32 = "NumDamageStages" @ 52,
        sort_level: i32 = "SortLevel" @ 56,
        prelit_version: u32 = "PrelitVersion" @ 60,
        future_counts: [u32; 1] = "FutureCounts" @ 64,
        vertex_channels: u32 = "VertexChannels" @ 68,
        face_channels: u32 = "FaceChannels" @ 72,
        min: W3dVectorStruct = "Min" @ 76,
        max: W3dVectorStruct = "Max" @ 88,
        sph_center: W3dVectorStruct = "SphCenter" @ 100,
        sph_radius: f32 = "SphRadius" @ 112,
    }
}
w3d_struct! {
    /// `struct W3dVertInfStruct`.
    W3dVertInfStruct = "W3dVertInfStruct", 8, {
        bone_idx: u16 = "BoneIdx" @ 0,
        pad: [u8; 6] = "Pad" @ 2,
    }
}
w3d_struct! {
    /// `struct W3dMeshDeform`.
    W3dMeshDeform = "W3dMeshDeform", 20, {
        set_count: u32 = "SetCount" @ 0,
        alpha_passes: u32 = "AlphaPasses" @ 4,
        reserved: [u32; 3] = "reserved" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dDeformSetInfo`.
    W3dDeformSetInfo = "W3dDeformSetInfo", 12, {
        keyframe_count: u32 = "KeyframeCount" @ 0,
        flags: u32 = "flags" @ 4,
        reserved: [u32; 1] = "reserved" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dDeformKeyframeInfo`.
    W3dDeformKeyframeInfo = "W3dDeformKeyframeInfo", 16, {
        deform_percent: f32 = "DeformPercent" @ 0,
        data_count: u32 = "DataCount" @ 4,
        reserved: [u32; 2] = "reserved" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dDeformData`.
    W3dDeformData = "W3dDeformData", 28, {
        vertex_index: u32 = "VertexIndex" @ 0,
        position: W3dVectorStruct = "Position" @ 4,
        color: W3dRGBAStruct = "Color" @ 16,
        reserved: [u32; 2] = "reserved" @ 20,
    }
}
w3d_struct! {
    /// `struct W3dMeshAABTreeHeader`.
    W3dMeshAABTreeHeader = "W3dMeshAABTreeHeader", 32, {
        node_count: u32 = "NodeCount" @ 0,
        poly_count: u32 = "PolyCount" @ 4,
        padding: [u32; 6] = "Padding" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dMeshAABTreeNode`.
    W3dMeshAABTreeNode = "W3dMeshAABTreeNode", 32, {
        min: W3dVectorStruct = "Min" @ 0,
        max: W3dVectorStruct = "Max" @ 12,
        front_or_poly0: u32 = "FrontOrPoly0" @ 24,
        back_or_poly_count: u32 = "BackOrPolyCount" @ 28,
    }
}
w3d_struct! {
    /// `struct W3dHierarchyStruct`.
    W3dHierarchyStruct = "W3dHierarchyStruct", 36, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
        num_pivots: u32 = "NumPivots" @ 20,
        center: W3dVectorStruct = "Center" @ 24,
    }
}
w3d_struct! {
    /// `struct W3dPivotStruct`.
    W3dPivotStruct = "W3dPivotStruct", 60, {
        name: Name = "Name" @ 0,
        parent_idx: u32 = "ParentIdx" @ 16,
        translation: W3dVectorStruct = "Translation" @ 20,
        euler_angles: W3dVectorStruct = "EulerAngles" @ 32,
        rotation: W3dQuaternionStruct = "Rotation" @ 44,
    }
}
w3d_struct! {
    /// `struct W3dPivotFixupStruct`.
    W3dPivotFixupStruct = "W3dPivotFixupStruct", 48, {
        tm: [[f32; 3]; 4] = "TM" @ 0,
    }
}
w3d_struct! {
    /// `struct W3dAnimHeaderStruct`.
    W3dAnimHeaderStruct = "W3dAnimHeaderStruct", 44, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
        hierarchy_name: Name = "HierarchyName" @ 20,
        num_frames: u32 = "NumFrames" @ 36,
        frame_rate: u32 = "FrameRate" @ 40,
    }
}
w3d_struct! {
    /// `struct W3dCompressedAnimHeaderStruct`.
    W3dCompressedAnimHeaderStruct = "W3dCompressedAnimHeaderStruct", 44, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
        hierarchy_name: Name = "HierarchyName" @ 20,
        num_frames: u32 = "NumFrames" @ 36,
        frame_rate: u16 = "FrameRate" @ 40,
        flavor: u16 = "Flavor" @ 42,
    }
}
w3d_struct! {
    /// `struct W3dAnimChannelStruct` (the fixed part; `Data` runs on).
    W3dAnimChannelStruct = "W3dAnimChannelStruct", 16, {
        first_frame: u16 = "FirstFrame" @ 0,
        last_frame: u16 = "LastFrame" @ 2,
        vector_len: u16 = "VectorLen" @ 4,
        flags: u16 = "Flags" @ 6,
        pivot: u16 = "Pivot" @ 8,
        pad: u16 = "pad" @ 10,
        data: [f32; 1] = "Data" @ 12,
    }
}
w3d_struct! {
    /// `struct W3dBitChannelStruct` (the fixed part; `Data` runs on).
    W3dBitChannelStruct = "W3dBitChannelStruct", 10, {
        first_frame: u16 = "FirstFrame" @ 0,
        last_frame: u16 = "LastFrame" @ 2,
        flags: u16 = "Flags" @ 4,
        pivot: u16 = "Pivot" @ 6,
        default_val: u8 = "DefaultVal" @ 8,
        data: [u8; 1] = "Data" @ 9,
    }
}
w3d_struct! {
    /// `struct W3dTimeCodedAnimChannelStruct` (the fixed part).
    W3dTimeCodedAnimChannelStruct = "W3dTimeCodedAnimChannelStruct", 12, {
        num_time_codes: u32 = "NumTimeCodes" @ 0,
        pivot: u16 = "Pivot" @ 4,
        vector_len: u8 = "VectorLen" @ 6,
        flags: u8 = "Flags" @ 7,
        data: [u32; 1] = "Data" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dTimeCodedBitChannelStruct` (the fixed part).
    W3dTimeCodedBitChannelStruct = "W3dTimeCodedBitChannelStruct", 12, {
        num_time_codes: u32 = "NumTimeCodes" @ 0,
        pivot: u16 = "Pivot" @ 4,
        flags: u8 = "Flags" @ 6,
        default_val: u8 = "DefaultVal" @ 7,
        data: [u32; 1] = "Data" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dAdaptiveDeltaAnimChannelStruct` (the fixed part).
    W3dAdaptiveDeltaAnimChannelStruct = "W3dAdaptiveDeltaAnimChannelStruct", 16, {
        num_frames: u32 = "NumFrames" @ 0,
        pivot: u16 = "Pivot" @ 4,
        vector_len: u8 = "VectorLen" @ 6,
        flags: u8 = "Flags" @ 7,
        scale: f32 = "Scale" @ 8,
        data: [u32; 1] = "Data" @ 12,
    }
}
w3d_struct! {
    /// `struct W3dMorphAnimHeaderStruct`.
    W3dMorphAnimHeaderStruct = "W3dMorphAnimHeaderStruct", 48, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
        hierarchy_name: Name = "HierarchyName" @ 20,
        frame_count: u32 = "FrameCount" @ 36,
        frame_rate: f32 = "FrameRate" @ 40,
        channel_count: u32 = "ChannelCount" @ 44,
    }
}
w3d_struct! {
    /// `struct W3dMorphAnimKeyStruct`.
    W3dMorphAnimKeyStruct = "W3dMorphAnimKeyStruct", 8, {
        morph_frame: u32 = "MorphFrame" @ 0,
        pose_frame: u32 = "PoseFrame" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dHModelHeaderStruct` (2 bytes of padding at the end).
    W3dHModelHeaderStruct = "W3dHModelHeaderStruct", 40, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
        hierarchy_name: Name = "HierarchyName" @ 20,
        num_connections: u16 = "NumConnections" @ 36,
    }
}
w3d_struct! {
    /// `struct W3dHModelNodeStruct`.
    W3dHModelNodeStruct = "W3dHModelNodeStruct", 18, {
        render_obj_name: Name = "RenderObjName" @ 0,
        pivot_idx: u16 = "PivotIdx" @ 16,
    }
}
w3d_struct! {
    /// `struct W3dLODModelHeaderStruct` (2 bytes of padding at the end).
    W3dLODModelHeaderStruct = "W3dLODModelHeaderStruct", 24, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
        num_lods: u16 = "NumLODs" @ 20,
    }
}
w3d_struct! {
    /// `struct W3dLODStruct`.
    W3dLODStruct = "W3dLODStruct", 40, {
        render_obj_name: Name2 = "RenderObjName" @ 0,
        lod_min: f32 = "LODMin" @ 32,
        lod_max: f32 = "LODMax" @ 36,
    }
}
w3d_struct! {
    /// `struct W3dCollectionHeaderStruct`.
    W3dCollectionHeaderStruct = "W3dCollectionHeaderStruct", 32, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
        render_object_count: u32 = "RenderObjectCount" @ 20,
        pad: [u32; 2] = "pad" @ 24,
    }
}
w3d_struct! {
    /// `struct W3dPlaceholderStruct`.
    W3dPlaceholderStruct = "W3dPlaceholderStruct", 56, {
        version: u32 = "version" @ 0,
        transform: [[f32; 3]; 4] = "transform" @ 4,
        name_len: u32 = "name_len" @ 52,
    }
}
w3d_struct! {
    /// `struct W3dTransformNodeStruct`.
    W3dTransformNodeStruct = "W3dTransformNodeStruct", 56, {
        version: u32 = "version" @ 0,
        transform: [[f32; 3]; 4] = "transform" @ 4,
        name_len: u32 = "name_len" @ 52,
    }
}
w3d_struct! {
    /// `struct W3dLightStruct`.
    W3dLightStruct = "W3dLightStruct", 24, {
        attributes: u32 = "Attributes" @ 0,
        unused: u32 = "Unused" @ 4,
        ambient: W3dRGBStruct = "Ambient" @ 8,
        diffuse: W3dRGBStruct = "Diffuse" @ 12,
        specular: W3dRGBStruct = "Specular" @ 16,
        intensity: f32 = "Intensity" @ 20,
    }
}
w3d_struct! {
    /// `struct W3dSpotLightStruct`.
    W3dSpotLightStruct = "W3dSpotLightStruct", 20, {
        spot_direction: W3dVectorStruct = "SpotDirection" @ 0,
        spot_angle: f32 = "SpotAngle" @ 12,
        spot_exponent: f32 = "SpotExponent" @ 16,
    }
}
w3d_struct! {
    /// `struct W3dLightAttenuationStruct`.
    W3dLightAttenuationStruct = "W3dLightAttenuationStruct", 8, {
        start: f32 = "Start" @ 0,
        end: f32 = "End" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dLightTransformStruct`.
    W3dLightTransformStruct = "W3dLightTransformStruct", 48, {
        transform: [[f32; 4]; 3] = "Transform" @ 0,
    }
}
w3d_struct! {
    /// `struct W3dEmitterHeaderStruct`.
    W3dEmitterHeaderStruct = "W3dEmitterHeaderStruct", 20, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dEmitterUserInfoStruct` (the fixed part; 3 bytes of padding).
    W3dEmitterUserInfoStruct = "W3dEmitterUserInfoStruct", 12, {
        type_: u32 = "Type" @ 0,
        sizeof_string_param: u32 = "SizeofStringParam" @ 4,
        string_param: [u8; 1] = "StringParam" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dEmitterInfoStruct`.
    W3dEmitterInfoStruct = "W3dEmitterInfoStruct", 332, {
        texture_filename: [u8; 260] = "TextureFilename" @ 0,
        start_size: f32 = "StartSize" @ 260,
        end_size: f32 = "EndSize" @ 264,
        lifetime: f32 = "Lifetime" @ 268,
        emission_rate: f32 = "EmissionRate" @ 272,
        max_emissions: f32 = "MaxEmissions" @ 276,
        velocity_random: f32 = "VelocityRandom" @ 280,
        position_random: f32 = "PositionRandom" @ 284,
        fade_time: f32 = "FadeTime" @ 288,
        gravity: f32 = "Gravity" @ 292,
        elasticity: f32 = "Elasticity" @ 296,
        velocity: W3dVectorStruct = "Velocity" @ 300,
        acceleration: W3dVectorStruct = "Acceleration" @ 312,
        start_color: W3dRGBAStruct = "StartColor" @ 324,
        end_color: W3dRGBAStruct = "EndColor" @ 328,
    }
}
w3d_struct! {
    /// `struct W3dEmitterExtraInfoStruct`.
    W3dEmitterExtraInfoStruct = "W3dEmitterExtraInfoStruct", 40, {
        future_start_time: f32 = "FutureStartTime" @ 0,
        padding: [u32; 9] = "Padding" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dVolumeRandomizerStruct`.
    W3dVolumeRandomizerStruct = "W3dVolumeRandomizerStruct", 32, {
        class_id: u32 = "ClassID" @ 0,
        value1: f32 = "Value1" @ 4,
        value2: f32 = "Value2" @ 8,
        value3: f32 = "Value3" @ 12,
        reserved: [u32; 4] = "reserved" @ 16,
    }
}
w3d_struct! {
    /// `struct W3dEmitterInfoStructV2`.
    W3dEmitterInfoStructV2 = "W3dEmitterInfoStructV2", 124, {
        burst_size: u32 = "BurstSize" @ 0,
        creation_volume: W3dVolumeRandomizerStruct = "CreationVolume" @ 4,
        vel_random: W3dVolumeRandomizerStruct = "VelRandom" @ 36,
        outward_vel: f32 = "OutwardVel" @ 68,
        vel_inherit: f32 = "VelInherit" @ 72,
        shader: W3dShaderStruct = "Shader" @ 76,
        render_mode: u32 = "RenderMode" @ 92,
        frame_mode: u32 = "FrameMode" @ 96,
        reserved: [u32; 6] = "reserved" @ 100,
    }
}
w3d_struct! {
    /// `struct W3dEmitterPropertyStruct`.
    W3dEmitterPropertyStruct = "W3dEmitterPropertyStruct", 40, {
        color_keyframes: u32 = "ColorKeyframes" @ 0,
        opacity_keyframes: u32 = "OpacityKeyframes" @ 4,
        size_keyframes: u32 = "SizeKeyframes" @ 8,
        color_random: W3dRGBAStruct = "ColorRandom" @ 12,
        opacity_random: f32 = "OpacityRandom" @ 16,
        size_random: f32 = "SizeRandom" @ 20,
        reserved: [u32; 4] = "reserved" @ 24,
    }
}
w3d_struct! {
    /// `struct W3dEmitterColorKeyframeStruct`.
    W3dEmitterColorKeyframeStruct = "W3dEmitterColorKeyframeStruct", 8, {
        time: f32 = "Time" @ 0,
        color: W3dRGBAStruct = "Color" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dEmitterOpacityKeyframeStruct`.
    W3dEmitterOpacityKeyframeStruct = "W3dEmitterOpacityKeyframeStruct", 8, {
        time: f32 = "Time" @ 0,
        opacity: f32 = "Opacity" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dEmitterSizeKeyframeStruct`.
    W3dEmitterSizeKeyframeStruct = "W3dEmitterSizeKeyframeStruct", 8, {
        time: f32 = "Time" @ 0,
        size: f32 = "Size" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dEmitterRotationHeaderStruct`.
    W3dEmitterRotationHeaderStruct = "W3dEmitterRotationHeaderStruct", 16, {
        keyframe_count: u32 = "KeyframeCount" @ 0,
        random: f32 = "Random" @ 4,
        orientation_random: f32 = "OrientationRandom" @ 8,
        reserved: [u32; 1] = "Reserved" @ 12,
    }
}
w3d_struct! {
    /// `struct W3dEmitterRotationKeyframeStruct`.
    W3dEmitterRotationKeyframeStruct = "W3dEmitterRotationKeyframeStruct", 8, {
        time: f32 = "Time" @ 0,
        rotation: f32 = "Rotation" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dEmitterFrameHeaderStruct`.
    W3dEmitterFrameHeaderStruct = "W3dEmitterFrameHeaderStruct", 16, {
        keyframe_count: u32 = "KeyframeCount" @ 0,
        random: f32 = "Random" @ 4,
        reserved: [u32; 2] = "Reserved" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dEmitterFrameKeyframeStruct`.
    W3dEmitterFrameKeyframeStruct = "W3dEmitterFrameKeyframeStruct", 8, {
        time: f32 = "Time" @ 0,
        frame: f32 = "Frame" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dEmitterBlurTimeHeaderStruct`.
    W3dEmitterBlurTimeHeaderStruct = "W3dEmitterBlurTimeHeaderStruct", 12, {
        keyframe_count: u32 = "KeyframeCount" @ 0,
        random: f32 = "Random" @ 4,
        reserved: [u32; 1] = "Reserved" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dEmitterBlurTimeKeyframeStruct`.
    W3dEmitterBlurTimeKeyframeStruct = "W3dEmitterBlurTimeKeyframeStruct", 8, {
        time: f32 = "Time" @ 0,
        blur_time: f32 = "BlurTime" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dEmitterLinePropertiesStruct`.
    W3dEmitterLinePropertiesStruct = "W3dEmitterLinePropertiesStruct", 64, {
        flags: u32 = "Flags" @ 0,
        subdivision_level: u32 = "SubdivisionLevel" @ 4,
        noise_amplitude: f32 = "NoiseAmplitude" @ 8,
        merge_abort_factor: f32 = "MergeAbortFactor" @ 12,
        texture_tile_factor: f32 = "TextureTileFactor" @ 16,
        u_per_sec: f32 = "UPerSec" @ 20,
        v_per_sec: f32 = "VPerSec" @ 24,
        reserved: [u32; 9] = "Reserved" @ 28,
    }
}
w3d_struct! {
    /// `struct W3dAggregateHeaderStruct`.
    W3dAggregateHeaderStruct = "W3dAggregateHeaderStruct", 20, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dAggregateInfoStruct`.
    W3dAggregateInfoStruct = "W3dAggregateInfoStruct", 36, {
        base_model_name: Name2 = "BaseModelName" @ 0,
        subobject_count: u32 = "SubobjectCount" @ 32,
    }
}
w3d_struct! {
    /// `struct W3dAggregateSubobjectStruct`.
    W3dAggregateSubobjectStruct = "W3dAggregateSubobjectStruct", 64, {
        subobject_name: Name2 = "SubobjectName" @ 0,
        bone_name: Name2 = "BoneName" @ 32,
    }
}
w3d_struct! {
    /// `struct W3dTextureReplacerHeaderStruct`.
    W3dTextureReplacerHeaderStruct = "W3dTextureReplacerHeaderStruct", 4, {
        replaced_textures_count: u32 = "ReplacedTexturesCount" @ 0,
    }
}
w3d_struct! {
    /// `struct W3dTextureReplacerStruct`.
    W3dTextureReplacerStruct = "W3dTextureReplacerStruct", 1492, {
        mesh_path: [[u8; MESH_PATH_ENTRY_LEN]; MESH_PATH_ENTRIES] = "MeshPath" @ 0,
        bone_path: [[u8; MESH_PATH_ENTRY_LEN]; MESH_PATH_ENTRIES] = "BonePath" @ 480,
        old_texture_name: [u8; 260] = "OldTextureName" @ 960,
        new_texture_name: [u8; 260] = "NewTextureName" @ 1220,
        texture_params: W3dTextureInfoStruct = "TextureParams" @ 1480,
    }
}
w3d_struct! {
    /// `struct W3dAggregateMiscInfo`.
    W3dAggregateMiscInfo = "W3dAggregateMiscInfo", 20, {
        original_class_id: u32 = "OriginalClassID" @ 0,
        flags: u32 = "Flags" @ 4,
        reserved: [u32; 3] = "reserved" @ 8,
    }
}
w3d_struct! {
    /// `struct W3dHLodHeaderStruct`.
    W3dHLodHeaderStruct = "W3dHLodHeaderStruct", 40, {
        version: u32 = "Version" @ 0,
        lod_count: u32 = "LodCount" @ 4,
        name: Name = "Name" @ 8,
        hierarchy_name: Name = "HierarchyName" @ 24,
    }
}
w3d_struct! {
    /// `struct W3dHLodArrayHeaderStruct`.
    W3dHLodArrayHeaderStruct = "W3dHLodArrayHeaderStruct", 8, {
        model_count: u32 = "ModelCount" @ 0,
        max_screen_size: f32 = "MaxScreenSize" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dHLodSubObjectStruct`.
    W3dHLodSubObjectStruct = "W3dHLodSubObjectStruct", 36, {
        bone_index: u32 = "BoneIndex" @ 0,
        name: Name2 = "Name" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dBoxStruct`.
    W3dBoxStruct = "W3dBoxStruct", 68, {
        version: u32 = "Version" @ 0,
        attributes: u32 = "Attributes" @ 4,
        name: Name2 = "Name" @ 8,
        color: W3dRGBStruct = "Color" @ 40,
        center: W3dVectorStruct = "Center" @ 44,
        extent: W3dVectorStruct = "Extent" @ 56,
    }
}
w3d_struct! {
    /// `struct W3dNullObjectStruct`.
    W3dNullObjectStruct = "W3dNullObjectStruct", 48, {
        version: u32 = "Version" @ 0,
        attributes: u32 = "Attributes" @ 4,
        pad: [u32; 2] = "pad" @ 8,
        name: Name2 = "Name" @ 16,
    }
}
w3d_struct! {
    /// `struct W3dSoundRObjHeaderStruct`.
    W3dSoundRObjHeaderStruct = "W3dSoundRObjHeaderStruct", 56, {
        version: u32 = "Version" @ 0,
        name: Name = "Name" @ 4,
        flags: u32 = "Flags" @ 20,
        padding: [u32; 8] = "Padding" @ 24,
    }
}
w3d_struct! {
    /// `struct W3dShdMeshHeaderStruct`.
    W3dShdMeshHeaderStruct = "W3dShdMeshHeaderStruct", 80, {
        version: u32 = "Version" @ 0,
        attributes: u32 = "Attributes" @ 4,
        num_tris: u32 = "NumTris" @ 8,
        num_vertices: u32 = "NumVertices" @ 12,
        num_sub_meshes: u32 = "NumSubMeshes" @ 16,
        future_counts: [u32; 5] = "FutureCounts" @ 20,
        box_min: W3dVectorStruct = "BoxMin" @ 40,
        box_max: W3dVectorStruct = "BoxMax" @ 52,
        sph_center: W3dVectorStruct = "SphCenter" @ 64,
        sph_radius: f32 = "SphRadius" @ 76,
    }
}
w3d_struct! {
    /// `struct W3dShdSubMeshHeaderStruct`.
    W3dShdSubMeshHeaderStruct = "W3dShdSubMeshHeaderStruct", 56, {
        num_tris: u32 = "NumTris" @ 0,
        num_vertices: u32 = "NumVertices" @ 4,
        future_counts: [u32; 2] = "FutureCounts" @ 8,
        box_min: W3dVectorStruct = "BoxMin" @ 16,
        box_max: W3dVectorStruct = "BoxMax" @ 28,
        sph_center: W3dVectorStruct = "SphCenter" @ 40,
        sph_radius: f32 = "SphRadius" @ 52,
    }
}

// --- w3d_obsolete.h ---------------------------------------------------------

w3d_struct! {
    /// `struct W3dMaterialStruct` (obsolete; 1 byte of padding at the end).
    W3dMaterialStruct = "W3dMaterialStruct", 56, {
        material_name: Name = "MaterialName" @ 0,
        primary_name: Name = "PrimaryName" @ 16,
        secondary_name: Name = "SecondaryName" @ 32,
        render_flags: u32 = "RenderFlags" @ 48,
        red: u8 = "Red" @ 52,
        green: u8 = "Green" @ 53,
        blue: u8 = "Blue" @ 54,
    }
}
w3d_struct! {
    /// `struct W3dMaterial2Struct` (obsolete).
    W3dMaterial2Struct = "W3dMaterial2Struct", 72, {
        material_name: Name = "MaterialName" @ 0,
        primary_name: Name = "PrimaryName" @ 16,
        secondary_name: Name = "SecondaryName" @ 32,
        render_flags: u32 = "RenderFlags" @ 48,
        red: u8 = "Red" @ 52,
        green: u8 = "Green" @ 53,
        blue: u8 = "Blue" @ 54,
        alpha: u8 = "Alpha" @ 55,
        primary_num_frames: u16 = "PrimaryNumFrames" @ 56,
        secondary_num_frames: u16 = "SecondaryNumFrames" @ 58,
        pad: [u8; 12] = "Pad" @ 60,
    }
}
w3d_struct! {
    /// `struct W3dMaterial3Struct` (obsolete; read by `read_v3_materials`).
    W3dMaterial3Struct = "W3dMaterial3Struct", 44, {
        attributes: u32 = "Attributes" @ 0,
        diffuse_color: W3dRGBStruct = "DiffuseColor" @ 4,
        specular_color: W3dRGBStruct = "SpecularColor" @ 8,
        emissive_coefficients: W3dRGBStruct = "EmissiveCoefficients" @ 12,
        ambient_coefficients: W3dRGBStruct = "AmbientCoefficients" @ 16,
        diffuse_coefficients: W3dRGBStruct = "DiffuseCoefficients" @ 20,
        specular_coefficients: W3dRGBStruct = "SpecularCoefficients" @ 24,
        shininess: f32 = "Shininess" @ 28,
        opacity: f32 = "Opacity" @ 32,
        translucency: f32 = "Translucency" @ 36,
        fog_coeff: f32 = "FogCoeff" @ 40,
    }
}
w3d_struct! {
    /// `struct W3dMap3Struct` (obsolete; read by `read_v3_materials`).
    W3dMap3Struct = "W3dMap3Struct", 8, {
        mapping_type: u16 = "MappingType" @ 0,
        frame_count: u16 = "FrameCount" @ 2,
        frame_rate: f32 = "FrameRate" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dSurrenderTriStruct` (obsolete).
    W3dSurrenderTriStruct = "W3dSurrenderTriStruct", 68, {
        vindex: [u32; 3] = "Vindex" @ 0,
        tex_coord: [W3dTexCoordStruct; 3] = "TexCoord" @ 12,
        material_idx: u32 = "MaterialIdx" @ 36,
        normal: W3dVectorStruct = "Normal" @ 40,
        attributes: u32 = "Attributes" @ 52,
        gouraud: [W3dRGBStruct; 3] = "Gouraud" @ 56,
    }
}
w3d_struct! {
    /// `struct W3dMeshHeaderStruct` (obsolete pre-3.0 mesh header).
    W3dMeshHeaderStruct = "W3dMeshHeaderStruct", 372, {
        version: u32 = "Version" @ 0,
        mesh_name: Name = "MeshName" @ 4,
        attributes: u32 = "Attributes" @ 20,
        num_tris: u32 = "NumTris" @ 24,
        num_quads: u32 = "NumQuads" @ 28,
        num_sr_tris: u32 = "NumSrTris" @ 32,
        num_pov_tris: u32 = "NumPovTris" @ 36,
        num_pov_quads: u32 = "NumPovQuads" @ 40,
        num_vertices: u32 = "NumVertices" @ 44,
        num_normals: u32 = "NumNormals" @ 48,
        num_sr_normals: u32 = "NumSrNormals" @ 52,
        num_tex_coords: u32 = "NumTexCoords" @ 56,
        num_materials: u32 = "NumMaterials" @ 60,
        num_vert_colors: u32 = "NumVertColors" @ 64,
        num_vert_influences: u32 = "NumVertInfluences" @ 68,
        num_damage_stages: u32 = "NumDamageStages" @ 72,
        future_counts: [u32; 5] = "FutureCounts" @ 76,
        lod_min: f32 = "LODMin" @ 96,
        lod_max: f32 = "LODMax" @ 100,
        min: W3dVectorStruct = "Min" @ 104,
        max: W3dVectorStruct = "Max" @ 116,
        sph_center: W3dVectorStruct = "SphCenter" @ 128,
        sph_radius: f32 = "SphRadius" @ 140,
        translation: W3dVectorStruct = "Translation" @ 144,
        rotation: [f32; 9] = "Rotation" @ 156,
        mass_center: W3dVectorStruct = "MassCenter" @ 192,
        inertia: [f32; 9] = "Inertia" @ 204,
        volume: f32 = "Volume" @ 240,
        hierarchy_tree_name: Name = "HierarchyTreeName" @ 244,
        hierarchy_model_name: Name = "HierarchyModelName" @ 260,
        future_use: [u32; 24] = "FutureUse" @ 276,
    }
}
w3d_struct! {
    /// `struct W3dMeshDamageStruct` (obsolete).
    W3dMeshDamageStruct = "W3dMeshDamageStruct", 32, {
        num_damage_materials: u32 = "NumDamageMaterials" @ 0,
        num_damage_verts: u32 = "NumDamageVerts" @ 4,
        num_damage_colors: u32 = "NumDamageColors" @ 8,
        damage_index: u32 = "DamageIndex" @ 12,
        future_use: [u32; 4] = "FutureUse" @ 16,
    }
}
w3d_struct! {
    /// `struct W3dMeshDamageVertexStruct` (obsolete).
    W3dMeshDamageVertexStruct = "W3dMeshDamageVertexStruct", 16, {
        vertex_index: u32 = "VertexIndex" @ 0,
        new_vertex: W3dVectorStruct = "NewVertex" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dMeshDamageColorStruct` (obsolete).
    W3dMeshDamageColorStruct = "W3dMeshDamageColorStruct", 8, {
        vertex_index: u32 = "VertexIndex" @ 0,
        new_color: W3dRGBStruct = "NewColor" @ 4,
    }
}
w3d_struct! {
    /// `struct W3dHModelAuxDataStruct` (obsolete).
    W3dHModelAuxDataStruct = "W3dHModelAuxDataStruct", 184, {
        attributes: u32 = "Attributes" @ 0,
        mesh_count: u32 = "MeshCount" @ 4,
        collision_count: u32 = "CollisionCount" @ 8,
        skin_count: u32 = "SkinCount" @ 12,
        shadow_count: u32 = "ShadowCount" @ 16,
        null_count: u32 = "NullCount" @ 20,
        future_counts: [u32; 6] = "FutureCounts" @ 24,
        lod_min: f32 = "LODMin" @ 48,
        lod_max: f32 = "LODMax" @ 52,
        future_use: [u32; 32] = "FutureUse" @ 56,
    }
}

/// Calls `$m!(Type)` for every struct above (used by the layout tests).
#[macro_export]
#[doc(hidden)]
macro_rules! for_each_w3d_struct {
    ($m:ident) => {
        $m!(W3dChunkHeader);
        $m!(W3dVectorStruct);
        $m!(W3dQuaternionStruct);
        $m!(W3dTexCoordStruct);
        $m!(W3dRGBStruct);
        $m!(W3dRGBAStruct);
        $m!(W3dMaterialInfoStruct);
        $m!(W3dVertexMaterialStruct);
        $m!(W3dShaderStruct);
        $m!(W3dPS2ShaderStruct);
        $m!(W3dTextureInfoStruct);
        $m!(W3dTriStruct);
        $m!(W3dMeshHeader3Struct);
        $m!(W3dVertInfStruct);
        $m!(W3dMeshDeform);
        $m!(W3dDeformSetInfo);
        $m!(W3dDeformKeyframeInfo);
        $m!(W3dDeformData);
        $m!(W3dMeshAABTreeHeader);
        $m!(W3dMeshAABTreeNode);
        $m!(W3dHierarchyStruct);
        $m!(W3dPivotStruct);
        $m!(W3dPivotFixupStruct);
        $m!(W3dAnimHeaderStruct);
        $m!(W3dCompressedAnimHeaderStruct);
        $m!(W3dAnimChannelStruct);
        $m!(W3dBitChannelStruct);
        $m!(W3dTimeCodedAnimChannelStruct);
        $m!(W3dTimeCodedBitChannelStruct);
        $m!(W3dAdaptiveDeltaAnimChannelStruct);
        $m!(W3dMorphAnimHeaderStruct);
        $m!(W3dMorphAnimKeyStruct);
        $m!(W3dHModelHeaderStruct);
        $m!(W3dHModelNodeStruct);
        $m!(W3dLODModelHeaderStruct);
        $m!(W3dLODStruct);
        $m!(W3dCollectionHeaderStruct);
        $m!(W3dPlaceholderStruct);
        $m!(W3dTransformNodeStruct);
        $m!(W3dLightStruct);
        $m!(W3dSpotLightStruct);
        $m!(W3dLightAttenuationStruct);
        $m!(W3dLightTransformStruct);
        $m!(W3dEmitterHeaderStruct);
        $m!(W3dEmitterUserInfoStruct);
        $m!(W3dEmitterInfoStruct);
        $m!(W3dEmitterExtraInfoStruct);
        $m!(W3dVolumeRandomizerStruct);
        $m!(W3dEmitterInfoStructV2);
        $m!(W3dEmitterPropertyStruct);
        $m!(W3dEmitterColorKeyframeStruct);
        $m!(W3dEmitterOpacityKeyframeStruct);
        $m!(W3dEmitterSizeKeyframeStruct);
        $m!(W3dEmitterRotationHeaderStruct);
        $m!(W3dEmitterRotationKeyframeStruct);
        $m!(W3dEmitterFrameHeaderStruct);
        $m!(W3dEmitterFrameKeyframeStruct);
        $m!(W3dEmitterBlurTimeHeaderStruct);
        $m!(W3dEmitterBlurTimeKeyframeStruct);
        $m!(W3dEmitterLinePropertiesStruct);
        $m!(W3dAggregateHeaderStruct);
        $m!(W3dAggregateInfoStruct);
        $m!(W3dAggregateSubobjectStruct);
        $m!(W3dTextureReplacerHeaderStruct);
        $m!(W3dTextureReplacerStruct);
        $m!(W3dAggregateMiscInfo);
        $m!(W3dHLodHeaderStruct);
        $m!(W3dHLodArrayHeaderStruct);
        $m!(W3dHLodSubObjectStruct);
        $m!(W3dBoxStruct);
        $m!(W3dNullObjectStruct);
        $m!(W3dSoundRObjHeaderStruct);
        $m!(W3dShdMeshHeaderStruct);
        $m!(W3dShdSubMeshHeaderStruct);
        $m!(W3dMaterialStruct);
        $m!(W3dMaterial2Struct);
        $m!(W3dMaterial3Struct);
        $m!(W3dMap3Struct);
        $m!(W3dSurrenderTriStruct);
        $m!(W3dMeshHeaderStruct);
        $m!(W3dMeshDamageStruct);
        $m!(W3dMeshDamageVertexStruct);
        $m!(W3dMeshDamageColorStruct);
        $m!(W3dHModelAuxDataStruct);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every member lies inside the struct, in order, without overlap, and
    /// is aligned to its own element alignment.
    #[test]
    fn layouts_are_consistent() {
        macro_rules! check {
            ($t:ident) => {{
                let mut end = 0;
                for f in $t::FIELDS {
                    assert!(f.offset >= end, "{}.{} overlaps", $t::C_NAME, f.name);
                    end = f.offset + f.size;
                }
                assert!(end <= <$t as W3dPod>::SIZE, "{} overruns", $t::C_NAME);
                let bytes: Vec<u8> = (0..<$t as W3dPod>::SIZE)
                    .map(|i| (i * 7 + 3) as u8)
                    .collect();
                let v = <$t as W3dPod>::read_le(&bytes);
                let back = v.to_bytes();
                for f in $t::FIELDS {
                    assert_eq!(
                        &back[f.offset..f.offset + f.size],
                        &bytes[f.offset..f.offset + f.size],
                        "{}.{}",
                        $t::C_NAME,
                        f.name
                    );
                }
            }};
        }
        for_each_w3d_struct!(check);
    }
}
