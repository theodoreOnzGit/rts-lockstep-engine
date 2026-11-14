// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hlod.cpp
//   (HLodClass::HLodClass(const HLodDefClass &): the sub-objects of each LOD
//   and the aggregates, bound to their bones; #65 batch c: Remove_Sub_Object,
//   Notify_Removed, Free's release of the sub-objects)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/scene.cpp
//   (#65 batch c: SimpleSceneClass::Register's RELEASE list, Customized_Render's
//   update loop order, Post_Render_Processing)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp
//   (#65 batch c: RTS3DScene::Render, which never calls Post_Render_Processing)
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
//!   HLODs, ~~emitters~~) are listed in [`RenderModel::not_drawn`], not drawn.
//!   **Corrected 2026-11-07 (#49):** particle emitters are kept
//!   ([`RenderModel::emitters`]), bound to their bones, and run by
//!   [`RenderModel::update_emitters`]; a bone's visibility
//!   (`Set_Animation_Hidden` from the hierarchy) does not stop them yet.
//! - **(#65 batch c) The scene's side of the emitters** is kept with the
//!   model: the order the scene's update list runs them in, their release
//!   requests ([`RenderModel::release_list`]), the release itself in a scene
//!   that processes it ([`RenderModel::post_render_processing`],
//!   [`SceneKind`]), and the buffers an emitter leaves drawing in the scene
//!   when it is released or its HLOD is removed ([`RenderModel::orphans`],
//!   [`RenderModel::remove_from_scene`]). The HLOD's bounding volumes and
//!   sub-object bits (`Update_Obj_Space_Bounding_Volumes`,
//!   `Update_Sub_Object_Bits`, which `Remove_Sub_Object` calls) are not
//!   kept: nothing here reads them (the client does not cull), and the base
//!   pose `Update_Obj_Space_Bounding_Volumes` leaves on the hierarchy is
//!   undone by its `Set_Hierarchy_Valid(false)` before the next read.
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
    /// The hierarchy (posed by [`RenderModel::update_render_obj`] or [`RenderModel::update`]); `None` for a lone
    /// mesh, or when the HLOD's could not be found.
    pub htree: Option<HTree>,
    pub sub_objects: Vec<SubObject>,
    /// Sub-objects that are not drawn: name, and why.
    pub not_drawn: Vec<(Vec<u8>, String)>,
    /// The particle emitters among the sub-objects (#49), on their bones.
    pub emitters: Vec<EmitterSub>,
    /// (#65 batch c) The buffers this HLOD's emitters left in the scene (released or removed with the HLOD), still
    /// drawing their particles, in sub-object order.
    pub orphans: Vec<OrphanBuffer>,
    /// (#65 batch c) The scene's `ReleaseList` as far as this model's objects are in it: the ids ([`EmitterSub::id`]) of
    /// the emitters and orphaned buffers registered for release, head first (`Add` puts an object at the head, once).
    pub release_list: Vec<u32>,
    /// (#65 batch c) How many times [`RenderModel::update_emitters`] has run (stamps [`EmitterSub::buffer_joined`]).
    pub updates: u32,
}

/// (#65 batch c) The scene a model is in, for what its objects' release requests do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneKind {
    /// `RTS3DScene`, the world: its `Render` replaces `SceneClass::Render` ("Made virtual so we can override -MW") and
    /// never calls `Post_Render_Processing`, so a release request is listed and never processed.
    World,
    /// A `SimpleSceneClass` drawn through `SceneClass::Render` (`RTS2DScene`, `RTS3DInterfaceScene`: the W3D cursors),
    /// which runs `Post_Render_Processing` after every render.
    Simple,
}

/// (#65 batch c) What keeps an orphaned buffer's emitter, if anything.
#[derive(Clone, Debug, PartialEq)]
pub enum Orphaned {
    /// The emitter is out of its HLOD and stopped (`Notify_Removed`) but alive: the scene's release list holds a
    /// reference to it. Its buffer keeps its `Emitter` pointer, so it never completes while the emitter lives.
    Held(super::part_emt::ParticleEmitter),
    /// The emitter is destroyed (`~ParticleEmitterClass`: `Buffer->Emitter_Is_Dead()`): the buffer alone, kept by the
    /// scene.
    Dead(super::part_buf::ParticleBuffer),
}

/// (#65 batch c) A particle buffer still in the scene after its emitter left its HLOD.
#[derive(Clone, Debug, PartialEq)]
pub struct OrphanBuffer {
    /// The emitter's [`EmitterSub::id`].
    pub id: u32,
    pub name: Vec<u8>,
    pub obj: Orphaned,
    /// Its [`EmitterSub::buffer_joined`].
    pub buffer_joined: u32,
    /// In the scene's release list: the emitter for [`Orphaned::Held`] (always), the buffer for [`Orphaned::Dead`]
    /// (`ParticleBufferClass::On_Frame_Update` registers a complete buffer).
    pub in_release_list: bool,
    /// What it drew at the last update.
    pub draw: Option<super::part_buf::ParticleDraw>,
    /// Why it stopped: an input upstream is undefined on.
    pub failed: Option<String>,
}

impl OrphanBuffer {
    /// The buffer.
    pub fn buffer(&self) -> &super::part_buf::ParticleBuffer {
        match &self.obj {
            Orphaned::Held(e) => &e.buffer,
            Orphaned::Dead(b) => b,
        }
    }

    /// `ParticleBufferClass::On_Frame_Update()`: with its (stopped) emitter, `Emitter->Emit()`, which emits nothing and
    /// takes the transform as the previous one; then whether it is complete (`IsEmitterDead` and no particles), in
    /// which case it registers for release. Returns whether this added it to the release list.
    fn on_frame_update(&mut self, clock: &super::part_buf::Ww3dClock, rng: &mut super::part_emt::ParticleRandom) -> bool {
        let complete = match &mut self.obj {
            Orphaned::Held(e) => match e.buffer_on_frame_update(clock, rng) {
                Ok(c) => c,
                Err(err) => {
                    self.failed = Some(format!("{err}"));
                    false
                }
            },
            Orphaned::Dead(b) => b.on_frame_update(),
        };
        if complete && !self.in_release_list && !crate::logic::modules::plant(10136) {
            self.in_release_list = true;
            return true;
        }
        false
    }

    /// `ParticleBufferClass::Render`: its emitter's position and stopped state while it has one.
    fn render(&mut self, clock: &super::part_buf::Ww3dClock) {
        let r = match &mut self.obj {
            Orphaned::Held(e) => {
                let at = (e.get_position(), e.is_stopped());
                e.buffer.render(clock, Some(at))
            }
            Orphaned::Dead(b) => b.render(clock, None),
        };
        match r {
            Ok(d) => self.draw = Some(d),
            Err(err) => {
                self.failed = Some(format!("{err}"));
                self.draw = None;
            }
        }
    }

    /// Whether it can never draw a particle again: no particles left and no emitter that could emit (a dead emitter's
    /// buffer complete, or a held emitter's stopped one empty). **New** (not upstream): the client's test for dropping it
    /// (see [`SceneKind::World`]: upstream keeps such a buffer in the world scene, drawing nothing, until the scene
    /// goes).
    pub fn is_drained(&self) -> bool {
        match &self.obj {
            Orphaned::Held(e) => e.is_stopped() && e.buffer.get_particle_count() == 0,
            Orphaned::Dead(b) => b.is_complete(),
        }
    }
}

/// A particle emitter bound to a bone of a [`RenderModel`] (#49).
#[derive(Clone, Debug, PartialEq)]
pub struct EmitterSub {
    /// The name it was created by.
    pub name: Vec<u8>,
    pub bone_index: i32,
    /// Its definition (the prototype).
    pub def: super::part_ldr::ParticleEmitterDef,
    /// The render object, made at the first [`RenderModel::update_emitters`]
    /// (`Create_From_Definition` needs the clock and the generators).
    pub emitter: Option<super::part_emt::ParticleEmitter>,
    /// What the buffer drew at the last update.
    pub draw: Option<super::part_buf::ParticleDraw>,
    /// Why it stopped: an input upstream is undefined on.
    pub failed: Option<String>,
    /// (#65 batch c) Which emitter it is: its place among the HLOD's emitters when made (the order the scene's lists
    /// put its buffer in; [`OrphanBuffer::id`] keeps it).
    pub id: u32,
    /// (#65 batch c) Registered for release and in the scene's release list.
    pub in_release_list: bool,
    /// (#65 batch c) The update ([`RenderModel::updates`]) in which its buffer joined the scene (its place in the
    /// scene's update list: a later one is nearer the head).
    pub buffer_joined: u32,
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
                    emitters: Vec::new(),
                    orphans: Vec::new(),
                    release_list: Vec::new(),
                    updates: 0,
                }))
            }
            Prototype::HLod(def) => {
                let htree = am.get_htree(fs, &def.hierarchy_tree_name)?.cloned();
                let mut out = RenderModel {
                    name: name.to_vec(),
                    is_hlod: true,
                    htree,
                    sub_objects: Vec::new(),
                    not_drawn: Vec::new(),
                    emitters: Vec::new(),
                    orphans: Vec::new(),
                    release_list: Vec::new(),
                    updates: 0,
                };
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
                        Some(Prototype::ParticleEmitter(def)) => {
                            let id = out.emitters.len() as u32;
                            out.emitters.push(EmitterSub {
                                name: sub_name,
                                bone_index: s.bone_index,
                                def,
                                emitter: None,
                                draw: None,
                                failed: None,
                                id,
                                in_release_list: false,
                                buffer_joined: 0,
                            });
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

    /// ~~The hierarchy update a render does~~ **Corrected 2026-11-13 (#180):** the hierarchy posed through the
    /// *virtual* `HTreeClass::Anim_Update(root, motion, frame)` ([`HTree::anim_update`], which interpolates a raw
    /// animation between frames), or the base pose, under `root`. A render does not do this upstream: a render object
    /// takes animobj.h's inline update, [`RenderModel::update_render_obj`], which the client's drawn pose uses. This one
    /// stays for [`crate::client::w3d_model_draw::W3dFiles::skeleton`] (the pristine bones, which also feed the
    /// logic's art bones, #175): those ask only integer frames (0 or `NumFrames - 1`), where the two paths agree bit
    /// for bit under the default `Float_To_Long` ([`FloatToLongMode::Win32Fistp`](crate::ww::wwmath::FloatToLongMode);
    /// checked by the test `integer_frames_agree_on_both_paths_in_win32`), and a change to the logic's function is
    /// the owner's decision (#180).
    pub fn update<P: super::hmorphanim::PoseLookup>(&mut self, root: &Matrix3D, anim: Option<(&HAnim, &P, f32)>) -> W3dResult<()> {
        let Some(tree) = &mut self.htree else { return Ok(()) };
        match anim {
            Some((motion, poses, frame)) => tree.anim_update(root, motion, poses, frame),
            None => tree.base_update(root),
        }
    }

    /// (#180) The hierarchy update a render object's render does (animobj.cpp's `Update_Sub_Object_Transforms`): with
    /// an animation, animobj.h's inline `Anim_Update(Transform, Motion, Frame)` ([`HTree::anim_update_animobj`], the
    /// code the live render object `RenderObj` runs: a raw animation through the raw fast path at one integer frame,
    /// `Float_To_Long(frame)`, the others through the virtual update); without one, `Base_Update(root)`. `root` is the
    /// render object's `Transform`. Nothing without a hierarchy. The client's drawn pose
    /// (`rts_client::drawables::WorldDrawables`).
    pub fn update_render_obj<P: super::hmorphanim::PoseLookup>(&mut self, root: &Matrix3D, anim: Option<(&HAnim, &P, f32)>) -> W3dResult<()> {
        if crate::logic::modules::plant(9831) {
            return self.update(root, anim);
        }
        let root = if crate::logic::modules::plant(9834) { Matrix3D::IDENTITY } else { *root };
        let Some(tree) = &mut self.htree else { return Ok(()) };
        match anim {
            Some((motion, poses, frame)) if !crate::logic::modules::plant(9832) => tree.anim_update_animobj(&root, motion, poses, frame),
            _ => tree.base_update(&root),
        }
    }

    /// The hierarchy update a render does in `MULTIPLE_ANIM`
    /// (`Animatable3DObjClass::Update_Sub_Object_Transforms`'s
    /// `Combo_Update(Transform, ModeCombo.AnimCombo)`, after
    /// `HLodClass::Set_Animation(HAnimComboClass *)`; #162): the combo's
    /// pose under `root`. Nothing without a hierarchy. `motion` resolves a
    /// combo entry's animation handle. ~~The embedded-sound triggering that
    /// follows upstream is not translated (no animation of the game's
    /// combos has embedded sounds: no game code makes a combo, see
    /// [`super::hanim_combo`]).~~ **Corrected 2026-11-14 (#181 item 3):**
    /// the embedded-sound triggering that follows upstream is translated
    /// on the render object (`client::w3d_model_draw::RenderObj::
    /// update_sub_object_transforms`, through [`super::animatedsoundmgr`]),
    /// not here: this pose is the drawn model's, and no game code makes a
    /// combo (see [`super::hanim_combo`]).
    pub fn update_combo<'w, P: super::hmorphanim::PoseLookup, M: Clone>(
        &mut self,
        root: &Matrix3D,
        combo: &super::hanim_combo::HAnimCombo<M>,
        poses: &P,
        motion: impl Fn(&M) -> &'w HAnim,
    ) -> W3dResult<()> {
        let Some(tree) = &mut self.htree else { return Ok(()) };
        tree.combo_update(root, combo, poses, motion)
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

    /// One frame of the particle emitters at the clock (#49): each emitter
    /// takes its bone's transform under `world` (`HLodClass` updating its
    /// sub-objects' transforms; the particles live in the world, so the
    /// object's own transform is part of it) and runs a frame. The first
    /// time, the emitter is made, added to the scene and shown, which starts
    /// it (`Notify_Added`, `Set_Hidden(false)`). An emitter upstream is
    /// undefined on stops, with the reason in [`EmitterSub::failed`].
    ///
    /// ~~Each emitter runs its frame in turn ([`super::part_emt::ParticleEmitter::frame`]: its `On_Frame_Update`, its
    /// buffer's, its buffer's `Render`).~~ **Corrected 2026-11-14 (#65, batch c):** the frame runs in the scene's order
    /// (scene.cpp `SimpleSceneClass::Customized_Render`, W3DScene.cpp `RTS3DScene::Customized_Render`): visibility is
    /// checked first, then every object of the update list has its `On_Frame_Update`, then the visible ones render. The
    /// update list adds at its head (multilist.cpp `Internal_Add`) and an object added during the loop is not reached,
    /// so the HLOD's emitters (added by `HLodClass::Notify_Added` in sub-object order) run last and in reverse, and the
    /// buffers (each added by its emitter's first active `On_Frame_Update`, after the emitters) run first, from the frame
    /// after: those added later first, those added in the same frame in sub-object order. A buffer added this frame is neither updated (no `Emit`) nor drawn
    /// (its visible bit is `DEFAULT_BITS`'s, clear, and the visibility check has run): its draw is `None`. The
    /// orphaned buffers ([`Self::orphans`]) run with the buffers, by id. A release request (`Scene->Register(this,
    /// RELEASE)`) puts the emitter or orphan at the head of [`Self::release_list`] unless it is listed already; the
    /// release is [`Self::post_render_processing`]'s. Every buffer in the scene is taken as visible (the client does not
    /// cull).
    pub fn update_emitters(&mut self, world: &Matrix3D, clock: &super::part_buf::Ww3dClock, rng: &mut super::part_emt::ParticleRandom) {
        use super::part_emt::ParticleEmitter;
        // the transforms; the emitters made and added the first time; which buffers are in the update list already
        let mut updating = vec![false; self.emitters.len()];
        for i in 0..self.emitters.len() {
            if self.emitters[i].failed.is_some() {
                continue;
            }
            let bone = self.bone_transform(self.emitters[i].bone_index).unwrap_or(Matrix3D::IDENTITY);
            let tm = Matrix3D::multiply(world, &bone);
            let e = &mut self.emitters[i];
            if e.emitter.is_none() {
                match ParticleEmitter::create_from_definition(&e.def, rng, clock) {
                    Ok(mut em) => {
                        em.set_transform(tm);
                        em.notify_added();
                        em.set_hidden(false);
                        e.emitter = Some(em);
                    }
                    Err(err) => {
                        e.failed = Some(format!("{err}"));
                        continue;
                    }
                }
            }
            let em = e.emitter.as_mut().expect("made above");
            em.set_transform(tm);
            updating[i] = em.buffer_in_scene;
        }
        self.updates = self.updates.wrapping_add(1);
        // the update list: first the buffers (the emitters' and the orphans'), each `On_Frame_Update`, in the list's
        // order: those that joined later first, those that joined in the same update in sub-object order (their
        // emitters ran in reverse, each putting its buffer at the head)
        let mut buffers: Vec<(std::cmp::Reverse<u32>, u32, bool, usize)> = Vec::new();
        for (i, e) in self.emitters.iter().enumerate() {
            if updating[i] && e.failed.is_none() {
                buffers.push((std::cmp::Reverse(e.buffer_joined), e.id, false, i));
            }
        }
        for (k, o) in self.orphans.iter().enumerate() {
            if o.failed.is_none() {
                buffers.push((std::cmp::Reverse(o.buffer_joined), o.id, true, k));
            }
        }
        buffers.sort();
        if crate::logic::modules::plant(10137) {
            buffers.reverse();
        }
        for &(_, id, orphan, k) in &buffers {
            if orphan {
                if self.orphans[k].on_frame_update(clock, rng) {
                    Self::register_release(&mut self.release_list, id);
                }
            } else {
                let e = &mut self.emitters[k];
                let em = e.emitter.as_mut().expect("updating");
                // (never complete while it has its emitter)
                if let Err(err) = em.buffer_on_frame_update(clock, rng) {
                    e.failed = Some(format!("{err}"));
                    e.draw = None;
                }
            }
        }
        // then the emitters, last added first: each `On_Frame_Update` (the first puts its buffer in the scene)
        let order: Vec<usize> = if crate::logic::modules::plant(10138) { (0..self.emitters.len()).collect() } else { (0..self.emitters.len()).rev().collect() };
        for i in order {
            let e = &mut self.emitters[i];
            if e.failed.is_some() {
                continue;
            }
            let em = e.emitter.as_mut().expect("made above");
            let joined = em.buffer_in_scene;
            let release = em.on_frame_update();
            if !joined && em.buffer_in_scene {
                e.buffer_joined = self.updates;
            }
            if release && (!e.in_release_list || crate::logic::modules::plant(10139)) {
                e.in_release_list = true;
                Self::register_release(&mut self.release_list, e.id);
            }
        }
        // the render: the buffers that were in the scene before the visibility check
        for (i, e) in self.emitters.iter_mut().enumerate() {
            if e.failed.is_some() {
                continue;
            }
            let em = e.emitter.as_mut().expect("made above");
            if !(updating[i] || crate::logic::modules::plant(10140)) || !em.buffer_in_scene {
                e.draw = None;
                continue;
            }
            let at = (em.get_position(), em.is_stopped());
            match em.buffer.render(clock, Some(at)) {
                Ok(d) => e.draw = Some(d),
                Err(err) => {
                    e.failed = Some(format!("{err}"));
                    e.draw = None;
                }
            }
        }
        for o in self.orphans.iter_mut() {
            if o.failed.is_none() {
                o.render(clock);
            }
        }
    }

    /// (#65 batch d) The rest of each line-mode buffer's `Render_Line` (`LineRenderer->Render(rinfo, ...)`), with the
    /// camera's view `view` (`CameraClass::Get_View_Matrix`, `CameraInvTransform`, which `Apply_D3D_State` gives
    /// `D3DTS_VIEW`): every run of the [`super::part_buf::ParticleDraw::Lines`] the last
    /// [`Self::update_emitters`] drew, through [`super::part_buf::ParticleBuffer::render_line_runs`] at `clock`, in the
    /// order the buffers rendered there (the emitters' in sub-object order, then the orphans'). `v3` is the
    /// randomizers' shared generator. Returns the chunks in that order; a buffer upstream is undefined on stops, with
    /// the reason in its `failed`, and draws nothing. **Differs from upstream** (see `render_line_runs`): upstream
    /// draws the runs inside each buffer's `Render`; here the caller draws them after the update, with the camera.
    pub fn render_line_runs(&mut self, clock: &super::part_buf::Ww3dClock, view: &Matrix3D, v3: &mut crate::ww::random::Random3) -> Vec<super::seglinerenderer::SegLineChunk> {
        use super::part_buf::ParticleDraw;
        let mut out = Vec::new();
        for e in self.emitters.iter_mut() {
            let EmitterSub { emitter, draw, failed, .. } = e;
            let (Some(em), Some(ParticleDraw::Lines { runs, .. }), None) = (emitter.as_mut(), draw.as_ref(), failed.as_ref()) else { continue };
            match em.buffer.render_line_runs(runs, clock, view, v3) {
                Ok(Some(chunks)) => out.extend(chunks.into_iter().flatten()),
                Ok(None) => {}
                Err(err) => {
                    *failed = Some(format!("{err}"));
                    *draw = None;
                }
            }
        }
        for o in self.orphans.iter_mut() {
            let OrphanBuffer { obj, draw, failed, .. } = o;
            let (Some(ParticleDraw::Lines { runs, .. }), None) = (draw.as_ref(), failed.as_ref()) else { continue };
            let buffer = match obj {
                Orphaned::Held(e) => &mut e.buffer,
                Orphaned::Dead(b) => b,
            };
            match buffer.render_line_runs(runs, clock, view, v3) {
                Ok(Some(chunks)) => out.extend(chunks.into_iter().flatten()),
                Ok(None) => {}
                Err(err) => {
                    *failed = Some(format!("{err}"));
                    *draw = None;
                }
            }
        }
        out
    }

    /// (#65 batch e) The rest of each line-group buffer's `Render_Line_Group` (`LineGroup->Render(rinfo)`), with the
    /// camera's transform `camera` (`rinfo.Camera.Get_Transform()`): every buffer whose last [`Self::update_emitters`]
    /// drew a [`super::part_buf::ParticleDraw::LineGroup`], through
    /// [`super::part_buf::ParticleBuffer::render_line_groups`], in the order the buffers rendered there (the emitters'
    /// in sub-object order, then the orphans'). Returns the draws in that order (world space); a buffer upstream is
    /// undefined on stops, with the reason in its `failed`, and draws nothing. **Differs from upstream** (as
    /// [`Self::render_line_runs`]): upstream draws inside each buffer's `Render`; here the caller draws after the update,
    /// with the camera. `LineGroupClass::Render` keeps no state but its shader's edits, which give the same bits every
    /// time: calling this more than once a frame draws the same.
    pub fn render_line_groups(&mut self, camera: &Matrix3D) -> Vec<super::linegrp::LineGroupChunk> {
        use super::part_buf::ParticleDraw;
        let mut out = Vec::new();
        for e in self.emitters.iter_mut() {
            let EmitterSub { emitter, draw, failed, .. } = e;
            let (Some(em), Some(ParticleDraw::LineGroup(_)), None) = (emitter.as_mut(), draw.as_ref(), failed.as_ref()) else { continue };
            match em.buffer.render_line_groups(camera) {
                Ok(Some(c)) => out.push(c),
                Ok(None) => {}
                Err(err) => {
                    *failed = Some(format!("{err}"));
                    *draw = None;
                }
            }
        }
        for o in self.orphans.iter_mut() {
            let OrphanBuffer { obj, draw, failed, .. } = o;
            let (Some(ParticleDraw::LineGroup(_)), None) = (draw.as_ref(), failed.as_ref()) else { continue };
            let buffer = match obj {
                Orphaned::Held(e) => &mut e.buffer,
                Orphaned::Dead(b) => b,
            };
            match buffer.render_line_groups(camera) {
                Ok(Some(c)) => out.push(c),
                Ok(None) => {}
                Err(err) => {
                    *failed = Some(format!("{err}"));
                    *draw = None;
                }
            }
        }
        out
    }

    /// `SimpleSceneClass::Register(obj, RELEASE)`: `ReleaseList.Add(obj)`, at the head (the caller checks it is not
    /// listed: `Add`'s once-only test).
    fn register_release(list: &mut Vec<u32>, id: u32) {
        if crate::logic::modules::plant(10145) {
            list.push(id);
        } else {
            list.insert(0, id);
        }
    }

    /// (#65 batch c) `HLodClass::Remove_Sub_Object(removeme)` for the emitter at `index` (`None`, upstream's 0, when
    /// there is none): it leaves the sub-object list (`Lod[lod].Delete(model)` or `AdditionalModels.Delete(model)`: the
    /// top LOD and the aggregates are both current here), loses its container, and, the HLOD being in the scene,
    /// `Notify_Removed` (the emitter stops; its buffer stays in the scene). The HLOD's reference is released
    /// (`Release_Ref`): the emitter is returned to the caller, who holds the other reference (the release list's) or
    /// destroys it. `Update_Sub_Object_Bits` and `Update_Obj_Space_Bounding_Volumes` are not kept (see the module docs).
    pub fn remove_sub_object(&mut self, index: usize) -> Option<EmitterSub> {
        if index >= self.emitters.len() {
            return None;
        }
        let mut e = self.emitters.remove(index);
        if let Some(em) = e.emitter.as_mut() {
            em.notify_removed();
        }
        Some(e)
    }

    /// (#65 batch c) An emitter destroyed (`~ParticleEmitterClass`): `Buffer->Emitter_Is_Dead()`; its buffer, if the scene
    /// has it, stays as an orphan (in id order); otherwise it goes with the emitter.
    fn destroy_emitter(&mut self, id: u32, name: Vec<u8>, joined: u32, em: super::part_emt::ParticleEmitter) {
        if !em.buffer_in_scene && !crate::logic::modules::plant(10146) {
            return;
        }
        let o = OrphanBuffer { id, name, obj: Orphaned::Dead(em.into_buffer()), buffer_joined: joined, in_release_list: false, draw: None, failed: None };
        let at = self.orphans.partition_point(|x| x.id < id);
        self.orphans.insert(at, o);
    }

    /// (#65 batch c) The scene's `Post_Render_Processing` for this model's objects. In a [`SceneKind::Simple`] scene,
    /// `SimpleSceneClass::Post_Render_Processing`: first each listed object, head first, is unlinked (an emitter still
    /// in the HLOD by [`Self::remove_sub_object`]; an object with no container by `Remove()`: a dead emitter's buffer
    /// leaves the scene, a held emitter, already out of the scene, is untouched), then the list is released head first
    /// (`Release_Head`): a released emitter is destroyed (its buffer orphaned, [`Orphaned::Dead`]), a removed buffer
    /// goes. Returns the ids processed, in order. In the world ([`SceneKind::World`]) nothing: `RTS3DScene::Render`
    /// never calls it, and the list keeps its entries.
    pub fn post_render_processing(&mut self, scene: SceneKind) -> Vec<u32> {
        if scene == SceneKind::World && !crate::logic::modules::plant(10142) {
            return Vec::new();
        }
        let list = std::mem::take(&mut self.release_list);
        // first pass: unlinked from the HLOD or the scene
        let mut released: Vec<EmitterSub> = Vec::new();
        let mut removed: Vec<u32> = Vec::new();
        for &id in &list {
            if let Some(i) = self.emitters.iter().position(|e| e.id == id) {
                if let Some(e) = self.remove_sub_object(i) {
                    released.push(e);
                }
            } else if let Some(o) = self.orphans.iter().find(|o| o.id == id) {
                if matches!(o.obj, Orphaned::Dead(_)) {
                    removed.push(id);
                }
            }
        }
        if !crate::logic::modules::plant(10143) {
            self.orphans.retain(|o| !removed.contains(&o.id));
        }
        // second pass: the list's references released
        for &id in &list {
            if let Some(k) = released.iter().position(|e| e.id == id) {
                let e = released.remove(k);
                if let Some(em) = e.emitter {
                    self.destroy_emitter(e.id, e.name, e.buffer_joined, em);
                }
            } else if let Some(k) = self.orphans.iter().position(|o| o.id == id && matches!(o.obj, Orphaned::Held(_))) {
                let o = self.orphans.remove(k);
                if let Orphaned::Held(em) = o.obj {
                    self.destroy_emitter(o.id, o.name, o.buffer_joined, em);
                }
            } else if let Some(o) = self.orphans.iter_mut().find(|o| o.id == id) {
                // a dead buffer the scene keeps (a planted run): delisted
                o.in_release_list = false;
            }
        }
        list
    }

    /// (#65 batch c) The model taken out of the scene and let go, as `W3DModelDraw::nukeCurrentRender` does
    /// (`Remove_Render_Object(m_renderObject)`, then `REF_PTR_RELEASE`): `HLodClass::Notify_Removed` stops each current
    /// sub-object (the emitters, `Notify_Removed`), then the HLOD's destruction (`Free`) clears each one's container and
    /// releases it: an emitter in the scene's release list lives on, held by it ([`Orphaned::Held`]); any other is
    /// destroyed and its buffer, if the scene has it, orphaned ([`Orphaned::Dead`]). What is left is a model with no
    /// meshes, hierarchy or emitters: its orphaned buffers and release list, which go on drawing and being processed
    /// through [`Self::update_emitters`] and [`Self::post_render_processing`].
    pub fn remove_from_scene(self) -> RenderModel {
        let mut rest = RenderModel {
            name: self.name,
            is_hlod: self.is_hlod,
            htree: None,
            sub_objects: Vec::new(),
            not_drawn: Vec::new(),
            emitters: Vec::new(),
            orphans: self.orphans,
            release_list: self.release_list,
            updates: self.updates,
        };
        let mut emitters = self.emitters;
        // HLodClass::Notify_Removed
        for e in emitters.iter_mut() {
            if let Some(em) = e.emitter.as_mut() {
                if !crate::logic::modules::plant(10141) {
                    em.notify_removed();
                }
            }
        }
        // HLodClass::Free
        for e in emitters {
            let (Some(em), None) = (e.emitter, &e.failed) else { continue };
            if e.in_release_list && !crate::logic::modules::plant(10144) {
                let o = OrphanBuffer {
                    id: e.id,
                    name: e.name,
                    obj: Orphaned::Held(em),
                    buffer_joined: e.buffer_joined,
                    in_release_list: true,
                    draw: None,
                    failed: None,
                };
                let at = rest.orphans.partition_point(|x| x.id < o.id);
                rest.orphans.insert(at, o);
            } else {
                rest.destroy_emitter(e.id, e.name, e.buffer_joined, em);
            }
        }
        rest
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

    /// #49: an emitter that is a sub-object of an HLOD, on the arm bone (at
    /// (0, 0, 5)), with the object at (10, 0, 0).
    ///
    /// Predicted before the run: the HLOD keeps the emitter (not in
    /// `not_drawn`) on bone 1; the first update (sync 0) makes and starts
    /// it and draws an empty point group (quads) (**Corrected 2026-11-14, #65
    /// batch c:** draws nothing, `None`: see `update_emitters`; predicted so
    /// in issue65c P5, held); 250 ms later two bursts
    /// of one (period 100 ms) are drawn, stamped 100 and 200, rising at 10
    /// units a second (+-1) from within 0.5 of (10, 0, 5): x within 10 +-
    /// 0.7, z between 5 and 7.5; their alpha (opacity 1 fading to 0 over
    /// the second) under 1.
    ///
    /// *Result (2026-11-07), first run:* held.
    #[test]
    fn an_hlod_emitter_emits_on_its_bone() {
        let ident = [0.0, 0.0, 0.0, 1.0];
        let mut f = htree_w3d(b"ROBOT", &[
            PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident },
            PivotSpec { name: b"ARM".to_vec(), parent: 0, translation: [0.0, 0.0, 5.0], rotation: ident },
        ]);
        f.extend(MeshSpec::cuboid(b"ROBOT", b"BOX", [-1.0; 3], [1.0; 3]).to_w3d());
        f.extend(EmitterSpec::puff(b"PUFF").to_w3d());
        f.extend(hlod_w3d(b"ROBOT", b"ROBOT", &[(b"ROBOT.BOX", 1), (b"PUFF", 1)]));
        let fs = fs_with(&[("Art/W3D/robot.w3d", f)]);
        let mut am = AssetManager::new_game(W3dFileFactory::default());
        let mut model = RenderModel::create(&mut am, &fs, b"ROBOT").unwrap().unwrap();
        assert!(model.not_drawn.is_empty(), "{:?}", model.not_drawn);
        assert_eq!((model.emitters.len(), model.emitters[0].bone_index), (1, 1));
        model.update::<AssetManager>(&Matrix3D::IDENTITY, None).unwrap();
        let world = Matrix3D::from_translation(&crate::ww::vector3::Vector3::new(10.0, 0.0, 0.0));
        let mut rng = super::super::part_emt::ParticleRandom::default();
        use super::super::part_buf::{ParticleDraw, Ww3dClock};
        model.update_emitters(&world, &Ww3dClock::default(), &mut rng);
        // ~~the first update draws an empty point group~~ Corrected 2026-11-14 (#65, batch c): the buffer joins the
        // scene during the update loop, after the visibility check, so it is not drawn in its first frame
        assert_eq!(model.emitters[0].draw, None, "{:?}", model.emitters[0].failed);
        assert!(model.emitters[0].emitter.as_ref().unwrap().buffer_in_scene);
        model.update_emitters(&world, &Ww3dClock { sync_time: 250, frame_time: 250, frame_count: 1 }, &mut rng);
        match &model.emitters[0].draw {
            Some(ParticleDraw::Points(p)) => {
                assert_eq!(p.points.len(), 2);
                for s in &p.points {
                    assert!((s.position.x - 10.0).abs() <= 0.7 && s.position.z > 5.0 && s.position.z < 7.5, "{s:?}");
                    assert!(s.diffuse.w < 1.0, "{s:?}");
                }
            }
            d => panic!("{d:?}"),
        }
    }

    /// (#65 batch d) A line-mode emitter (the puff, `W3D_EMITTER_RENDER_MODE_LINE`, intersections merged, no
    /// subdivision or noise) in an HLOD, at the origin, its view 100 back along the camera's -Z.
    ///
    /// *Predictions, written before the first run (issue65d P2):* the first update draws nothing, so
    /// `render_line_runs` gives no chunk; at 250 ms one run of 3 points (the two particles and the emitter's position)
    /// becomes one chunk of 6 vertices and 4 triangles with the puff's texture, equal to what
    /// `ParticleBuffer::render_line_runs` gives on a copy of the buffer with a copy of the generator, which ends as the
    /// model's.
    ///
    /// *Results (2026-11-14), first run:* all held (one chunk, 6 vertices, 4 triangles).
    #[test]
    fn line_mode_emitters_draw_their_runs_through_the_segment_renderer() {
        use super::super::part_buf::{ParticleDraw, Ww3dClock};
        let ident = [0.0, 0.0, 0.0, 1.0];
        let mut f = htree_w3d(b"ROBOT", &[PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident }]);
        let mut spec = EmitterSpec::puff(b"LINE");
        spec.render_mode = super::super::part_buf::W3D_EMITTER_RENDER_MODE_LINE as u32;
        let mut lp = W3dEmitterLinePropertiesStruct::from_bytes(&[0; W3dEmitterLinePropertiesStruct::SIZE]);
        (lp.flags, lp.merge_abort_factor, lp.texture_tile_factor) = (super::super::seglinerenderer::W3D_ELINE_MERGE_INTERSECTIONS, 1.5, 1.0);
        spec.line_props = Some(lp);
        f.extend(spec.to_w3d());
        f.extend(hlod_w3d(b"ROBOT", b"ROBOT", &[(b"LINE", 0)]));
        let fs = fs_with(&[("Art/W3D/robot.w3d", f)]);
        let mut am = AssetManager::new_game(W3dFileFactory::default());
        let mut model = RenderModel::create(&mut am, &fs, b"ROBOT").unwrap().unwrap();
        model.update::<AssetManager>(&Matrix3D::IDENTITY, None).unwrap();
        let mut rng = super::super::part_emt::ParticleRandom::default();
        let mut view = Matrix3D::identity();
        view.row[2].w = -100.0;
        let c0 = Ww3dClock::default();
        model.update_emitters(&Matrix3D::IDENTITY, &c0, &mut rng);
        assert!(model.render_line_runs(&c0, &view, &mut rng.v3).is_empty());
        let c1 = Ww3dClock { sync_time: 250, frame_time: 250, frame_count: 1 };
        model.update_emitters(&Matrix3D::IDENTITY, &c1, &mut rng);
        let runs = match &model.emitters[0].draw {
            Some(ParticleDraw::Lines { runs, .. }) => runs.clone(),
            d => panic!("{d:?} {:?}", model.emitters[0].failed),
        };
        assert_eq!(runs.iter().map(|r| r.points.len()).collect::<Vec<_>>(), vec![3]);
        let (mut buffer, mut v3) = (model.emitters[0].emitter.as_ref().unwrap().buffer.clone(), rng.v3.clone());
        let direct: Vec<_> = buffer.render_line_runs(&runs, &c1, &view, &mut v3).unwrap().unwrap().into_iter().flatten().collect();
        let chunks = model.render_line_runs(&c1, &view, &mut rng.v3);
        assert_eq!(chunks, direct);
        assert_eq!((chunks.len(), chunks[0].vertices.len(), chunks[0].triangles.len()), (1, 6, 4), "{chunks:?}");
        assert_eq!(chunks[0].texture, Some(b"puff.tga".to_vec()));
        assert_eq!((rng.v3.clone(), model.emitters[0].emitter.as_ref().unwrap().buffer.line_renderer.clone()), (v3, buffer.line_renderer));
    }

    /// (#65 batch e) A line-group emitter (the puff as `W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM`) in an HLOD, at the
    /// origin, the camera 100 back along +Z turned 30 degrees about z.
    ///
    /// *Predictions, written before the first run (issue65e P1):* the first update draws a line group of no lines, so
    /// `render_line_groups` gives no draw; at 250 ms 2 lines become one prism draw of 12 vertices and 16 triangles with
    /// the puff's texture, not in camera space, equal to what `ParticleBuffer::render_line_groups` gives on a copy of the
    /// buffer; a second call gives the same draw.
    ///
    /// *Results (2026-11-14), first run:* held.
    #[test]
    fn line_group_emitters_draw_their_prisms() {
        use super::super::part_buf::{ParticleDraw, Ww3dClock, W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM};
        let ident = [0.0, 0.0, 0.0, 1.0];
        let mut f = htree_w3d(b"ROBOT", &[PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident }]);
        let mut spec = EmitterSpec::puff(b"PRISMS");
        spec.render_mode = W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM as u32;
        f.extend(spec.to_w3d());
        f.extend(hlod_w3d(b"ROBOT", b"ROBOT", &[(b"PRISMS", 0)]));
        let fs = fs_with(&[("Art/W3D/robot.w3d", f)]);
        let mut am = AssetManager::new_game(W3dFileFactory::default());
        let mut model = RenderModel::create(&mut am, &fs, b"ROBOT").unwrap().unwrap();
        model.update::<AssetManager>(&Matrix3D::IDENTITY, None).unwrap();
        let mut rng = super::super::part_emt::ParticleRandom::default();
        let mut camera = Matrix3D::from_axis_angle(&crate::ww::vector3::Vector3::new(0.0, 0.0, 1.0), crate::ww::wwmath::WWMATH_PI / 6.0);
        camera.row[2].w = 100.0;
        let c0 = Ww3dClock::default();
        model.update_emitters(&Matrix3D::IDENTITY, &c0, &mut rng);
        assert!(model.render_line_groups(&camera).is_empty());
        let c1 = Ww3dClock { sync_time: 250, frame_time: 250, frame_count: 1 };
        model.update_emitters(&Matrix3D::IDENTITY, &c1, &mut rng);
        match &model.emitters[0].draw {
            Some(ParticleDraw::LineGroup(g)) => assert_eq!((g.prism, g.lines.len()), (true, 2)),
            d => panic!("{d:?} {:?}", model.emitters[0].failed),
        }
        let mut buffer = model.emitters[0].emitter.as_ref().unwrap().buffer.clone();
        let direct = buffer.render_line_groups(&camera).unwrap().expect("a draw");
        let chunks = model.render_line_groups(&camera);
        assert_eq!(chunks, vec![direct]);
        assert_eq!((chunks[0].vertices.len(), chunks[0].triangles.len(), chunks[0].view_identity), (12, 16, false));
        assert_eq!(chunks[0].texture, Some(b"puff.tga".to_vec()));
        assert_eq!(model.render_line_groups(&camera), chunks);
    }

    /// (#65 batch c) Two emitters (`PUFF`, 10 a second, bursts of 1, 1 s lifetimes) in an HLOD; the first given one
    /// particle to emit (so it completes in the 250 ms frame).
    fn two_puffs() -> RenderModel {
        let ident = [0.0, 0.0, 0.0, 1.0];
        let mut f = htree_w3d(b"ROBOT", &[PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident }]);
        f.extend(EmitterSpec::puff(b"PUFF").to_w3d());
        f.extend(hlod_w3d(b"ROBOT", b"ROBOT", &[(b"PUFF", 0), (b"PUFF", 0)]));
        let fs = fs_with(&[("Art/W3D/robot.w3d", f)]);
        let mut am = AssetManager::new_game(W3dFileFactory::default());
        RenderModel::create(&mut am, &fs, b"ROBOT").unwrap().unwrap()
    }

    /// (#65 batch c) *Predictions, written before the first run (issue65c):* in a simple scene, the capped emitter
    /// completes in the 250 ms frame and registers for release in that same frame (buffers run before emitters); the
    /// scene's post-render processing takes it out of the HLOD (the other stays) and leaves its buffer orphaned, dead,
    /// drawing its one particle until it ages out (emitted at 100 ms, gone by 1,500 ms), then registering and being
    /// removed at the next processing. In the world nothing is processed: the request stays listed once however many
    /// frames repeat it; removing the HLOD leaves the listed emitter held (alive) and the other's buffer dead, both
    /// drawing until drained.
    #[test]
    fn released_emitters_leave_their_buffers_drawing() {
        use super::super::part_buf::Ww3dClock;
        let c = |t: u32, dt: u32, n: u32| Ww3dClock { sync_time: t, frame_time: dt, frame_count: n };
        let points = |d: &Option<super::super::part_buf::ParticleDraw>| match d {
            Some(super::super::part_buf::ParticleDraw::Points(p)) => p.points.len(),
            d => panic!("{d:?}"),
        };
        for scene in [SceneKind::Simple, SceneKind::World] {
            let mut model = two_puffs();
            assert_eq!(model.emitters.iter().map(|e| e.id).collect::<Vec<_>>(), vec![0, 1]);
            let mut rng = super::super::part_emt::ParticleRandom::default();
            model.update_emitters(&Matrix3D::IDENTITY, &c(0, 0, 0), &mut rng);
            assert!(model.emitters.iter().all(|e| e.draw.is_none()));
            model.emitters[0].emitter.as_mut().unwrap().particles_left = 1;
            model.update_emitters(&Matrix3D::IDENTITY, &c(250, 250, 1), &mut rng);
            assert_eq!((points(&model.emitters[0].draw), points(&model.emitters[1].draw)), (1, 2));
            assert_eq!(model.release_list, vec![0]);
            if scene == SceneKind::World {
                assert_eq!(model.post_render_processing(scene), Vec::<u32>::new());
                model.update_emitters(&Matrix3D::IDENTITY, &c(300, 50, 2), &mut rng);
                assert_eq!((model.release_list.clone(), model.emitters.len()), (vec![0], 2), "listed once, never processed");
                let mut rest = model.remove_from_scene();
                assert!(rest.emitters.is_empty() && rest.sub_objects.is_empty());
                assert!(matches!(rest.orphans[0].obj, Orphaned::Held(_)) && matches!(rest.orphans[1].obj, Orphaned::Dead(_)));
                rest.update_emitters(&Matrix3D::IDENTITY, &c(500, 200, 3), &mut rng);
                assert_eq!((points(&rest.orphans[0].draw), points(&rest.orphans[1].draw)), (1, 2));
                rest.update_emitters(&Matrix3D::IDENTITY, &c(1500, 1000, 4), &mut rng);
                assert!(rest.orphans.iter().all(|o| o.is_drained()), "{:?}", rest.orphans.iter().map(|o| o.buffer().get_particle_count()).collect::<Vec<_>>());
                rest.update_emitters(&Matrix3D::IDENTITY, &c(1600, 100, 5), &mut rng);
                assert_eq!(rest.release_list, vec![1, 0], "the dead buffer registers; the held emitter stays listed");
                assert_eq!(rest.post_render_processing(scene), Vec::<u32>::new());
                assert_eq!(rest.orphans.len(), 2, "the world keeps them");
                continue;
            }
            assert_eq!(model.post_render_processing(scene), vec![0]);
            assert_eq!(model.emitters.iter().map(|e| e.id).collect::<Vec<_>>(), vec![1]);
            assert_eq!(model.orphans.len(), 1);
            assert!(matches!(model.orphans[0].obj, Orphaned::Dead(_)) && !model.orphans[0].is_drained());
            assert!(model.release_list.is_empty());
            model.update_emitters(&Matrix3D::IDENTITY, &c(500, 250, 2), &mut rng);
            assert_eq!(points(&model.orphans[0].draw), 1);
            model.update_emitters(&Matrix3D::IDENTITY, &c(1500, 1000, 3), &mut rng);
            assert_eq!(points(&model.orphans[0].draw), 0);
            assert!(model.orphans[0].is_drained() && model.release_list.is_empty());
            model.update_emitters(&Matrix3D::IDENTITY, &c(1600, 100, 4), &mut rng);
            assert_eq!(model.release_list, vec![0]);
            assert_eq!(model.post_render_processing(scene), vec![0]);
            assert!(model.orphans.is_empty() && model.remove_sub_object(5).is_none());
        }
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
