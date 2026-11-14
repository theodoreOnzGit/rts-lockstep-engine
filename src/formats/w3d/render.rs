// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hlod.cpp
//   (HLodClass::HLodClass(const HLodDefClass &): the sub-objects of each LOD
//   and the aggregates, bound to their bones; #65 batch c: Remove_Sub_Object,
//   Notify_Removed, Free's release of the sub-objects; #181 LOD: Add_Lod_Model's
//   bone clamp, Add_Sub_Object_To_Bone's bone check, Recalculate_Static_LOD_Factors,
//   Calculate_Cost_Value_Arrays, Set_LOD_Level, Get_LOD_Level, Get_LOD_Count,
//   Render's Lod[CurLod]), hlod.h (ModelArrayClass's factors)
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
//   Get_Deformed_Vertices; #181 LOD: Get_Num_Polys)
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
//! - ~~**The LOD** is the highest (`LodCount - 1`); upstream picks by screen
//!   size and cost.~~ **Corrected 2026-11-14 (#181 LOD):** the drawn model
//!   ([`RenderModel::create_drawn`]) draws `Lod[CurLod]`, the LOD upstream's
//!   constructor selects ([`HLodLevels::new`]); Zero Hour never changes it
//!   afterwards (`Prepare_LOD` and the LOD optimizer are commented out in the
//!   game). [`RenderModel::create`], the skeleton's view, still lists the top
//!   LOD (#181 item 13). Sub-objects that are not meshes (boxes, nulls, nested
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
    /// mesh, or when the HLOD's could not be found. **(#181 items 9, 10)** The client's drawn model instead takes the
    /// live render object's pivots, in the world (`client::w3d_model_draw::RenderObj::pose_render_model`), and an HLOD
    /// whose hierarchy was not found takes upstream's default one there ([`HTree::init_default`]).
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
    /// (#181 LOD) `HLodClass`'s levels of detail as its constructor leaves them ([`HLodLevels`]: `LodCount`, `CurLod`,
    /// `Cost`, `Value`); the drawn sub-objects are `Lod[CurLod]`'s and the aggregates' ([`RenderModel::create_drawn`]).
    /// `None` for a lone mesh, and for the skeleton's view ([`RenderModel::create`]), which does not select a LOD.
    pub lod: Option<HLodLevels>,
    /// (#181 LOD) For each of [`Self::sub_objects`], its index among the sub-objects the draw module's skeleton lists
    /// (`client::w3d_model_draw::Skeleton`, made from [`RenderModel::create`]: the top LOD's meshes, then the
    /// aggregates'), by which the render object keeps its hidden flags and material overrides; `None` for a drawn
    /// sub-object the skeleton does not list (a mesh of a LOD below the top). `None` (the whole field) when the two lists
    /// are the same, index for index.
    pub skeleton_index: Option<Vec<Option<usize>>>,
    /// (#181 LOD) What `HLodClass::Cast_Ray` (hlod.cpp:2686-2706) casts at that the drawn list does not hold: when the
    /// drawn LOD (`Lod[CurLod]`) is not the top one, the top LOD's meshes (their bones as `Add_Lod_Model` left them),
    /// and how many of [`Self::sub_objects`] are the drawn LOD's (the rest are the aggregates). `None` when the drawn
    /// LOD is the top one. See [`RenderModel::cast_ray_sub_objects`].
    pub cast_ray_top: Option<(Vec<SubObject>, usize)>,
}

/// (#181 LOD) `HLodClass::ModelArrayClass`'s factors (hlod.h:244-252), without its models: `MaxScreenSize`,
/// `NonPixelCost`, `PixelCostPerArea`, `BenefitFactor`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodFactors {
    pub max_screen_size: f32,
    pub non_pixel_cost: f32,
    pub pixel_cost_per_area: f32,
    pub benefit_factor: f32,
}

impl Default for LodFactors {
    /// `ModelArrayClass(void) : MaxScreenSize(NO_MAX_SCREEN_SIZE), NonPixelCost(0.0f), PixelCostPerArea(0.0f),
    /// BenefitFactor(0.0f)`.
    fn default() -> Self {
        LodFactors { max_screen_size: super::w3d_file::NO_MAX_SCREEN_SIZE, non_pixel_cost: 0.0, pixel_cost_per_area: 0.0, benefit_factor: 0.0 }
    }
}

/// (#181 LOD) `HLodClass`'s level-of-detail state (hlod.h): `LodCount`, `CurLod`, `Lod[i]`'s factors, `Cost`
/// (`LodCount` entries), `Value` (`LodCount + 1`: "so PostIncrementValue can always use Value[CurLod + 1]") and
/// `LODBias`.
///
/// **What Zero Hour drives.** Only the constructors' last step: `Recalculate_Static_LOD_Factors()`, then
/// `minlod = Calculate_Cost_Value_Arrays(1.0f, Value, Cost)` ("a screen area of 1 pixel") and
/// `if (CurLod < minlod) Set_LOD_Level(minlod)` ([`HLodLevels::new`]). `RTS3DScene::Customized_Render` has its
/// `Prepare_LOD` commented out ("We're not using LOD yet", W3DScene.cpp:520-526) and `W3DDisplay` its
/// `PredictiveLODOptimizerClass::Optimize_LODs` (W3DDisplay.cpp:1752), and no game code calls `Set_LOD_Level`,
/// `Increment_LOD`, `Decrement_LOD`, `Set_Max_Screen_Size`, `Include_NULL_Lod` or `Set_LOD_Bias` on an HLOD: so an
/// HLOD draws, for its whole life, the first LOD whose `MaxScreenSize` is not below 1 (else the top one). Those
/// functions, `Prepare_LOD`, `Get_Cost`, `Get_Value` and `Get_Post_Increment_Value` are not translated.
#[derive(Clone, Debug, PartialEq)]
pub struct HLodLevels {
    pub lod_count: i32,
    pub cur_lod: i32,
    pub lods: Vec<LodFactors>,
    pub cost: Vec<f32>,
    pub value: Vec<f32>,
    pub lod_bias: f32,
}

impl HLodLevels {
    /// The level-of-detail part of `HLodClass(const HLodDefClass & def)` (hlod.cpp:1082-1156): `LodCount =
    /// def.LodCount`, `CurLod(0)`, `LODBias(1.0f)`, `Lod[ilod].MaxScreenSize = def.Lod[ilod].MaxScreenSize`; then, the
    /// models added, `Recalculate_Static_LOD_Factors()` (`polycounts`: each LOD's poly count over its models that are
    /// `Is_Not_Hidden_At_All()`), and "so that the object is ready for use after construction"
    /// `minlod = Calculate_Cost_Value_Arrays(1.0f, Value, Cost)` and "Ensure lod is no less than minimum allowed":
    /// `if (CurLod < minlod) Set_LOD_Level(minlod)`. `Cost` and `Value` start at 0 here (upstream's `new float[]`,
    /// which the game's allocator zero-fills); the calculation writes every entry when `LodCount >= 1`.
    pub fn new(max_screen_sizes: &[f32], polycounts: &[i32]) -> HLodLevels {
        let lod_count = max_screen_sizes.len() as i32;
        let lods = max_screen_sizes.iter().map(|&m| LodFactors { max_screen_size: m, ..LodFactors::default() }).collect();
        let n = max_screen_sizes.len();
        let mut l = HLodLevels { lod_count, cur_lod: 0, lods, cost: vec![0.0; n], value: vec![0.0; n + 1], lod_bias: 1.0 };
        l.recalculate_static_lod_factors(polycounts);
        let minlod = l.calculate_cost_value_arrays(1.0f32);
        if l.cur_lod < minlod && !crate::logic::modules::plant(10279) {
            l.set_lod_level(minlod);
        }
        l
    }

    /// `HLodClass::Recalculate_Static_LOD_Factors()` (hlod.cpp:2922-2952): for each LOD, `PixelCostPerArea = 0` ("there
    /// are no pixel-related costs"), `NonPixelCost` its poly count (`polycounts[i]`, the sum of `Get_Num_Polys()` over
    /// its models that are `Is_Not_Hidden_At_All()`) or `0.000001f` for none ("to avoid divisions by zero"), and
    /// `BenefitFactor = 1 - (0.5f / (polycount * polycount))`, 0 for none. The `int` product wraps as the 32-bit
    /// original's does (formally undefined past 46,340 polygons). A LOD with no entry in `polycounts` has none.
    pub fn recalculate_static_lod_factors(&mut self, polycounts: &[i32]) {
        for (i, l) in self.lods.iter_mut().enumerate() {
            // Currently there are no pixel-related costs taken into account
            l.pixel_cost_per_area = 0.0f32;
            let polycount = polycounts.get(i).copied().unwrap_or(0);
            // If polycount is zero set Cost to a small nonzero amount to avoid divisions by zero.
            l.non_pixel_cost = if polycount != 0 { polycount as f32 } else if crate::logic::modules::plant(10287) { 0.0f32 } else { 0.000001f32 };
            // A polycount of zero yields a benefit factor of zero: otherwise apply formula.
            let square = if crate::logic::modules::plant(10286) { polycount } else { polycount.wrapping_mul(polycount) };
            l.benefit_factor = if polycount != 0 { 1.0f32 - (0.5f32 / square as f32) } else { 0.0f32 };
        }
    }

    /// `HLodClass::Calculate_Cost_Value_Arrays(screen_area, Value, Cost)` (hlod.cpp:3164-3205): each LOD's cost
    /// `NonPixelCost + PixelCostPerArea * screen_area`; every LOD whose `MaxScreenSize` is smaller than `screen_area`,
    /// and the first one after them (unless there is none: then `LodCount - 1`), valued `AT_MIN_LOD`; that one is
    /// `minlod`, returned; each later one `(BenefitFactor * screen_area * LODBias) / cost`; `Value[LodCount]`
    /// `AT_MAX_LOD`. With no LODs `minlod` is -1 (and only `Value[0]` is written).
    pub fn calculate_cost_value_arrays(&mut self, screen_area: f32) -> i32 {
        use super::part_buf::{AT_MAX_LOD, AT_MIN_LOD};
        let lod_count = self.lod_count;
        // Calculate Cost heuristic for each LOD based on normalized screen area:
        for lod in 0..lod_count.max(0) as usize {
            self.cost[lod] = self.lods[lod].non_pixel_cost + self.lods[lod].pixel_cost_per_area * screen_area;
        }
        // Calculate Value heuristic. First, all LOD levels for which MaxScreenSize is smaller than screen_area have
        // their Value set to AT_MIN_LOD, as well as the first LOD after that (unless there are no other LODs):
        let mut lod = 0i32;
        while lod < lod_count && {
            let m = self.lods[lod as usize].max_screen_size;
            m < screen_area || (crate::logic::modules::plant(10276) && m == screen_area)
        } {
            self.value[lod as usize] = AT_MIN_LOD;
            lod += 1;
        }
        if lod >= lod_count {
            lod = if crate::logic::modules::plant(10278) { 0 } else { lod_count - 1 };
        } else {
            self.value[lod as usize] = AT_MIN_LOD;
        }
        // Now lod is the lowest allowed - return this value.
        let minlod = lod;
        // Calculate Value heuristic for any remaining LODs based on normalized screen area:
        lod += 1;
        while lod < lod_count {
            let l = &self.lods[lod as usize];
            self.value[lod as usize] = (l.benefit_factor * screen_area * self.lod_bias) / self.cost[lod as usize];
            lod += 1;
        }
        // Post-inc value will flag max LOD.
        if let Some(v) = self.value.get_mut(lod_count.max(0) as usize) {
            *v = if crate::logic::modules::plant(10288) { AT_MIN_LOD } else { AT_MAX_LOD };
        }
        minlod
    }

    /// `HLodClass::Set_LOD_Level(lod)` (hlod.cpp:3090-3115) on an HLOD not in a scene (the constructor's call): `lod`
    /// clamped to `0..=LodCount - 1` (`MAX` then `MIN`), and `CurLod` set. In a scene upstream also moves the old LOD's
    /// models out (`Notify_Removed`) and the new one's in (`Notify_Added`); Zero Hour never calls it there (see the
    /// type's docs), so that part is not translated.
    pub fn set_lod_level(&mut self, lod: i32) {
        let lod = lod.max(0);
        let lod = lod.min(self.lod_count - 1);
        if lod == self.cur_lod {
            return;
        }
        self.cur_lod = lod;
    }

    /// `HLodClass::Get_LOD_Level()` (hlod.cpp:3128): `CurLod`.
    pub fn get_lod_level(&self) -> i32 {
        self.cur_lod
    }

    /// `HLodClass::Get_LOD_Count()` (hlod.cpp:3146): `LodCount`.
    pub fn get_lod_count(&self) -> i32 {
        self.lod_count
    }
}

/// (#181 LOD) `MeshClass::Get_Num_Polys()` (mesh.cpp:635-645): the model's pass count times its polygon count (an
/// `int`, wrapping as the original's).
pub fn mesh_num_polys(m: &MeshModel) -> i32 {
    (m.pass_count as i32).wrapping_mul(m.poly_count() as i32)
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

/// (#181 LOD) A sub-object as `Create_Render_Obj(name)` made it for an HLOD, with the bone the definition gives it.
#[derive(Clone, Debug, PartialEq)]
pub enum MadeSubObject {
    /// A mesh (`MeshClass`).
    Mesh(SubObject),
    /// A particle emitter (#49): name, bone, definition.
    Emitter(Vec<u8>, i32, super::part_ldr::ParticleEmitterDef),
    /// Another render object, or none: name, bone, and why it is not drawn.
    NotDrawn(Vec<u8>, i32, String),
}

impl MadeSubObject {
    /// Its bone.
    pub fn bone_index(&self) -> i32 {
        match self {
            MadeSubObject::Mesh(s) => s.bone_index,
            MadeSubObject::Emitter(_, b, _) | MadeSubObject::NotDrawn(_, b, _) => *b,
        }
    }

    fn set_bone_index(&mut self, bone: i32) {
        match self {
            MadeSubObject::Mesh(s) => s.bone_index = bone,
            MadeSubObject::Emitter(_, b, _) | MadeSubObject::NotDrawn(_, b, _) => *b = bone,
        }
    }

    /// Its name.
    pub fn name(&self) -> &[u8] {
        match self {
            MadeSubObject::Mesh(s) => &s.name,
            MadeSubObject::Emitter(n, _, _) | MadeSubObject::NotDrawn(n, _, _) => n,
        }
    }
}

/// `Create_Render_Obj(sub_name)` for an HLOD's sub-object on `bone`.
fn make_sub_object<D: LocalDevice>(am: &mut AssetManager, fs: &FileSystem<D>, sub_name: Vec<u8>, bone: i32) -> W3dResult<MadeSubObject> {
    Ok(match am.create_render_obj(fs, &sub_name)?.cloned() {
        Some(Prototype::Mesh(mesh)) => {
            let model = MeshModel::from_mesh(&mesh)?;
            let hidden = model.w3d_attributes & W3D_MESH_FLAG_HIDDEN != 0;
            MadeSubObject::Mesh(SubObject { name: sub_name, bone_index: bone, hidden, model, uv_offset: None })
        }
        Some(Prototype::ParticleEmitter(def)) => MadeSubObject::Emitter(sub_name, bone, def),
        Some(other) => {
            let why = format!("a {} prototype, not a mesh", prototype_kind(&other));
            MadeSubObject::NotDrawn(sub_name, bone, why)
        }
        None => MadeSubObject::NotDrawn(sub_name, bone, "no such render object".into()),
    })
}

impl RenderModel {
    /// The render object `Create_Render_Obj(name)` makes, ~~as drawn~~ **Corrected 2026-11-14 (#181 LOD):** as the
    /// draw module's skeleton reads it (`client::w3d_model_draw::W3dFiles::skeleton`, which feeds the logic's art bones,
    /// #175): an HLOD ~~(its top LOD and aggregates)~~ with its top LOD's sub-objects and the aggregates, every bone as
    /// the file gives it, or a mesh. Upstream draws `Lod[CurLod]` (the LOD its constructor selects) and clamps or
    /// refuses bones out of range; the client's drawn model is [`RenderModel::create_drawn`]. This view is kept as it
    /// was because changing what the skeleton lists is the owner's decision (#181 item 13). `None` when there is no
    /// such prototype, or it is another kind.
    pub fn create<D: LocalDevice>(am: &mut AssetManager, fs: &FileSystem<D>, name: &[u8]) -> W3dResult<Option<RenderModel>> {
        let proto = match am.create_render_obj(fs, name)? {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        match proto {
            Prototype::Mesh(mesh) => Ok(Some(RenderModel::lone_mesh(name, &mesh)?)),
            Prototype::HLod(def) => {
                let htree = am.get_htree(fs, &def.hierarchy_tree_name)?.cloned();
                let mut out = RenderModel::empty_hlod(name, htree);
                let mut subs = Vec::new();
                if let Some(top) = def.lods.last() {
                    subs.extend(top.sub_objects.iter().cloned());
                }
                subs.extend(def.aggregates.sub_objects.iter().cloned());
                for s in subs {
                    let Some(sub_name) = s.name else { continue };
                    let made = make_sub_object(am, fs, sub_name, s.bone_index)?;
                    out.add_made(made);
                }
                Ok(Some(out))
            }
            _ => Ok(None),
        }
    }

    /// (#181 LOD) The render object `Create_Render_Obj(name)` makes, as upstream draws it (the client's drawn model).
    /// A mesh as [`RenderModel::create`]'s. An HLOD as `HLodClass(const HLodDefClass & def)` (hlod.cpp:1082-1156) makes
    /// it: the hierarchy `def.HierarchyTreeName`'s (upstream's default one, `Init_Default`, when it is not found:
    /// `htree` `None`, one pivot); each LOD's sub-objects created by name (`Create_Render_Obj`) and added by
    /// `Add_Lod_Model` (hlod.cpp:3519-3541: a bone at or past `HTree->Num_Pivots()` becomes bone 0, "survive the case
    /// where the skeleton for this object no longer has the bone"); the LOD factors and the LOD selection
    /// ([`HLodLevels::new`]: the first LOD whose `MaxScreenSize` is not below 1, else the top); then the aggregates,
    /// each by `Add_Sub_Object_To_Bone` (hlod.cpp:2562-2592: one on a bone outside `0..Num_Pivots()` is refused, and
    /// is listed in [`RenderModel::not_drawn`]). The drawn sub-objects are what `HLodClass::Render` draws
    /// (hlod.cpp:2146-2170): `Lod[CurLod]`'s models (meshes in [`RenderModel::sub_objects`], emitters in
    /// [`RenderModel::emitters`], the others in [`RenderModel::not_drawn`]), then the aggregates
    /// (`AdditionalModels`; `SUBOBJS_MATCH_LOD` is never set on a loaded HLOD). [`RenderModel::skeleton_index`] maps
    /// them to the skeleton's list. A LOD's poly count (`Get_Num_Polys`) is its meshes' ([`mesh_num_polys`]); other
    /// render objects count 0 here (only `Cost` and `Value` read it, and nothing in Zero Hour reads those). With
    /// `LodCount` 0 (undefined upstream: `Render` reads `Lod[0]` of an empty array) only the aggregates are drawn.
    pub fn create_drawn<D: LocalDevice>(am: &mut AssetManager, fs: &FileSystem<D>, name: &[u8]) -> W3dResult<Option<RenderModel>> {
        let proto = match am.create_render_obj(fs, name)? {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        match proto {
            Prototype::Mesh(mesh) => Ok(Some(RenderModel::lone_mesh(name, &mesh)?)),
            Prototype::HLod(def) => {
                let htree = am.get_htree(fs, &def.hierarchy_tree_name)?.cloned();
                let mut lods = Vec::new();
                for lod in &def.lods {
                    let mut made = Vec::new();
                    for s in &lod.sub_objects {
                        let Some(sub_name) = s.name.clone() else { continue };
                        made.push(make_sub_object(am, fs, sub_name, s.bone_index)?);
                    }
                    lods.push((lod.max_screen_size, made));
                }
                let mut aggregates = Vec::new();
                for s in &def.aggregates.sub_objects {
                    let Some(sub_name) = s.name.clone() else { continue };
                    aggregates.push(make_sub_object(am, fs, sub_name, s.bone_index)?);
                }
                Ok(Some(RenderModel::hlod_from_def(name, htree, lods, aggregates)))
            }
            _ => Ok(None),
        }
    }

    /// (#181 LOD) The HLOD [`RenderModel::create_drawn`] makes, from the sub-objects already created
    /// ([`MadeSubObject`], each with the bone the definition gives it): `lods` each LOD's `MaxScreenSize` and models,
    /// `aggregates` the aggregates. See [`RenderModel::create_drawn`].
    pub fn hlod_from_def(name: &[u8], htree: Option<HTree>, lods: Vec<(f32, Vec<MadeSubObject>)>, aggregates: Vec<MadeSubObject>) -> RenderModel {
        use crate::logic::modules::plant;
        // Init_Default's one pivot when the hierarchy was not found
        let num_pivots = htree.as_ref().map_or(1, |t| t.num_pivots());
        let mut out = RenderModel::empty_hlod(name, htree);
        // the skeleton's list (RenderModel::create): the top LOD's meshes, then every aggregate mesh
        let top_meshes = lods.last().map_or(0, |(_, l)| l.iter().filter(|m| matches!(m, MadeSubObject::Mesh(_))).count());
        // HLodClass::Add_Lod_Model's clamp
        let lods: Vec<(f32, Vec<MadeSubObject>)> = lods
            .into_iter()
            .map(|(size, models)| {
                let models = models
                    .into_iter()
                    .map(|mut m| {
                        if m.bone_index() >= num_pivots && !plant(10280) {
                            m.set_bone_index(0);
                        }
                        m
                    })
                    .collect();
                (size, models)
            })
            .collect();
        let polycounts: Vec<i32> = lods
            .iter()
            .map(|(_, models)| {
                models.iter().fold(0i32, |n, m| match m {
                    MadeSubObject::Mesh(s) if !s.hidden || plant(10289) => n.wrapping_add(mesh_num_polys(&s.model)),
                    _ => n,
                })
            })
            .collect();
        let sizes: Vec<f32> = lods.iter().map(|(m, _)| *m).collect();
        let levels = HLodLevels::new(&sizes, &polycounts);
        let top = lods.len().checked_sub(1);
        let drawn_lod = if plant(10277) { top } else { usize::try_from(levels.cur_lod).ok().filter(|&c| c < lods.len()) };
        let mut index: Vec<Option<usize>> = Vec::new();
        let mut identity = drawn_lod == top;
        let mut lods = lods;
        if let Some(c) = drawn_lod {
            let models = std::mem::take(&mut lods[c].1);
            let on_top = Some(c) == top;
            for m in models {
                if matches!(m, MadeSubObject::Mesh(_)) {
                    index.push(if on_top { Some(index.len()) } else { None });
                }
                out.add_made(m);
            }
        }
        // (#181 LOD) HLodClass::Cast_Ray's "collide against the top LOD", kept apart when it is not the drawn one
        if let Some(t) = top.filter(|&t| Some(t) != drawn_lod) {
            let meshes: Vec<SubObject> = lods[t].1.iter().filter_map(|m| if let MadeSubObject::Mesh(s) = m { Some(s.clone()) } else { None }).collect();
            out.cast_ray_top = Some((meshes, out.sub_objects.len()));
        }
        if plant(10291) {
            // the pre-#181-LOD emitters: the top LOD's
            out.emitters.clear();
            if let Some(t) = top {
                for m in std::mem::take(&mut lods[t].1) {
                    if let MadeSubObject::Emitter(..) = m {
                        out.add_made(m);
                    }
                }
            }
        }
        // HLodClass::Add_Sub_Object_To_Bone: "if ((boneindex < 0) || (boneindex >= HTree->Num_Pivots())) return 0;"
        let mut agg_mesh = 0usize;
        for m in aggregates {
            let is_mesh = matches!(m, MadeSubObject::Mesh(_));
            let bone = m.bone_index();
            if (bone < 0 || bone >= num_pivots) && !plant(10281) {
                out.not_drawn.push((m.name().to_vec(), format!("bone {bone} out of range: Add_Sub_Object_To_Bone refuses it")));
                if is_mesh {
                    agg_mesh += 1;
                    identity = false;
                }
                continue;
            }
            if is_mesh {
                index.push(Some(top_meshes + agg_mesh));
                agg_mesh += 1;
            }
            out.add_made(m);
        }
        out.lod = Some(levels);
        out.skeleton_index = if identity { None } else { Some(index) };
        out
    }

    /// A lone mesh's render model (`MeshClass`).
    fn lone_mesh(name: &[u8], mesh: &super::mesh::Mesh) -> W3dResult<RenderModel> {
        let model = MeshModel::from_mesh(mesh)?;
        let hidden = model.w3d_attributes & W3D_MESH_FLAG_HIDDEN != 0;
        Ok(RenderModel {
            name: name.to_vec(),
            is_hlod: false,
            htree: None,
            sub_objects: vec![SubObject { name: name.to_vec(), bone_index: 0, hidden, model, uv_offset: None }],
            not_drawn: Vec::new(),
            emitters: Vec::new(),
            orphans: Vec::new(),
            release_list: Vec::new(),
            updates: 0,
            lod: None,
            skeleton_index: None,
            cast_ray_top: None,
        })
    }

    /// An HLOD's render model with no sub-objects yet.
    fn empty_hlod(name: &[u8], htree: Option<HTree>) -> RenderModel {
        RenderModel {
            name: name.to_vec(),
            is_hlod: true,
            htree,
            sub_objects: Vec::new(),
            not_drawn: Vec::new(),
            emitters: Vec::new(),
            orphans: Vec::new(),
            release_list: Vec::new(),
            updates: 0,
            lod: None,
            skeleton_index: None,
            cast_ray_top: None,
        }
    }

    /// A made sub-object into the model: a mesh drawn, an emitter (its id the next), the others listed as not drawn.
    fn add_made(&mut self, m: MadeSubObject) {
        match m {
            MadeSubObject::Mesh(s) => self.sub_objects.push(s),
            MadeSubObject::Emitter(name, bone_index, def) => {
                let id = self.emitters.len() as u32;
                self.emitters.push(EmitterSub { name, bone_index, def, emitter: None, draw: None, failed: None, id, in_release_list: false, buffer_joined: 0 });
            }
            MadeSubObject::NotDrawn(name, _, why) => self.not_drawn.push((name, why)),
        }
    }

    /// ~~The hierarchy update a render does~~ **Corrected 2026-11-13 (#180):** the hierarchy posed through the
    /// *virtual* `HTreeClass::Anim_Update(root, motion, frame)` ([`HTree::anim_update`], which interpolates a raw
    /// animation between frames), or the base pose, under `root`. A render does not do this upstream: a render object
    /// takes animobj.h's inline update, [`RenderModel::update_render_obj`], ~~which the client's drawn pose uses~~
    /// **Corrected 2026-11-14 (#181 item 9):** whose dispatch the live render object runs, and the client's drawn pose
    /// copies the live render object's hierarchy (`client::w3d_model_draw::RenderObj::pose_render_model`). This one
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
    /// render object's `Transform`. Nothing without a hierarchy. ~~The client's drawn pose
    /// (`rts_client::drawables::WorldDrawables`).~~ **Corrected 2026-11-14 (#181 item 9):** the client's drawn pose no
    /// longer calls this: it takes the live render object's hierarchy as its own last update left it
    /// (`client::w3d_model_draw::RenderObj::pose_render_model`), which runs this same dispatch
    /// ([`HTree::anim_update_animobj`]) under the render object's `Transform`. Kept for `differential_w3d`'s op 50.
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
    /// **(#181 item 9)** In the client's drawn model the bones are in the world (the live render object's), so this is
    /// the world transform upstream's renderer gives the sub-object; a lone mesh's is the render object's `Transform`,
    /// which `RenderObj::pose_render_model` returns.
    pub fn sub_object_transform(&self, i: usize) -> W3dResult<Matrix3D> {
        let s = self.sub_objects.get(i).ok_or(W3dUndefined::IndexOutOfRange)?;
        self.sub_transform(s)
    }

    /// (#181 item 9) `!Is_Not_Animation_Hidden()` of sub-object `i`: hidden by its bone's animated visibility
    /// (`HLodClass::Update_Sub_Object_Transforms`'s `Set_Animation_Hidden(!HTree->Get_Visibility(bone))`, hlod.cpp:
    /// 3370-3382); never without a hierarchy, or for a bone out of range (undefined upstream). The visibility is the
    /// hierarchy's last update's (the client's drawn model: the live render object's).
    pub fn sub_object_animation_hidden(&self, i: usize) -> bool {
        let Some(s) = self.sub_objects.get(i) else { return false };
        match &self.htree {
            Some(t) => !t.get_visibility(s.bone_index).unwrap_or(true),
            None => false,
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
            lod: None,
            skeleton_index: None,
            cast_ray_top: None,
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

    /// (#181 LOD) The meshes `HLodClass::Cast_Ray` (hlod.cpp:2686-2706) casts at, in its order: "collide against the
    /// top LOD" (`Lod[LodCount-1]`'s models), then `AdditionalModels`. The top LOD is the drawn one unless
    /// [`Self::cast_ray_top`] holds it; a lone mesh's is itself. Their transforms ([`Self::sub_transform`]) and skins'
    /// vertices ([`Self::deformed_sub`]) are the hierarchy's, which the client's posing gives the live pivots
    /// (`RenderObj::pose_render_model`, which also gives these meshes the render object's hidden flags). Upstream's
    /// first `if (Are_Sub_Object_Transforms_Dirty()) Update_Sub_Object_Transforms();` is the client's: it picks against
    /// the pose of the last draw.
    pub fn cast_ray_sub_objects(&self) -> Vec<&SubObject> {
        match &self.cast_ray_top {
            Some((top, n)) if !crate::logic::modules::plant(10293) => top.iter().chain(self.sub_objects.iter().skip(*n)).collect(),
            _ => self.sub_objects.iter().collect(),
        }
    }

    /// (#181 LOD) [`Self::sub_object_transform`] of a sub-object given by reference (one of
    /// [`Self::cast_ray_sub_objects`]): its bone's transform, or the identity for a skin.
    pub fn sub_transform(&self, s: &SubObject) -> W3dResult<Matrix3D> {
        if s.is_skin() {
            Ok(Matrix3D::IDENTITY)
        } else {
            self.bone_transform(s.bone_index)
        }
    }

    /// (#181 LOD) [`Self::deformed`] of a sub-object given by reference.
    pub fn deformed_sub(&self, s: &SubObject) -> W3dResult<super::meshmdl::Deformed> {
        match (&self.htree, s.is_skin()) {
            (Some(t), true) => s.model.get_deformed_vertices(t),
            _ => Ok((s.model.vertices.clone(), s.model.normals.clone())),
        }
    }

    /// `Get_Deformed_Vertices(dst_vert, dst_norm)` of skin `i` (vertices and
    /// normals in the model's space); its own vertices for a rigid mesh or
    /// without a hierarchy.
    pub fn deformed(&self, i: usize) -> W3dResult<super::meshmdl::Deformed> {
        let s = self.sub_objects.get(i).ok_or(W3dUndefined::IndexOutOfRange)?;
        self.deformed_sub(s)
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
    /// (#181 LOD) A three-LOD HLOD on the two-bone ROBOT: LOD 0 (`MaxScreenSize` 0.5: the hidden quad, the box on a bone
    /// past the hierarchy), LOD 1 (2.0: the box on the arm, the puff emitter), LOD 2 (`NO_MAX_SCREEN_SIZE`: the box on
    /// the arm, the skin); aggregates the box on the arm and one on bone 9.
    ///
    /// Predicted before the run, from hlod.cpp: `CurLod` 1 (LOD 0's 0.5 is below 1, LOD 1's 2.0 is not); `Cost`
    /// `[12, 12, 14]` (the hidden quad not counted; the box's 12 triangles, one pass; the skin's 2), `Value`
    /// `[AT_MIN_LOD, AT_MIN_LOD, (1 - 0.5 / 196) / 14, AT_MAX_LOD]`; drawn: LOD 1's box, then the aggregate box (the one on
    /// bone 9 refused and listed as not drawn); the puff the one emitter; the skeleton map `[None, Some(2)]` (the
    /// skeleton's view lists the top LOD's box and skin, then both aggregate boxes). The skeleton's view
    /// (`RenderModel::create`) is unchanged: four meshes, the last on bone 9. LOD 0's box, had it been drawn, would be
    /// on bone 0 (`Add_Lod_Model`'s clamp): checked through a model whose only LOD is that one.
    #[test]
    fn the_drawn_model_is_the_lod_the_constructor_selects() {
        use super::super::part_buf::{AT_MAX_LOD, AT_MIN_LOD};
        use super::super::w3d_file::NO_MAX_SCREEN_SIZE;
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
        f.extend(EmitterSpec::puff(b"PUFF").to_w3d());
        let lod0: &[(&[u8], u32)] = &[(b"ROBOT.HIDE", 0), (b"ROBOT.BOX", 7)];
        let lod1: &[(&[u8], u32)] = &[(b"ROBOT.BOX", 1), (b"PUFF", 1)];
        let lod2: &[(&[u8], u32)] = &[(b"ROBOT.BOX", 1), (b"ROBOT.SKIN", 0)];
        f.extend(hlod_lods_w3d(b"ROBOT", b"ROBOT", &[(0.5, lod0), (2.0, lod1), (NO_MAX_SCREEN_SIZE, lod2)], &[(b"ROBOT.BOX", 1), (b"ROBOT.BOX", 9)]));
        let low = hlod_lods_w3d(b"LOW", b"ROBOT", &[(NO_MAX_SCREEN_SIZE, lod0)], &[]);
        let fs = fs_with(&[("Art/W3D/robot.w3d", f), ("Art/W3D/low.w3d", low)]);
        let mut am = AssetManager::new_game(W3dFileFactory::default());
        let m = RenderModel::create_drawn(&mut am, &fs, b"ROBOT").unwrap().unwrap();
        let l = m.lod.clone().unwrap();
        assert_eq!((l.lod_count, l.cur_lod, l.get_lod_level(), l.get_lod_count()), (3, 1, 1, 3));
        assert_eq!(l.cost, vec![12.0, 12.0, 14.0]);
        assert_eq!(l.value, vec![AT_MIN_LOD, AT_MIN_LOD, (1.0f32 - 0.5f32 / 196.0f32) * 1.0 * 1.0 / 14.0, AT_MAX_LOD]);
        let drawn: Vec<(&[u8], i32)> = m.sub_objects.iter().map(|s| (&s.name[..], s.bone_index)).collect();
        assert_eq!(drawn, vec![(&b"ROBOT.BOX"[..], 1), (&b"ROBOT.BOX"[..], 1)]);
        assert_eq!(m.emitters.len(), 1);
        assert_eq!(m.not_drawn.len(), 1, "{:?}", m.not_drawn);
        assert_eq!(m.skeleton_index, Some(vec![None, Some(2)]));
        // the skeleton's view, as before
        let k = RenderModel::create(&mut am, &fs, b"ROBOT").unwrap().unwrap();
        let listed: Vec<(&[u8], i32)> = k.sub_objects.iter().map(|s| (&s.name[..], s.bone_index)).collect();
        assert_eq!(listed, vec![(&b"ROBOT.BOX"[..], 1), (&b"ROBOT.SKIN"[..], 0), (&b"ROBOT.BOX"[..], 1), (&b"ROBOT.BOX"[..], 9)]);
        assert_eq!((k.lod.is_none(), k.skeleton_index.is_none(), k.emitters.len()), (true, true, 0));
        // Add_Lod_Model's clamp: the box on bone 7 of a two-bone hierarchy is on bone 0
        let low = RenderModel::create_drawn(&mut am, &fs, b"LOW").unwrap().unwrap();
        let drawn: Vec<(&[u8], i32, bool)> = low.sub_objects.iter().map(|s| (&s.name[..], s.bone_index, s.hidden)).collect();
        assert_eq!(drawn, vec![(&b"ROBOT.HIDE"[..], 0, true), (&b"ROBOT.BOX"[..], 0, false)]);
        assert_eq!(low.skeleton_index, None);
    }

    /// (#181 LOD) `HLodLevels` on its own (`Calculate_Cost_Value_Arrays` at a screen area of 1): no LOD below 1 keeps
    /// `CurLod` 0; every LOD below 1 gives the top; a NaN `MaxScreenSize` stops the scan (`NaN < 1` is false); a size of
    /// exactly 1 is not below 1; `Set_LOD_Level` clamps; a poly count of 46,341 wraps its square as the original's `int`.
    #[test]
    fn the_lod_selection_follows_calculate_cost_value_arrays() {
        let sel = |sizes: &[f32]| HLodLevels::new(sizes, &[]).cur_lod;
        assert_eq!(sel(&[2.0, 5.0]), 0);
        assert_eq!(sel(&[0.0, 0.5, 0.9]), 2);
        assert_eq!(sel(&[0.0, f32::NAN, 0.5]), 1);
        assert_eq!(sel(&[0.5, 1.0, 0.5]), 1);
        assert_eq!(sel(&[f32::from_bits(1.0f32.to_bits() - 1), -1.0, f32::INFINITY]), 2);
        let mut l = HLodLevels::new(&[0.0, 0.0], &[3, 0]);
        assert_eq!((l.cur_lod, l.cost.clone()), (1, vec![3.0, 0.000001]));
        assert_eq!(l.lods[1].benefit_factor, 0.0);
        l.set_lod_level(-4);
        assert_eq!(l.get_lod_level(), 0);
        l.set_lod_level(9);
        assert_eq!(l.get_lod_level(), 1);
        let big = HLodLevels::new(&[super::super::w3d_file::NO_MAX_SCREEN_SIZE], &[46_341]);
        assert_eq!(big.lods[0].benefit_factor, 1.0f32 - 0.5f32 / (46_341i32.wrapping_mul(46_341) as f32));
    }

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
