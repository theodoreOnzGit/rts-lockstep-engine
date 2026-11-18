//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdcubemap.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdcubemap.h
//!
//! The definition (`ShdCubeMapDefClass`): its editable parameters (the `NAMED_*_PARAM` registrations are the level editor's and are
//! not translated), `Load`, and the requirements the renderer asks of it. Not translated: `Save`
//! (needs `ChunkSaveClass`), `Is_Valid_Config` (always true), `Create` (builds the device shader: see the `shd*`
//! shader files, which take the definition by reference).

use super::shdchunk::*;
use super::shddef::{ShdDefClass, ShdVersion};
use crate::formats::w3d::W3dResult;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `CHUNKID_VARIABLES` of this shader (shdcubemap.cpp).
pub const CHUNKID_VARIABLES: u32 = 0x1649_0470;
/// `VARID_TEXTURE_NAME`.
pub const VARID_TEXTURE_NAME: u32 = 0x00;
/// `VARID_AMBIENT_COLOR`.
pub const VARID_AMBIENT_COLOR: u32 = 0x01;
/// `VARID_DIFFUSE_COLOR`.
pub const VARID_DIFFUSE_COLOR: u32 = 0x02;
/// `VARID_SPECULAR_COLOR`.
pub const VARID_SPECULAR_COLOR: u32 = 0x03;

/// `class ShdCubeMapDefClass` (shdcubemap.h).
#[derive(Clone, Debug)]
pub struct ShdCubeMapDefClass {
    pub base: ShdDefClass,
    pub texture_name: String,
    pub ambient: Vector3,
    pub diffuse: Vector3,
    pub specular: Vector3,
}

impl ShdCubeMapDefClass {
    /// `ShdCubeMapDefClass::ShdCubeMapDefClass()`: the `SHDDEF_CLASSID_CUBEMAP` base and the default parameter values.
    pub fn new() -> Self {
        ShdCubeMapDefClass {
            base: ShdDefClass::new(SHDDEF_CLASSID_CUBEMAP),
            texture_name: String::new(),
            ambient: Vector3::new(1.0, 1.0, 1.0),
            diffuse: Vector3::new(1.0, 1.0, 1.0),
            specular: Vector3::new(0.0, 0.0, 0.0),
        }
    }

    /// The shader's name in the editor / `REGISTER_SHDDEF`.
    pub const NAME: &'static str = "Cube Map";

    /// `Requires_Normals`.
    pub fn requires_normals(&self) -> bool {
        true
    }

    /// `Requires_Tangent_Space_Vectors`.
    pub fn requires_tangent_space_vectors(&self) -> bool {
        false
    }

    /// `Requires_Sorting`.
    pub fn requires_sorting(&self) -> bool {
        false
    }

    /// `Static_Sort_Index`.
    pub fn static_sort_index(&self) -> i32 {
        0
    }

    /// `Is_Valid_Config`: always true.
    pub fn is_valid_config(&self) -> bool {
        true
    }

    /// `Get_TextureName`.
    pub fn get_texture_name(&self) -> &str {
        &self.texture_name
    }

    /// `Get_Ambient`.
    pub fn get_ambient(&self) -> &Vector3 {
        &self.ambient
    }

    /// `Get_Diffuse`.
    pub fn get_diffuse(&self) -> &Vector3 {
        &self.diffuse
    }

    /// `Get_Specular`.
    pub fn get_specular(&self) -> &Vector3 {
        &self.specular
    }

    /// `ShdCubeMapDefClass::Load` (shdcubemap.cpp): the base class's chunk, then one `CHUNKID_VARIABLES` chunk of micro-chunks;
    /// unknown chunks and micro-chunks are skipped. Always true.
    pub fn load(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        self.base.load(cload)?;
        while cload.open_chunk()? {
            if cload.cur_chunk_id()? == CHUNKID_VARIABLES {
                while cload.open_micro_chunk()? {
                    match cload.cur_micro_chunk_id() {
                        VARID_TEXTURE_NAME => self.texture_name = read_micro_wwstring(cload)?,
                        VARID_AMBIENT_COLOR => self.ambient = read_micro_vector3(cload)?,
                        VARID_DIFFUSE_COLOR => self.diffuse = read_micro_vector3(cload)?,
                        VARID_SPECULAR_COLOR => self.specular = read_micro_vector3(cload)?,
                        _ => {}
                    }
                    cload.close_micro_chunk()?;
                }
            }
            cload.close_chunk()?;
        }
        Ok(true)
    }
}

impl Default for ShdCubeMapDefClass {
    fn default() -> Self {
        Self::new()
    }
}
