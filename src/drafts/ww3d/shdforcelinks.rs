//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 1 case (the six shader definitions) (test: tests/differential_draft_ww3d7_shd.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdforcelinks.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdforcelinks.h
//!
//! `SHD_Force_Links` exists only to stop the MSVC linker from discarding the translation units whose static
//! `REGISTER_SHDDEF` objects register the shader definitions (`FORCE_LINK` from wwhack.h). Rust has no such
//! linker behaviour: the registration is an explicit list, which is what this file records. Which definition
//! registers under which id and name is in each shader's file (`shdsimple.rs`, ...).

/// The shader definitions `SHD_Force_Links` forces, as `(FORCE_LINK name, SHDDEF class id, REGISTER_SHDDEF name)`.
pub const FORCED_SHADER_DEFS: [(&str, u32, &str); 6] = [
    ("SimpleShader", super::shdchunk::SHDDEF_CLASSID_SIMPLE, "Simple"),
    ("GlossMaskShader", super::shdchunk::SHDDEF_CLASSID_GLOSSMASK, "Gloss Mask"),
    ("BumpSpecShader", super::shdchunk::SHDDEF_CLASSID_BUMPSPEC, "Bump Specular"),
    ("BumpDiffShader", super::shdchunk::SHDDEF_CLASSID_BUMPDIFF, "Bump Diffuse"),
    ("CubeMapShader", super::shdchunk::SHDDEF_CLASSID_CUBEMAP, "Cube Map"),
    ("LegacyW3DShader", super::shdchunk::SHDDEF_CLASSID_LEGACYW3D, "LegacyW3D"),
];
