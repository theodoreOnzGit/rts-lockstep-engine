//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 cases (test: tests/differential_draft_ww3d7_shd.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdloader.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdloader.h
//!
//! The two prototype loaders the asset manager registers for shader meshes: `ShdMeshLoaderClass` loads a
//! `W3D_CHUNK_SHDMESH` with `ShdMeshClass::Load_W3D`, and `ShdMeshLegacyLoaderClass` loads an ordinary mesh with
//! `MeshClass::Load_W3D` then converts it (`ShdMeshClass::Init_From_Legacy_Mesh`). The mesh classes are owned
//! elsewhere (shdmesh.cpp is not drafted in this pass, `MeshClass` is not in the verified port), so the loaders are
//! generic over the load/convert steps.

/// `ShdMeshLoaderClass::Load_W3D` (shdloader.cpp:36): `None` when the mesh fails to load (upstream asserts the single
/// reference, releases it and returns NULL); otherwise the prototype is made from the mesh (`PrimitivePrototypeClass`).
pub fn shd_mesh_load_w3d<M, E, F: FnOnce() -> Result<M, E>>(load_mesh: F) -> Option<M> {
    match load_mesh() {
        Ok(mesh) => Some(mesh),
        Err(_) => None,
    }
}

/// `ShdMeshLegacyLoaderClass::Load_W3D` (shdloader.cpp:62): load the legacy mesh, convert it
/// (`Init_From_Legacy_Mesh`), and build the prototype from the converted mesh; `None` if the legacy load fails.
pub fn shd_mesh_legacy_load_w3d<L, S, E, F, C>(load_legacy: F, convert: C) -> Option<S>
where
    F: FnOnce() -> Result<L, E>,
    C: FnOnce(&L) -> S,
{
    match load_legacy() {
        Ok(mesh) => Some(convert(&mesh)),
        Err(_) => None,
    }
}
