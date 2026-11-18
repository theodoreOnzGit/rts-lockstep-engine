//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdlib.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/wwshade/shdlib.h
//!
//! `shdlib.cpp` is six forwarding functions onto the shader renderer singleton (`ShdRendererClass`, shdrenderer.rs)
//! and the asset manager's prototype-loader registry. The singletons are the owner's; the functions are written as
//! the sequence of calls each makes, so the owner's wiring is checkable against upstream.

/// What `SHD_Register_Loader` registers with `WW3DAssetManager::Register_Prototype_Loader`, in order:
/// `_ShdMeshLoader` then `_MeshLoader` (the legacy mesh loader of `mesh.cpp`).
pub const SHD_REGISTER_LOADER_ORDER: [&str; 2] = ["_ShdMeshLoader", "_MeshLoader"];

/// The calls each `SHD_*` function makes: `ShdRendererClass::Peek_Instance()->Init()` etc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShdLibCall {
    /// `SHD_Init`: `ShdRendererClass::Peek_Instance()->Init()`.
    RendererInit,
    /// `SHD_Shutdown`: `ShdRendererClass::Peek_Instance()->Shutdown()`.
    RendererShutdown,
    /// `SHD_Init_Shaders`: `ShdRendererClass::Init_Shaders()` (static).
    InitShaders,
    /// `SHD_Shutdown_Shaders`: `ShdRendererClass::Shutdown_Shaders()` (static).
    ShutdownShaders,
    /// `SHD_Flush`: `ShdRendererClass::Peek_Instance()->Flush()`.
    RendererFlush,
}
