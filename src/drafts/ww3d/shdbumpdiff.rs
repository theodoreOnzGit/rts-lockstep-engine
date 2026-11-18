//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdbumpdiff.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdbumpdiff.h
//!
//! The definition (`ShdBumpDiffDefClass`): its editable parameters (the `NAMED_*_PARAM` registrations are the level editor's and are
//! not translated), `Load`, and the requirements the renderer asks of it. Not translated: `Save`
//! (needs `ChunkSaveClass`), `Is_Valid_Config` (always true), `Create` (builds the device shader: see the `shd*`
//! shader files, which take the definition by reference).
//! `Init`/`Shutdown` pick the shader implementation by hardware ([`select_shader_version`]); the per-version
//! `Init`/`Shutdown` create and destroy D3D vertex/pixel shaders (device-only).

use super::shdchunk::*;
use super::shddef::{ShdDefClass, ShdVersion};
use crate::formats::w3d::W3dResult;
use crate::ww::chunkio::ChunkLoad;
use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;

/// `CHUNKID_VARIABLES` of this shader (shdbumpdiff.cpp).
pub const CHUNKID_VARIABLES: u32 = 0x1649_0450;
/// `VARID_TEXTURE_NAME`.
pub const VARID_TEXTURE_NAME: u32 = 0x00;
/// `VARID_BUMP_MAP_NAME`.
pub const VARID_BUMP_MAP_NAME: u32 = 0x01;
/// `VARID_AMBIENT_COLOR`.
pub const VARID_AMBIENT_COLOR: u32 = 0x02;
/// `VARID_DIFFUSE_COLOR`.
pub const VARID_DIFFUSE_COLOR: u32 = 0x03;
/// `VARID_DIFFUSE_BUMPINESS`.
pub const VARID_DIFFUSE_BUMPINESS: u32 = 0x04;

/// `ShdVersion ShdBumpDiffDefClass::Version` (a class static upstream; one per process).

/// `class ShdBumpDiffDefClass` (shdbumpdiff.h).
#[derive(Clone, Debug)]
pub struct ShdBumpDiffDefClass {
    pub base: ShdDefClass,
    pub texture_name: String,
    pub bump_map_name: String,
    pub ambient: Vector3,
    pub diffuse: Vector3,
    pub diffuse_bumpiness: Vector2,
}

impl ShdBumpDiffDefClass {
    /// `ShdBumpDiffDefClass::ShdBumpDiffDefClass()`: the `SHDDEF_CLASSID_BUMPDIFF` base and the default parameter values.
    pub fn new() -> Self {
        ShdBumpDiffDefClass {
            base: ShdDefClass::new(SHDDEF_CLASSID_BUMPDIFF),
            texture_name: String::new(),
            bump_map_name: String::new(),
            ambient: Vector3::new(1.0, 1.0, 1.0),
            diffuse: Vector3::new(1.0, 1.0, 1.0),
            diffuse_bumpiness: Vector2::new(1.0, 0.0),
        }
    }

    /// The shader's name in the editor / `REGISTER_SHDDEF`.
    pub const NAME: &'static str = "Bump Diffuse";

    /// `Requires_Normals`.
    pub fn requires_normals(&self) -> bool {
        true
    }

    /// `Requires_Tangent_Space_Vectors`.
    pub fn requires_tangent_space_vectors(&self) -> bool {
        true
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

    /// `Get_BumpMapName`.
    pub fn get_bump_map_name(&self) -> &str {
        &self.bump_map_name
    }

    /// `Get_Ambient`.
    pub fn get_ambient(&self) -> &Vector3 {
        &self.ambient
    }

    /// `Get_Diffuse`.
    pub fn get_diffuse(&self) -> &Vector3 {
        &self.diffuse
    }

    /// `Get_Diffuse_Bumpiness`.
    pub fn get_diffuse_bumpiness(&self) -> &Vector2 {
        &self.diffuse_bumpiness
    }

    /// `ShdBumpDiffDefClass::Load` (shdbumpdiff.cpp): the base class's chunk, then one `CHUNKID_VARIABLES` chunk of micro-chunks;
    /// unknown chunks and micro-chunks are skipped. Always true.
    pub fn load(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        self.base.load(cload)?;
        while cload.open_chunk()? {
            if cload.cur_chunk_id()? == CHUNKID_VARIABLES {
                while cload.open_micro_chunk()? {
                    match cload.cur_micro_chunk_id() {
                        VARID_TEXTURE_NAME => self.texture_name = read_micro_wwstring(cload)?,
                        VARID_BUMP_MAP_NAME => self.bump_map_name = read_micro_wwstring(cload)?,
                        VARID_AMBIENT_COLOR => self.ambient = read_micro_vector3(cload)?,
                        VARID_DIFFUSE_COLOR => self.diffuse = read_micro_vector3(cload)?,
                        VARID_DIFFUSE_BUMPINESS => self.diffuse_bumpiness = read_micro_vector2(cload)?,
                        _ => {}
                    }
                    cload.close_micro_chunk()?;
                }
            }
            cload.close_chunk()?;
        }
        Ok(true)
    }

    /// `ShdBumpDiffDefClass::Init` (the version choice): stores it in `version` and returns it; the chosen class's `Init` follows
    /// (device-only). Inputs are the caps: pixel shader version, DOT3 support.
    pub fn init(version: &mut ShdVersion, ps_major: u32, ps_minor: u32, support_dot3: bool) -> super::shddef::SHDVER {
        let v = select_shader_version(ps_major, ps_minor, support_dot3);
        version.set(v);
        v
    }
}

impl Default for ShdBumpDiffDefClass {
    fn default() -> Self {
        Self::new()
    }
}
