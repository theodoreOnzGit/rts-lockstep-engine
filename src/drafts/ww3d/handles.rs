//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Not a translation of one upstream file: the handle types the drafts in this directory use where upstream holds a
//! ref-counted pointer (`TextureClass *`, `VertexMaterialClass *`, `MaterialPassClass *`, `RenderObjClass *`,
//! `CameraClass *`). The verified port has no such object classes (it is data-driven, see
//! `crate::formats::w3d::render`), so a draft that needs one stores an index into whatever owner the finished port
//! gives it. `None` is upstream's `NULL`. Ref counts (`Add_Ref`/`Release_Ref`) are the owner's business and are
//! not modelled; each place a draft would have adjusted one says so.

/// `TextureClass *`.
pub type TextureHandle = usize;
/// `VertexMaterialClass *`.
pub type VertexMaterialHandle = usize;
/// `MaterialPassClass *`.
pub type MaterialPassHandle = usize;
/// `RenderObjClass *`.
pub type RenderObjHandle = usize;
/// `CameraClass *`.
pub type CameraHandle = usize;
/// `LightEnvironmentClass *`.
pub type LightEnvironmentHandle = usize;
/// `TexProjectClass *`.
pub type TexProjectHandle = usize;
/// `OBBoxClass *`.
pub type ObBoxHandle = usize;
