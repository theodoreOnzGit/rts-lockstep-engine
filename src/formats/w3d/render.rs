// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hlod.cpp
//   (HLodClass::HLodClass(const HLodDefClass &): the sub-objects of each LOD
//   and the aggregates, bound to their bones)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/animobj.cpp
//   (Animatable3DObjClass: Compute_Current_Frame, the hierarchy update a
//   render does, the bone transforms its sub-objects take)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/mesh.cpp
//   (MeshClass::Init: Set_Hidden from W3D_MESH_FLAG_HIDDEN;
//   Get_Deformed_Vertices)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/assetmgr.cpp
//   (WW3DAssetManager::Get_Texture: the lower-cased name)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/textureloader.cpp
//   (TextureLoadTaskClass::Begin_Load: the compressed load first, then the
//   uncompressed; Begin_Uncompressed_Load's Targa orientation)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/ddsfile.cpp
//   (DDSFileClass::DDSFileClass: the name's last three characters made
//   "dds")
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, by Greg Hjelstrom, Jani Penttinen and others)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! A W3D render object as the renderer draws it, without a device:
//! [`RenderModel`] (an HLOD's meshes bound to the bones of its hierarchy, or
//! one mesh), posed by an animation, with its skins deformed; and the
//! texture images its meshes name ([`load_texture_image`]).
//!
//! # As upstream reads it
//!
//! - **`HLodClass(def)`**: the hierarchy is `def.HierarchyTreeName`'s; each
//!   LOD's sub-objects, then the aggregates, are created by name
//!   (`Create_Render_Obj`) and bound to their bone index. A sub-object's
//!   transform is its bone's posed transform (`HTree->Get_Transform(bone)`);
//!   a skin's vertices are deformed by the whole hierarchy instead and drawn
//!   in the model's space.
//! - **Meshes** are hidden when their header says `W3D_MESH_FLAG_HIDDEN`
//!   (`MeshClass::Init`).
//! - **Animation frames** ([`compute_current_frame`]): the frame advances by
//!   `frame rate * multiplier * direction * elapsed ms * 0.001`, then wraps by
//!   the mode (`ANIM_MODE_LOOP` subtracts `NumFrames - 1` once and resets to
//!   0 if still past it; `ONCE` stops at `NumFrames - 1`; the backward modes
//!   mirror them).
//! - **Textures**: `Get_Texture` lower-cases the name; the load tries the
//!   DDS first (the name with its last three characters replaced by `dds`),
//!   then the Targa by the name itself, each found through the game's file
//!   factory (`Data\<language>\Art\Textures\`, `Art\Textures\`, ...). A
//!   Targa's Y origin bit is flipped before loading ("DX8 uses image upside
//!   down compared to TGA"), so the rows come out top first.
//!
//! # Upstream quirks that are kept
//!
//! - The DDS name replaces the last three characters whatever they are
//!   (`foo.bmp` becomes `foo.dds`, `ab.c` becomes `a.dds`... of length 4
//!   kept).
//! - The Targa's Y-origin flip happens after `Open`, and `Load`'s own `Open`
//!   returns early on the open file, so the flipped header is what `Load`
//!   orients by.
//! - `YFlip` swaps each row pair in 16-byte blocks (`_swapBytes`) and leaves
//!   the last `row bytes & 15` in place: a row narrower than 16 bytes is not
//!   flipped at all, and a 24-bit row's tail stays (the Targa translation
//!   keeps this; real textures have 16-byte multiples).
//!
//! # Where this differs from upstream
//!
//! - **The LOD** is the highest (`LodCount - 1`); upstream picks by screen
//!   size and cost. Sub-objects that are not meshes (boxes, nulls, nested
//!   HLODs, emitters) are listed in [`RenderModel::not_drawn`], not drawn.
//! - A hierarchy that cannot be found poses every bone at the root
//!   (upstream's `Animatable3DObjClass` makes a one-pivot default tree, on
//!   which only bone 0 is defined).
//! - **Texture images** are decoded to RGBA8 at full size, top mip only:
//!   DXT1, DXT5, A8R8G8B8, X8R8G8B8 and R8G8B8 DDS files, and 24- and 32-bit
//!   Targas. Upstream converts to the device's formats, makes mip maps,
//!   applies texture reduction, and handles the paletted and 16-bit kinds;
//!   those give `None` here ([`TextureLoadError`] says why).
//! - A DDS name shorter than 3 characters indexes before the name: undefined
//!   ([`W3dUndefined::IndexOutOfRange`]); so does one of `_MAX_PATH`
//!   characters or more (`strncpy` leaves it unterminated).

use super::assetmgr::{AssetManager, HAnim, Prototype, W3dFileFactory, MAX_PATH};
use super::htree::HTree;
use super::meshmdl::MeshModel;
use super::w3d_file::W3D_MESH_FLAG_HIDDEN;
use super::{W3dResult, W3dUndefined};
use crate::common::file_system::{FileSystem, LocalDevice};
use crate::formats::texture::dds::DdsFile;
use crate::formats::texture::format::Ww3dFormat;
use crate::formats::texture::targa::{Targa, TGAF_IMAGE, TGAIDF_YORIGIN};
use crate::ww::matrix3d::Matrix3D;

/// One drawn sub-object of a [`RenderModel`].
#[derive(Clone, Debug, PartialEq)]
pub struct SubObject {
    /// The name it was created by (`CONTAINER.MESH`).
    pub name: Vec<u8>,
    /// The bone it is bound to.
    pub bone_index: i32,
    /// `Is_Hidden()` (`W3D_MESH_FLAG_HIDDEN`).
    pub hidden: bool,
    pub model: MeshModel,
    /// A `RenderObjClass::Material_Override` set as the sub-object's user data (`Set_User_Data`): its
    /// `customUVOffset`, which the DX8 renderer puts in the stage-0 `LinearOffsetTextureMapperClass`
    /// for the draw (`Set_Current_UV_Offset`, with no time passing) and takes back after
    /// (`dx8renderer.cpp`). `None` without one. Set by a draw module (`W3DTankDraw`'s treads, GitHub
    /// #41) through `RenderObj::pose_render_model`.
    pub uv_offset: Option<[f32; 2]>,
}

impl SubObject {
    /// A skin: deformed by the hierarchy, drawn in the model's space.
    pub fn is_skin(&self) -> bool {
        self.model.get_flag(super::mesh::mesh_flags::SKIN)
    }
}

/// A render object: an HLOD's meshes (or one mesh) and the hierarchy they
/// hang on.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderModel {
    pub name: Vec<u8>,
    /// Whether it is an HLOD (`Class_ID() == CLASSID_HLOD`), not a lone mesh.
    pub is_hlod: bool,
    /// The hierarchy (posed by [`RenderModel::update`]); `None` for a lone
    /// mesh, or when the HLOD's could not be found.
    pub htree: Option<HTree>,
    pub sub_objects: Vec<SubObject>,
    /// Sub-objects that are not drawn: name, and why.
    pub not_drawn: Vec<(Vec<u8>, String)>,
}

impl RenderModel {
    /// The render object `Create_Render_Obj(name)` makes, as drawn: an HLOD
    /// (its top LOD and aggregates) or a mesh. `None` when there is no such
    /// prototype, or it is another kind.
    pub fn create<D: LocalDevice>(am: &mut AssetManager, fs: &FileSystem<D>, name: &[u8]) -> W3dResult<Option<RenderModel>> {
        let proto = match am.create_render_obj(fs, name)? {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        match proto {
            Prototype::Mesh(mesh) => {
                let model = MeshModel::from_mesh(&mesh)?;
                let hidden = model.w3d_attributes & W3D_MESH_FLAG_HIDDEN != 0;
                Ok(Some(RenderModel {
                    name: name.to_vec(),
                    is_hlod: false,
                    htree: None,
                    sub_objects: vec![SubObject { name: name.to_vec(), bone_index: 0, hidden, model, uv_offset: None }],
                    not_drawn: Vec::new(),
                }))
            }
            Prototype::HLod(def) => {
                let htree = am.get_htree(fs, &def.hierarchy_tree_name)?.cloned();
                let mut out = RenderModel { name: name.to_vec(), is_hlod: true, htree, sub_objects: Vec::new(), not_drawn: Vec::new() };
                let mut subs = Vec::new();
                if let Some(top) = def.lods.last() {
                    subs.extend(top.sub_objects.iter().cloned());
                }
                subs.extend(def.aggregates.sub_objects.iter().cloned());
                for s in subs {
                    let Some(sub_name) = s.name else { continue };
                    match am.create_render_obj(fs, &sub_name)?.cloned() {
                        Some(Prototype::Mesh(mesh)) => {
                            let model = MeshModel::from_mesh(&mesh)?;
                            let hidden = model.w3d_attributes & W3D_MESH_FLAG_HIDDEN != 0;
                            out.sub_objects.push(SubObject { name: sub_name, bone_index: s.bone_index, hidden, model, uv_offset: None });
                        }
                        Some(other) => {
                            let why = format!("a {} prototype, not a mesh", prototype_kind(&other));
                            out.not_drawn.push((sub_name, why));
                        }
                        None => out.not_drawn.push((sub_name, "no such render object".into())),
                    }
                }
                Ok(Some(out))
            }
            _ => Ok(None),
        }
    }

    /// The hierarchy update a render does: the base pose, or the animation's
    /// pose at `frame` (`Anim_Update(root, motion, frame)`), under `root`.
    pub fn update<P: super::hmorphanim::PoseLookup>(&mut self, root: &Matrix3D, anim: Option<(&HAnim, &P, f32)>) -> W3dResult<()> {
        let Some(tree) = &mut self.htree else { return Ok(()) };
        match anim {
            Some((motion, poses, frame)) => tree.anim_update(root, motion, poses, frame),
            None => tree.base_update(root),
        }
    }

    /// Bone `bone`'s posed transform (`HTree->Get_Transform(bone)`); the
    /// identity without a hierarchy.
    pub fn bone_transform(&self, bone: i32) -> W3dResult<Matrix3D> {
        match &self.htree {
            Some(t) => t.get_transform(bone),
            None => Ok(Matrix3D::IDENTITY),
        }
    }

    /// The transform sub-object `i` is drawn with: its bone's, or the
    /// identity for a skin (whose vertices [`Self::deformed`] moves).
    pub fn sub_object_transform(&self, i: usize) -> W3dResult<Matrix3D> {
        let s = self.sub_objects.get(i).ok_or(W3dUndefined::IndexOutOfRange)?;
        if s.is_skin() {
            Ok(Matrix3D::IDENTITY)
        } else {
            self.bone_transform(s.bone_index)
        }
    }

    /// `Get_Deformed_Vertices(dst_vert, dst_norm)` of skin `i` (vertices and
    /// normals in the model's space); its own vertices for a rigid mesh or
    /// without a hierarchy.
    pub fn deformed(&self, i: usize) -> W3dResult<super::meshmdl::Deformed> {
        let s = self.sub_objects.get(i).ok_or(W3dUndefined::IndexOutOfRange)?;
        match (&self.htree, s.is_skin()) {
            (Some(t), true) => s.model.get_deformed_vertices(t),
            _ => Ok((s.model.vertices.clone(), s.model.normals.clone())),
        }
    }
}

fn prototype_kind(p: &Prototype) -> &'static str {
    match p {
        Prototype::Mesh(_) => "mesh",
        Prototype::HLod(_) => "HLOD",
        Prototype::Collection(_) => "collection",
        Prototype::Box(_) => "box",
        Prototype::Aggregate(_) => "aggregate",
        Prototype::Null(_) => "null",
        Prototype::Dazzle(_) => "dazzle",
        Prototype::Sphere(_) => "sphere",
        Prototype::Ring(_) => "ring",
        Prototype::HModel(_) => "hierarchical model",
        Prototype::DistLod(_) => "distance LOD",
        Prototype::ParticleEmitter(_) => "particle emitter",
    }
}

/// `HAnimClass::Get_Frame_Rate()`.
pub fn get_frame_rate(a: &HAnim) -> f32 {
    match a {
        HAnim::Raw(a) => a.frame_rate,
        HAnim::Compressed(a) => a.frame_rate,
        HAnim::Morph(a) => a.frame_rate,
    }
}

/// `RenderObjClass::AnimMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimMode {
    Manual,
    Loop,
    Once,
    LoopPingpong,
    LoopBackwards,
    OnceBackwards,
}

/// `Animatable3DObjClass::Compute_Current_Frame` for `SINGLE_ANIM`: the new
/// frame and direction after `sync_time_diff_ms` milliseconds.
pub fn compute_current_frame(
    frame: f32,
    num_frames: i32,
    frame_rate: f32,
    frame_rate_multiplier: f32,
    direction: f32,
    sync_time_diff_ms: f32,
    mode: AnimMode,
) -> (f32, f32) {
    let mut frame = frame;
    let mut new_direction = direction;
    if mode == AnimMode::Manual {
        return (frame, new_direction);
    }
    let delta = frame_rate * frame_rate_multiplier * direction * sync_time_diff_ms * 0.001;
    frame += delta;
    let last = (num_frames - 1) as f32;
    match mode {
        AnimMode::Once => {
            if frame >= last {
                frame = last;
            }
        }
        AnimMode::Loop => {
            if frame >= last {
                frame -= last;
            }
            // If it is still too far out, reset
            if frame >= last {
                frame = 0.0;
            }
        }
        AnimMode::OnceBackwards => {
            if frame < 0.0 {
                frame = 0.0;
            }
        }
        AnimMode::LoopBackwards => {
            if frame < 0.0 {
                frame += last;
            }
            if frame < 0.0 {
                frame = last;
            }
        }
        AnimMode::LoopPingpong => {
            if direction >= 1.0 {
                if frame >= last {
                    frame = last * 2.0 - frame;
                    if frame >= last {
                        frame = last;
                    }
                    new_direction = -direction; // direction * -1.0f
                }
            } else if frame < 0.0 {
                frame = -frame;
                if frame >= last {
                    frame = 0.0;
                }
                new_direction = -direction; // direction * -1.0f
            }
        }
        AnimMode::Manual => {}
    }
    (frame, new_direction)
}

/// A decoded texture: RGBA8, rows top first, at its top level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// The file it came from.
    pub path: Vec<u8>,
}

/// Why a texture gave no image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextureLoadError {
    /// Neither the DDS nor the Targa was found.
    Missing,
    /// Found, but in a format this decoder does not draw (named).
    Unsupported(String),
    /// Found, but the loader failed on it.
    Unreadable(String),
}

/// The names `Get_Texture(name)` loads by: the lower-cased name (the Targa,
/// and the texture's own name) and the DDS name made from it.
pub fn texture_file_names(name: &[u8]) -> W3dResult<(Vec<u8>, Vec<u8>)> {
    let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
    // _strlwr
    let lower: Vec<u8> = name.iter().map(|c| c.to_ascii_lowercase()).collect();
    // DDSFileClass: strncpy(Name, name, sizeof(Name)); Name[len-3..] = "dds"
    if lower.len() >= MAX_PATH {
        return Err(W3dUndefined::StringOverflow.into());
    }
    if lower.len() < 3 {
        return Err(W3dUndefined::IndexOutOfRange.into());
    }
    let mut dds = lower.clone();
    let n = dds.len();
    dds[n - 3..].copy_from_slice(b"dds");
    Ok((lower, dds))
}

/// `TextureLoadTaskClass::Begin_Load`'s choice of file, decoded: the DDS if
/// it is found and loads, else the Targa.
pub fn load_texture_image<D: LocalDevice>(
    factory: &W3dFileFactory,
    fs: &FileSystem<D>,
    name: &[u8],
) -> W3dResult<Result<TextureImage, TextureLoadError>> {
    let (tga_name, dds_name) = texture_file_names(name)?;
    let mut first_error = None;
    if let Some(path) = factory.resolve(fs, &dds_name)? {
        if let Some(data) = fs.open_file(&path) {
            match decode_dds(&data) {
                Ok(img) => return Ok(Ok(TextureImage { path, ..img })),
                Err(e) => first_error = Some(e),
            }
        }
    }
    if let Some(path) = factory.resolve(fs, &tga_name)? {
        if let Some(data) = fs.open_file(&path) {
            return Ok(decode_targa(&data).map(|img| TextureImage { path, ..img }));
        }
    }
    Ok(Err(first_error.unwrap_or(TextureLoadError::Missing)))
}

/// A DDS file's top level as RGBA8.
pub fn decode_dds(data: &[u8]) -> Result<TextureImage, TextureLoadError> {
    let un = |e: crate::formats::texture::dds::DdsUndefined| TextureLoadError::Unreadable(format!("{e:?}"));
    let mut d = DdsFile::new(data, 0).map_err(un)?;
    if !d.is_available() {
        return Err(TextureLoadError::Unreadable("no DDS header".into()));
    }
    if !d.load().map_err(un)? {
        return Err(TextureLoadError::Unreadable("no payload".into()));
    }
    let (w, h) = (d.get_width(0).map_err(un)?, d.get_height(0).map_err(un)?);
    let format = d.get_format();
    let px = w as usize * h as usize;
    let bgra = match format {
        Ww3dFormat::Dxt1 | Ww3dFormat::Dxt5 | Ww3dFormat::A8R8G8B8 | Ww3dFormat::X8R8G8B8 => {
            // Copy_Level_To_Surface to A8R8G8B8 (the DXT decode), or the
            // same-format copy
            let mut buf = vec![0u8; px * 4];
            let dest = if format == Ww3dFormat::X8R8G8B8 { format } else { Ww3dFormat::A8R8G8B8 };
            d.copy_level_to_surface(0, dest, w, h, &mut buf, w * 4, [0.0; 3]).map_err(un)?;
            if format == Ww3dFormat::X8R8G8B8 {
                for p in buf.chunks_exact_mut(4) {
                    p[3] = 0xff;
                }
            }
            buf
        }
        Ww3dFormat::R8G8B8 => {
            let mut buf = vec![0u8; px * 3];
            d.copy_level_to_surface(0, Ww3dFormat::R8G8B8, w, h, &mut buf, w * 3, [0.0; 3]).map_err(un)?;
            buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 0xff]).collect()
        }
        f => return Err(TextureLoadError::Unsupported(format!("DDS format {f:?}"))),
    };
    Ok(TextureImage { width: w, height: h, rgba: bgra_to_rgba(bgra), path: Vec::new() })
}

/// A Targa as `Begin_Uncompressed_Load` reads it (the Y origin flipped
/// before `Load`), as RGBA8.
pub fn decode_targa(data: &[u8]) -> Result<TextureImage, TextureLoadError> {
    let un = |e: crate::formats::texture::targa::TgaUndefined| TextureLoadError::Unreadable(format!("{e:?}"));
    let mut t = Targa::new();
    let err = t.open(data).map_err(un)?;
    if err != 0 {
        return Err(TextureLoadError::Unreadable(format!("Targa open error {err}")));
    }
    // DX8 uses image upside down compared to TGA
    t.header.image_descriptor ^= TGAIDF_YORIGIN;
    let depth = t.header.pixel_depth;
    let err = t.load(data, TGAF_IMAGE, false).map_err(un)?;
    if err != 0 {
        return Err(TextureLoadError::Unreadable(format!("Targa load error {err}")));
    }
    let (w, h) = (t.header.width as u16 as u32, t.header.height as u16 as u32);
    let img = t.image().ok_or_else(|| TextureLoadError::Unreadable("no image".into()))?;
    let px = w as usize * h as usize;
    let bgra: Vec<u8> = match depth {
        32 => img.get(..px * 4).ok_or_else(|| TextureLoadError::Unreadable("short image".into()))?.to_vec(),
        24 => img.get(..px * 3).ok_or_else(|| TextureLoadError::Unreadable("short image".into()))?.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 0xff]).collect(),
        d => return Err(TextureLoadError::Unsupported(format!("{d}-bit Targa"))),
    };
    Ok(TextureImage { width: w, height: h, rgba: bgra_to_rgba(bgra), path: Vec::new() })
}

/// B, G, R, A bytes (A8R8G8B8 in memory) to R, G, B, A.
fn bgra_to_rgba(mut v: Vec<u8>) -> Vec<u8> {
    for p in v.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::file_system::MemoryDevice;
    use crate::formats::w3d::synth::*;
    use crate::formats::w3d::w3d_file::*;

    fn fs_with(files: &[(&str, Vec<u8>)]) -> FileSystem<MemoryDevice> {
        let mut d = MemoryDevice::new();
        for (p, data) in files {
            d.insert(p.as_bytes(), data.clone());
        }
        FileSystem::new(d)
    }

    /// A two-bone model: a hierarchy ROOT/ARM (ARM at (0, 0, 5) under the
    /// root), a rigid box on the arm, a skin quad whose vertices 0 and 1 are
    /// on the root and 2 and 3 on the arm, and a hidden mesh; an animation
    /// of 3 frames moving the arm along x by 0, 1, 2.
    fn robot() -> Vec<u8> {
        let ident = [0.0, 0.0, 0.0, 1.0];
        let mut f = htree_w3d(b"ROBOT", &[
            PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident },
            PivotSpec { name: b"ARM".to_vec(), parent: 0, translation: [0.0, 0.0, 5.0], rotation: ident },
        ]);
        f.extend(MeshSpec::cuboid(b"ROBOT", b"BOX", [-1.0; 3], [1.0; 3]).to_w3d());
        let mut skin = MeshSpec::quad(b"ROBOT", b"SKIN");
        skin.attributes = W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN;
        skin.bones = Some(vec![0, 0, 1, 1]);
        f.extend(skin.to_w3d());
        let mut hidden = MeshSpec::quad(b"ROBOT", b"HIDE");
        hidden.attributes = W3D_MESH_FLAG_HIDDEN;
        f.extend(hidden.to_w3d());
        f.extend(hlod_w3d(b"ROBOT", b"ROBOT", &[(b"ROBOT.BOX", 1), (b"ROBOT.SKIN", 0), (b"ROBOT.HIDE", 0), (b"ROBOT.NONE", 0)]));
        f.extend(raw_anim_w3d(b"ROBOT", b"WAVE", 3, 30, &[ChannelSpec { pivot: 1, kind: ANIM_CHANNEL_X, vector_len: 1, data: vec![0.0, 1.0, 2.0] }]));
        f
    }

    /// Predictions, written before the first run:
    /// - `Create_Render_Obj("ROBOT")` loads `robot.w3d` on demand and gives
    ///   three meshes (BOX on bone 1, SKIN on bone 0, HIDE hidden) and one
    ///   name not drawn (ROBOT.NONE, no such object);
    /// - in the base pose the box is at the arm, (0, 0, 5); the skin's
    ///   vertex 2 (1, 1, 0) on the arm goes to (1, 1, 5), vertex 0 stays;
    /// - with the animation at frame 2 (exact: frames 2 and 3, ratio 0 under
    ///   the portable rounding) the arm is at (2, 0, 5) and so is the box;
    ///   the skin's vertex 2 is at (3, 1, 5).
    #[test]
    fn an_hlod_is_bound_to_its_bones_and_posed() {
        let fs = fs_with(&[("Art/W3D/robot.w3d", robot())]);
        let mut am = AssetManager::new_game(W3dFileFactory::default());
        let mut model = RenderModel::create(&mut am, &fs, b"ROBOT").unwrap().unwrap();
        let names: Vec<&[u8]> = model.sub_objects.iter().map(|s| s.name.as_slice()).collect();
        assert_eq!(names, vec![&b"ROBOT.BOX"[..], b"ROBOT.SKIN", b"ROBOT.HIDE"]);
        assert_eq!(model.sub_objects.iter().map(|s| s.hidden).collect::<Vec<_>>(), vec![false, false, true]);
        assert_eq!(model.not_drawn.len(), 1);
        assert!(model.sub_objects[1].is_skin());

        model.update::<AssetManager>(&Matrix3D::IDENTITY, None).unwrap();
        assert_eq!(model.sub_object_transform(0).unwrap().get_translation(), crate::ww::vector3::Vector3::new(0.0, 0.0, 5.0));
        let (v, _) = model.deformed(1).unwrap();
        assert_eq!((v[0], v[2]), ([0.0, 0.0, 0.0], [1.0, 1.0, 5.0]));

        let anim = am.get_hanim(&fs, b"ROBOT.WAVE").unwrap().unwrap().clone();
        assert_eq!(get_frame_rate(&anim), 30.0);
        model.update(&Matrix3D::IDENTITY, Some((&anim, &am, 2.0))).unwrap();
        assert_eq!(model.bone_transform(1).unwrap().get_translation(), crate::ww::vector3::Vector3::new(2.0, 0.0, 5.0));
        let (v, _) = model.deformed(1).unwrap();
        assert_eq!(v[2], [3.0, 1.0, 5.0]);
        assert_eq!(model.sub_object_transform(1).unwrap(), Matrix3D::IDENTITY, "a skin is drawn in model space");
    }

    /// Predictions: a 30 fps animation of 10 frames, looping, advances 3
    /// frames in 100 ms; from frame 8, 100 ms reaches 11, past the last
    /// frame (9) by 2: frame 2; 1000 ms (30 frames) is still past after one
    /// wrap: frame 0. `ONCE` stops at 9.
    #[test]
    fn frames_wrap_as_upstream() {
        let f = |fr, ms, mode| compute_current_frame(fr, 10, 30.0, 1.0, 1.0, ms, mode).0;
        assert!((f(0.0, 100.0, AnimMode::Loop) - 3.0).abs() < 1e-5);
        assert!((f(8.0, 100.0, AnimMode::Loop) - 2.0).abs() < 1e-5);
        assert_eq!(f(0.0, 1000.0, AnimMode::Loop), 0.0);
        assert_eq!(f(8.0, 100.0, AnimMode::Once), 9.0);
    }

    /// Predictions: `Quad.TGA` looks for `quad.dds` then `quad.tga`; with
    /// only a 32-bit bottom-up Targa present the image comes from the Targa
    /// under `Art/Textures/`, with its bottom row last (rows top first);
    /// names under 3 characters are undefined.
    ///
    /// **Prediction missed (2026-11-05), first run with a 2x2 image:** the
    /// rows came out in file order. `YFlip`'s `_swapBytes` swaps rows in
    /// 16-byte blocks and leaves the tail (`count & 15` bytes) in place, so
    /// an 8-byte row does not move at all (the Targa translation's
    /// `y_flip_leaves_the_row_tail`). The image is now 4x2 (16-byte rows);
    /// the quirk is in the module docs.
    #[test]
    fn textures_are_found_and_oriented_as_upstream() {
        assert_eq!(texture_file_names(b"Quad.TGA").unwrap(), (b"quad.tga".to_vec(), b"quad.dds".to_vec()));
        assert_eq!(texture_file_names(b"ab"), Err(W3dUndefined::IndexOutOfRange.into()));
        // 4x2, 32 bits, bottom-left origin: rows bottom first in the file
        let mut tga = vec![0u8, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 2, 0, 32, 8];
        // bottom row: blue, green, blue, green (B, G, R, A)
        tga.extend([255, 0, 0, 255, 0, 255, 0, 255].repeat(2));
        // top row: red, white, red, white
        tga.extend([0, 0, 255, 255, 255, 255, 255, 255].repeat(2));
        let fs = fs_with(&[("Art/Textures/quad.tga", tga)]);
        let img = load_texture_image(&W3dFileFactory::default(), &fs, b"Quad.TGA").unwrap().unwrap();
        assert_eq!(img.path, b"Art/Textures/quad.tga");
        assert_eq!((img.width, img.height), (4, 2));
        assert_eq!(&img.rgba[..16], &[255, 0, 0, 255, 255, 255, 255, 255].repeat(2)[..], "top row first: red, white");
        assert_eq!(&img.rgba[16..], &[0, 0, 255, 255, 0, 255, 0, 255].repeat(2)[..], "then blue, green");
        let none = load_texture_image(&W3dFileFactory::default(), &fs, b"other.tga").unwrap();
        assert_eq!(none, Err(TextureLoadError::Missing));
    }
}
