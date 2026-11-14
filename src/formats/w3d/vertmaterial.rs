// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/vertmaterial.cpp, .h
//   (VertexMaterialClass(), Parse_W3dVertexMaterialStruct, the mapper choice
//   of Parse_Mapping_Args, the Get_/Set_ functions for the colours, opacity,
//   shininess, colour sources, lighting, UV sources and flags)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3d_util.cpp
//   (W3dUtilityClass::Convert_Color(const W3dRGBStruct &, Vector3 *))
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, by Greg Hjelstrom, Jani Penttinen and others)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `VertexMaterialClass`: the material a W3D pass lights its vertices with
//! (the `D3DMATERIAL8` colours, the colour sources, lighting on or off, which
//! UV array each texture stage reads, and the texture-coordinate mappers).
//!
//! # As upstream reads it
//!
//! `VertexMaterialClass()` starts with ambient and diffuse white, opacity 1,
//! the rest of the `D3DMATERIAL8` zero, every colour source `D3DMCS_MATERIAL`,
//! lighting **off**, and stage `i` reading UV array `i`. A loaded material
//! ([`VertexMaterialState::from_w3d`], `Load_W3D`'s end) then takes the
//! file's ambient, diffuse, specular and emissive (`Convert_Color`: each byte
//! over 255), shininess and opacity, the depth-cue and copy-specular flags,
//! and one mapper per stage from the attribute bits
//! (`W3DVERTMAT_STAGE0_MAPPING_MASK`, `STAGE1`). `MeshMatDescClass::
//! Post_Load_Process` (in [`super::render`]) later sets the colour sources,
//! lighting and UV sources.
//!
//! # Upstream quirks that are kept
//!
//! - Opacity is the diffuse colour's alpha (`Material->Diffuse.a`).
//! - `W3DVERTMAT_STAGEn_MAPPING_SILHOUETTE` (5) and any value past `GRID_WS_
//!   ENVIRONMENT` install no mapper (the `switch` has no case for them).
//! - A loaded material's lighting stays off until `Post_Load_Process`
//!   configures it.
//!
//! # What is left out
//!
//! The mappers' arguments (`Parse_Mapping_Args` reads them as an INI) and
//! the mappers themselves (`mapper.cpp`: UV animation, environment maps,
//! screen mapping): the kind is kept ([`MapperKind`]) with the raw
//! argument text on [`super::mesh::VertexMaterial`]. The client draws every
//! stage with plain UVs for now.

use super::mesh::VertexMaterial;
use super::w3d_file::{W3dRGBStruct, W3DVERTMAT_COPY_SPECULAR_TO_DIFFUSE, W3DVERTMAT_USE_DEPTH_CUE};

/// `D3DMCS_MATERIAL`.
pub const D3DMCS_MATERIAL: u32 = 0;
/// `D3DMCS_COLOR1`.
pub const D3DMCS_COLOR1: u32 = 1;
/// `D3DMCS_COLOR2`.
pub const D3DMCS_COLOR2: u32 = 2;
/// `MeshBuilderClass::MAX_STAGES`: the size of `UVSource[]` and `Mapper[]`.
pub const MAX_STAGES: usize = 8;
/// `VertexMaterialClass::FlagsType`.
pub const FLAG_DEPTH_CUE: u32 = 0;
pub const FLAG_DEPTH_CUE_TO_ALPHA: u32 = 1;
pub const FLAG_COPY_SPECULAR_TO_DIFFUSE: u32 = 2;

/// The mapper `Parse_Mapping_Args` installs on a stage (`TextureMapperClass`
/// subclasses), by the attribute value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapperKind {
    /// No mapper: the stage reads its UV array (`MAPPING_UV`, and the values
    /// the `switch` has no case for).
    None,
    Environment,
    ClassicEnvironment,
    Screen,
    LinearOffset,
    Scale,
    Grid,
    Rotate,
    SineLinearOffset,
    StepLinearOffset,
    ZigZagLinearOffset,
    WsClassicEnvironment,
    WsEnvironment,
    GridClassicEnvironment,
    GridEnvironment,
    Random,
    Edge,
    BumpEnv,
    GridWsClassicEnvironment,
    GridWsEnvironment,
}

impl MapperKind {
    /// The `case` of `Parse_Mapping_Args`' `switch` for a stage's mapping
    /// value (the attribute byte, `W3DVERTMAT_STAGE0_MAPPING_*` >> 16).
    pub fn from_mapping(v: u32) -> Self {
        use MapperKind::*;
        match v {
            1 => Environment,
            2 => ClassicEnvironment,
            3 => Screen,
            4 => LinearOffset,
            // 5: SILHOUETTE, no case
            6 => Scale,
            7 => Grid,
            8 => Rotate,
            9 => SineLinearOffset,
            0x0A => StepLinearOffset,
            0x0B => ZigZagLinearOffset,
            0x0C => WsClassicEnvironment,
            0x0D => WsEnvironment,
            0x0E => GridClassicEnvironment,
            0x0F => GridEnvironment,
            0x10 => Random,
            0x11 => Edge,
            0x12 => BumpEnv,
            0x13 => GridWsClassicEnvironment,
            0x14 => GridWsEnvironment,
            _ => None,
        }
    }
}

/// `VertexMaterialClass`: the state that decides what is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct VertexMaterialState {
    /// The name, if the file gave one.
    pub name: Option<Vec<u8>>,
    /// `Material->Ambient` (r, g, b).
    pub ambient: [f32; 3],
    /// `Material->Diffuse` (r, g, b).
    pub diffuse: [f32; 3],
    /// `Material->Specular` (r, g, b).
    pub specular: [f32; 3],
    /// `Material->Emissive` (r, g, b).
    pub emissive: [f32; 3],
    /// `Material->Power`.
    pub shininess: f32,
    /// `Material->Diffuse.a`.
    pub opacity: f32,
    /// `Flags` (bit `1 << FLAG_*`).
    pub flags: u32,
    /// `AmbientColorSource` (`D3DMCS_*`).
    pub ambient_color_source: u32,
    /// `EmissiveColorSource`.
    pub emissive_color_source: u32,
    /// `DiffuseColorSource`.
    pub diffuse_color_source: u32,
    /// `UseLighting`.
    pub use_lighting: bool,
    /// `UVSource[stage]`.
    pub uv_source: [i32; MAX_STAGES],
    /// `Mapper[0]`, `Mapper[1]` (the loader installs at most these two).
    pub mapper: [MapperKind; 2],
}

impl Default for VertexMaterialState {
    /// `VertexMaterialClass()`.
    fn default() -> Self {
        let mut uv_source = [0; MAX_STAGES];
        for (i, s) in uv_source.iter_mut().enumerate() {
            *s = i as i32;
        }
        VertexMaterialState {
            name: None,
            ambient: [1.0; 3],
            diffuse: [1.0; 3],
            specular: [0.0; 3],
            emissive: [0.0; 3],
            shininess: 0.0,
            opacity: 1.0,
            flags: 0,
            ambient_color_source: D3DMCS_MATERIAL,
            emissive_color_source: D3DMCS_MATERIAL,
            diffuse_color_source: D3DMCS_MATERIAL,
            use_lighting: false,
            uv_source,
            mapper: [MapperKind::None; 2],
        }
    }
}

/// `W3dUtilityClass::Convert_Color(rgb, &v)`: each byte over 255.
pub fn convert_color(c: &W3dRGBStruct) -> [f32; 3] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]
}

/// `Set_*_Color_Source(src)`: `COLOR1` and `COLOR2` map to theirs, anything
/// else to `D3DMCS_MATERIAL`.
pub fn color_source(src: u32) -> u32 {
    match src {
        1 => D3DMCS_COLOR1,
        2 => D3DMCS_COLOR2,
        _ => D3DMCS_MATERIAL,
    }
}

impl VertexMaterialState {
    /// The end of `VertexMaterialClass::Load_W3D` on a new material:
    /// `Set_Name`, `Parse_W3dVertexMaterialStruct` and the mapper choice of
    /// `Parse_Mapping_Args`.
    ///
    /// (#67) A legacy material ([`VertexMaterial::legacy`], made by
    /// `read_v3_materials`) is `VertexMaterialClass()` with the colours,
    /// shininess and opacity of its `D3DMATERIAL8`; `Init_From_Material3`
    /// sets no flag and no mapper.
    pub fn from_w3d(vm: &VertexMaterial) -> Self {
        let mut m = VertexMaterialState { name: vm.name.clone(), ..Default::default() };
        if let Some(d) = &vm.legacy {
            m.ambient = [d.ambient[0], d.ambient[1], d.ambient[2]];
            m.diffuse = [d.diffuse[0], d.diffuse[1], d.diffuse[2]];
            m.specular = [d.specular[0], d.specular[1], d.specular[2]];
            m.emissive = [d.emissive[0], d.emissive[1], d.emissive[2]];
            m.shininess = d.power;
            m.opacity = d.diffuse[3];
            return m;
        }
        let v = &vm.info;
        m.ambient = convert_color(&v.ambient);
        m.diffuse = convert_color(&v.diffuse);
        m.specular = convert_color(&v.specular);
        m.emissive = convert_color(&v.emissive);
        m.shininess = v.shininess;
        m.opacity = v.opacity;
        if v.attributes & W3DVERTMAT_USE_DEPTH_CUE != 0 {
            m.set_flag(FLAG_DEPTH_CUE, true);
        }
        if v.attributes & W3DVERTMAT_COPY_SPECULAR_TO_DIFFUSE != 0 {
            m.set_flag(FLAG_COPY_SPECULAR_TO_DIFFUSE, true);
        }
        m.mapper[0] = MapperKind::from_mapping((v.attributes & 0x00FF_0000) >> 16);
        m.mapper[1] = MapperKind::from_mapping((v.attributes & 0x0000_FF00) >> 8);
        m
    }

    /// `Set_Flag(flag, onoff)`.
    pub fn set_flag(&mut self, flag: u32, onoff: bool) {
        if onoff {
            self.flags |= 1 << flag;
        } else {
            self.flags &= !(1 << flag);
        }
    }

    /// `Get_Flag(flag)`.
    pub fn get_flag(&self, flag: u32) -> bool {
        (self.flags >> flag) & 1 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::w3d_file::{W3dPod, W3dVertexMaterialStruct};

    /// Predictions: a new material is white ambient and diffuse, opacity 1,
    /// lighting off, stage i on UV array i; a loaded one takes the file's
    /// colours over 255 (128 -> 0.50196), its opacity, the copy-specular flag
    /// (bit 2), an environment mapper on stage 0 and none for the silhouette
    /// value (5) on stage 1.
    #[test]
    fn a_loaded_material_takes_the_file_values() {
        let d = VertexMaterialState::default();
        assert_eq!((d.ambient, d.diffuse, d.opacity, d.use_lighting), ([1.0; 3], [1.0; 3], 1.0, false));
        assert_eq!(d.uv_source[3], 3);

        let mut b = [0u8; W3dVertexMaterialStruct::SIZE];
        b[0..4].copy_from_slice(&(0x0001_0500u32 | W3DVERTMAT_COPY_SPECULAR_TO_DIFFUSE).to_le_bytes());
        b[8] = 128; // Diffuse.R
        b[16 + 2] = 255; // Emissive.B
        b[24..28].copy_from_slice(&0.25f32.to_le_bytes()); // Opacity
        let vm = VertexMaterial { name: Some(b"m".to_vec()), info: W3dVertexMaterialStruct::from_bytes(&b), mapper_args: [None, None], legacy: None };
        let m = VertexMaterialState::from_w3d(&vm);
        assert_eq!(m.diffuse, [128.0 / 255.0, 0.0, 0.0]);
        assert_eq!(m.ambient, [0.0; 3]);
        assert_eq!(m.emissive, [0.0, 0.0, 1.0]);
        assert_eq!(m.opacity, 0.25);
        assert!(m.get_flag(FLAG_COPY_SPECULAR_TO_DIFFUSE));
        assert!(!m.get_flag(FLAG_DEPTH_CUE));
        assert_eq!(m.mapper, [MapperKind::Environment, MapperKind::None]);
    }
}
