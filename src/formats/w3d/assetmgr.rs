// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/assetmgr.cpp, assetmgr.h
//   (WW3DAssetManager: the constructor's loader list, Free_Assets,
//   Load_3D_Assets, Load_Prototype, Create_Render_Obj's prototype lookup,
//   Render_Obj_Exists, Get_HAnim, Get_HTree, Register_Prototype_Loader,
//   Find_Prototype_Loader, Add_Prototype, Remove_Prototype, Find_Prototype,
//   the iterators' orders)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htreemgr.cpp, htreemgr.h
//   (HTreeManagerClass: Load_Tree, Get_Tree_ID, Get_Tree, Free_All_Trees)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hanimmgr.cpp, hanimmgr.h
//   (HAnimManagerClass: Load_Anim and its three loaders, Peek_Anim, Add_Anim,
//   Free_All_Anims, Register_Missing, Is_Missing)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/hash.cpp, hash.h
//   (HashTableClass: Add, Find, the iterator's order)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/proto.cpp, nullrobj.cpp,
//   dazzle.cpp (MeshLoaderClass, NullLoaderClass and DazzleLoaderClass's
//   Load_W3D, DazzlePrototypeClass::Load_W3D without the type lookup)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DAssetManager.cpp
//   (W3DAssetManager::Load_3D_Assets(const char *), Get_HAnim,
//   Create_Render_Obj(const char *))
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DFileSystem.cpp
//   (GameFileClass::Set_Name, Is_Available, Open: the game's _TheFileFactory)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp
//   (the two loaders the game registers, Set_WW3D_Load_On_Demand(true))
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, asset manager by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The W3D asset manager's loading side (`WW3DAssetManager`, with the game's
//! `W3DAssetManager` subclass): reading a W3D file chunk by chunk, choosing
//! the loader by the top-level chunk id, registering what loads by name
//! (render-object prototypes, hierarchies, animations), and finding them
//! again by name, loading `<name>.w3d` on demand. GitHub issue #10, step 2.
//! Only loading, registration and lookup: creating render objects from the
//! prototypes, textures, fonts and the exclusion-list freeing are not
//! translated.
//!
//! Compared with upstream's own code in `tests/differential_w3d.rs`
//! (section 12): `assetmgr.cpp`'s functions above (verbatim), `htreemgr.cpp`,
//! `hanimmgr.cpp` and `hash.cpp`, compiled, over the real loaders. The game's
//! parts (`W3DAssetManager`'s shortcut, `GameFileClass`'s path search) and
//! the thin prototype-loader wrappers of `proto.cpp`, `hlod.cpp`,
//! `collect.cpp`, `agg_def.cpp`, `nullrobj.cpp` and `dazzle.cpp` are
//! transcriptions on the C++ side too (their files cannot be compiled
//! there), so for them the comparison checks the Rust against a second
//! reading, not against compiled upstream.
//!
//! # As upstream reads it
//!
//! `Load_3D_Assets(file)` opens each top-level chunk in turn:
//! `W3D_CHUNK_HIERARCHY` goes to `HTreeManagerClass::Load_Tree`;
//! `W3D_CHUNK_ANIMATION`, `W3D_CHUNK_COMPRESSED_ANIMATION` and
//! `W3D_CHUNK_MORPH_ANIMATION` to `HAnimManagerClass::Load_Anim`; anything
//! else to `Load_Prototype`, which asks the first registered loader whose
//! `Chunk_Type` is the chunk id ([`PrototypeLoader`]; the constructor
//! registers mesh, hmodel, collection, box, HLOD, distance LOD, aggregate,
//! null, dazzle, ring, sphere, and the game adds a particle-emitter and a
//! second aggregate loader) and adds the prototype unless one of that name
//! exists. Each chunk is then closed. The result of each load is ignored;
//! `Load_3D_Assets` returns true once the file is open.
//!
//! Names are compared without ASCII case (`stricmp`; `_strlwr` keys for the
//! hierarchies). Prototypes and animations sit in hash tables indexed by
//! `CRC_Stringi(name)` ([`crate::ww::realcrc::crc_stringi`]) masked to 4096
//! and 2048 buckets, each bucket a list with the newest entry first, which a
//! lookup walks from the head. Hierarchies are kept in load order in an
//! array (`TreePtr`) plus a hash table of lower-cased names.
//!
//! On-demand loading (`WW3D_Load_On_Demand`, which the game turns on):
//! - `Create_Render_Obj(name)`: if no prototype `name` exists, load
//!   `<name up to its first '.'>.w3d` (or `<name>.w3d` without a dot), then
//!   `..\<that>` if the first load returns false, and look again.
//! - `Get_HTree(name)`: load `<name>.w3d` (then `..\`), look again.
//! - `Get_HAnim("H.A")`: unless the name is already registered as missing,
//!   load `<A>.w3d` (the part after the *first* dot; then `..\`), look again,
//!   and register the name as missing if it is still not there. A name
//!   without a dot is not looked for (an assertion in debug builds).
//! - The animation loaders ask `Get_HTree` for their hierarchy and the morph
//!   loader asks `Get_HAnim` for each channel's pose, so loading a file can
//!   load others, recursively.
//!
//! The game's `W3DAssetManager::Load_3D_Assets(filename)` first strips the
//! extension (from the last '.') and returns true without reading anything
//! when a prototype of that name exists. A file name becomes a path as
//! `GameFileClass::Set_Name` makes it ([`W3dFileFactory`]): for a `.w3d`
//! name, `Data/<language>/Art/W3D/<name>`, then `Art/W3D/<name>`, then
//! `../TestArt/<name>` (`LOAD_TEST_ASSETS` is defined in the release), then
//! `<user data>W3D/<name>`, the first that `TheFileSystem` has. Upstream
//! reads through `_TheFileFactory`, which the game sets to `W3DFileSystem`
//! (`GameFileClass` over `TheFileSystem`); here the files come from a
//! [`FileSystem`] through [`W3dFileFactory`].
//!
//! # Upstream quirks that are kept
//!
//! - **The first prototype, hierarchy or animation of a name wins** when
//!   loaded from a file (the later one is dropped), but **the last one
//!   wins** through [`AssetManager::add_prototype`] and
//!   [`AssetManager::add_anim`], which do not check (both entries stay; the
//!   newer one is at the head of its hash bucket).
//! - `"NULL"` (any case) always finds the built-in null prototype, so a null
//!   object (or any prototype) named `null` loaded from a file collides with
//!   it and is dropped.
//! - The game's shortcut skips a file whose base name is a prototype, even
//!   when the file was asked for because a *hierarchy* or *animation* of
//!   another name was missing: `Get_HAnim("SKL.FOO")` with a prototype
//!   `FOO` loaded does not read `FOO.w3d` and registers `SKL.FOO` as missing.
//! - The missing-animation table is never cleared by
//!   [`AssetManager::free_assets`] (upstream resets it only in the
//!   destructor), so an animation once missing stays missing after a free.
//! - A failed load inside a chunk leaves the chunks it opened open (see
//!   [`super::htree`]); the loop's `Close_Chunk` then closes the inner one
//!   and the following chunks are read *from inside* that top-level chunk,
//!   each dispatched by its own id like a top-level chunk.
//! - A chunk with no loader is skipped; so are hierarchies and animations
//!   whose load fails, and the result of `Load_3D_Assets` is true however
//!   much of the file loaded.
//! - Names run on past their fields where upstream's do: a 16-character
//!   hierarchy name continues into `NumPivots`' bytes (to its first 0 byte),
//!   a 32-character box, sphere or ring name into the following members of
//!   its definition struct.
//! - `Free_Assets` keeps the registered loaders and the on-demand setting.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: a registered name that does not end inside its object
//! (a collection name of 16 characters, which runs into a `DynamicVectorClass`;
//! a null object's 32-character name, which is the last member; a box,
//! sphere or ring name with no 0 before the end of its struct; a 16-character
//! hierarchy name whose `NumPivots` has no 0 byte; an animation's hierarchy
//! name of 16 characters, passed to `Get_HTree` as a C string:
//! [`W3dUndefined::StringOverrun`]); an aggregate with no header
//! (`Get_Name()` is null: [`W3dUndefined::NullPointer`]); a dazzle name chunk
//! whose bytes fill their buffer without a 0 (`StringClass::Get_Buffer(n)`
//! has no room for a terminator); a file name that overflows a `MAX_PATH`
//! (260) buffer (`sprintf`, `lstrcpyn`, `strcpy`) or the game's 512-byte base
//! name ([`W3dUndefined::StringOverflow`]); more than 16000 hierarchies
//! ([`W3dUndefined::IndexOutOfRange`]); a file larger than an `int` can size.
//! [`W3dError::NonTerminating`]: an on-demand load that reaches the same file
//! again with nothing loaded in between (upstream recurses until its stack
//! overflows).
//!
//! [`W3dError::Unsupported`]: a chunk whose loader is not translated: the
//! obsolete hierarchical model (`W3D_CHUNK_HMODEL`, `hmdldef.cpp`), the
//! distance LOD (`W3D_CHUNK_LODMODEL`, `distlod.cpp`) and the particle
//! emitter the game registers (`W3D_CHUNK_EMITTER`, `part_ldr.cpp`).
//!
//! # Representation
//!
//! [`AssetManager`] owns the prototypes (an enum over the loaders' outputs,
//! [`Prototype`]), the hierarchies ([`HTree`]) and the animations
//! ([`HAnim`]). The hash tables are not kept as tables: since equal names
//! fall in the same bucket and a bucket is newest-first, a lookup is "the
//! newest entry of that name", which the entries' insertion serials give;
//! the bucket only matters for iteration order
//! ([`AssetManager::anim_names`]), computed from the CRC. Reference counts
//! are not kept (nothing here releases an asset while it is in use). The
//! dazzle's type is kept as its name (`DazzleRenderObjClass::Get_Type_ID`
//! needs the dazzle INI, which is the renderer's).

use super::agg_def::{self, AggregateDef};
use super::boxrobj::{self, BoxPrototype};
use super::collect::{self, CollectionDef};
use super::hanim::{HCompressedAnim, HRawAnim};
use super::hlod::{self, HLodDef};
use super::hmorphanim::{self, HMorphAnim, MorphChain, PoseLookup};
use super::htree::{self, HTree};
use super::mesh::{self, Mesh, MeshLoadOptions};
use super::ringobj::{self, RingPrototype};
use super::sphereobj::{self, SpherePrototype};
use super::w3d_file::*;
use super::{W3dError, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::common::file_system::{FileSystem, LocalDevice};
use crate::ww::chunkio::{ChunkLoad, GameFile};
use crate::ww::matrix3d::Matrix3D;
use crate::ww::quat::Quaternion;
use crate::ww::realcrc::crc_stringi;
use crate::ww::vector3::Vector3;

/// `MAX_PATH` (and `_MAX_PATH`).
pub const MAX_PATH: usize = 260;
/// `PROTOTYPE_HASH_TABLE_SIZE`.
pub const PROTOTYPE_HASH_TABLE_SIZE: u32 = 4096;
/// The size of `HAnimManagerClass`'s `AnimPtrTable` and `MissingAnimTable`.
pub const ANIM_HASH_TABLE_SIZE: u32 = 2048;
/// `HTreeManagerClass::MAX_TREES`.
pub const MAX_TREES: usize = 16000;
/// `RenderObjClass::CLASSID_MESH`.
pub const CLASSID_MESH: u32 = 0;
/// `RenderObjClass::CLASSID_NULL`.
pub const CLASSID_NULL: u32 = 22;
/// `RenderObjClass::CLASSID_COLLECTION`.
pub const CLASSID_COLLECTION: u32 = 23;
/// `RenderObjClass::CLASSID_DAZZLE`.
pub const CLASSID_DAZZLE: u32 = 31;
/// `sizeof(StringClass::_HEADER)`.
const STRING_HEADER_SIZE: u64 = 8;
/// The game's `basename[512]` in `W3DAssetManager::Load_3D_Assets`.
const GAME_BASENAME_LEN: usize = 512;

/// `stricmp(a, b) == 0` in the C locale: ASCII case folded.
pub fn stricmp_eq(a: &[u8], b: &[u8]) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// The bytes of a C string held in `bytes` (its buffer and whatever memory
/// of the same object follows it), up to the first 0; `StringOverrun` when
/// there is none (the read leaves the object).
fn c_str_in(bytes: &[u8]) -> W3dResult<&[u8]> {
    match bytes.iter().position(|&c| c == 0) {
        Some(n) => Ok(&bytes[..n]),
        None => Err(W3dUndefined::StringOverrun.into()),
    }
}

/// A name as passed to the lookups: a C string's bytes, cut at a 0 if the
/// caller's slice has one.
fn c_name(name: &[u8]) -> &[u8] {
    &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())]
}

/// A prototype loader (`PrototypeLoaderClass`): the loaders upstream
/// registers, by the chunk they read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrototypeLoader {
    /// `_MeshLoader` (`W3D_CHUNK_MESH`).
    Mesh,
    /// `_HModelLoader` (`W3D_CHUNK_HMODEL`; not translated).
    HModel,
    /// `_CollectionLoader` (`W3D_CHUNK_COLLECTION`).
    Collection,
    /// `_BoxLoader` (`W3D_CHUNK_BOX`).
    Box,
    /// `_HLodLoader` (`W3D_CHUNK_HLOD`).
    HLod,
    /// `_DistLODLoader` (`W3D_CHUNK_LODMODEL`; not translated).
    DistLod,
    /// `_AggregateLoader` (`W3D_CHUNK_AGGREGATE`).
    Aggregate,
    /// `_NullLoader` (`W3D_CHUNK_NULL_OBJECT`).
    Null,
    /// `_DazzleLoader` (`W3D_CHUNK_DAZZLE`).
    Dazzle,
    /// `_RingLoader` (`W3D_CHUNK_RING`).
    Ring,
    /// `_SphereLoader` (`W3D_CHUNK_SPHERE`).
    Sphere,
    /// `_ParticleEmitterLoader` (`W3D_CHUNK_EMITTER`; registered by the
    /// game; not translated).
    ParticleEmitter,
}

impl PrototypeLoader {
    /// `Chunk_Type()`.
    pub fn chunk_type(self) -> u32 {
        match self {
            PrototypeLoader::Mesh => W3D_CHUNK_MESH,
            PrototypeLoader::HModel => W3D_CHUNK_HMODEL,
            PrototypeLoader::Collection => W3D_CHUNK_COLLECTION,
            PrototypeLoader::Box => W3D_CHUNK_BOX,
            PrototypeLoader::HLod => W3D_CHUNK_HLOD,
            PrototypeLoader::DistLod => W3D_CHUNK_LODMODEL,
            PrototypeLoader::Aggregate => W3D_CHUNK_AGGREGATE,
            PrototypeLoader::Null => W3D_CHUNK_NULL_OBJECT,
            PrototypeLoader::Dazzle => W3D_CHUNK_DAZZLE,
            PrototypeLoader::Ring => W3D_CHUNK_RING,
            PrototypeLoader::Sphere => W3D_CHUNK_SPHERE,
            PrototypeLoader::ParticleEmitter => W3D_CHUNK_EMITTER,
        }
    }
}

/// The loaders `WW3DAssetManager()` registers, in order (`USE_WWSHADE` is
/// not defined, so the mesh loader is first; `SHD_REG_LOADER` is empty).
pub const WW3D_LOADERS: [PrototypeLoader; 11] = [
    PrototypeLoader::Mesh,
    PrototypeLoader::HModel,
    PrototypeLoader::Collection,
    PrototypeLoader::Box,
    PrototypeLoader::HLod,
    PrototypeLoader::DistLod,
    PrototypeLoader::Aggregate,
    PrototypeLoader::Null,
    PrototypeLoader::Dazzle,
    PrototypeLoader::Ring,
    PrototypeLoader::Sphere,
];

/// `NullPrototypeClass`: its `Definition`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NullPrototype {
    pub definition: W3dNullObjectStruct,
}

impl NullPrototype {
    /// `NullPrototypeClass()`: the built-in `"NULL"` (upstream leaves the
    /// other members uninitialised; here they are 0).
    pub fn builtin() -> Self {
        let mut d = W3dNullObjectStruct::from_bytes(&[0; W3dNullObjectStruct::SIZE]);
        d.name[..4].copy_from_slice(b"NULL");
        NullPrototype { definition: d }
    }
}

/// `DazzlePrototypeClass`: its name and the name of its dazzle type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DazzlePrototype {
    /// `Name` (the bytes before its terminator).
    pub name: Vec<u8>,
    /// The type name `Load_W3D` reads (resolved upstream with
    /// `DazzleRenderObjClass::Get_Type_ID`, not here).
    pub type_name: Vec<u8>,
}

/// A render-object prototype (`PrototypeClass`), by the loader that made it.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Prototype {
    /// `PrimitivePrototypeClass` over a `MeshClass`.
    Mesh(Mesh),
    /// `HLodPrototypeClass`.
    HLod(HLodDef),
    /// `CollectionPrototypeClass`.
    Collection(CollectionDef),
    /// `BoxPrototypeClass`.
    Box(BoxPrototype),
    /// `AggregatePrototypeClass`.
    Aggregate(AggregateDef),
    /// `NullPrototypeClass`.
    Null(NullPrototype),
    /// `DazzlePrototypeClass`.
    Dazzle(DazzlePrototype),
    /// `SpherePrototypeClass`.
    Sphere(SpherePrototype),
    /// `RingPrototypeClass`.
    Ring(RingPrototype),
}

impl Prototype {
    /// `Get_Name()` as the C string upstream reads (see the module docs for
    /// names that run on).
    pub fn name(&self) -> W3dResult<Vec<u8>> {
        Ok(match self {
            Prototype::Mesh(m) => c_name(&m.name).to_vec(),
            Prototype::HLod(h) => c_name(h.name()).to_vec(),
            Prototype::Collection(c) => c_str_in(&c.name)?.to_vec(),
            Prototype::Box(b) => c_str_in(&b.definition.to_bytes()[8..])?.to_vec(),
            Prototype::Aggregate(a) => match &a.name {
                Some(n) => c_name(n).to_vec(),
                None => return Err(W3dUndefined::NullPointer.into()),
            },
            Prototype::Null(n) => c_str_in(&n.definition.name)?.to_vec(),
            Prototype::Dazzle(d) => d.name.clone(),
            Prototype::Sphere(s) => c_str_in(&s.definition.to_bytes()[8..])?.to_vec(),
            Prototype::Ring(r) => c_str_in(&r.definition.to_bytes()[8..])?.to_vec(),
        })
    }

    /// `Get_Class_ID()`.
    pub fn class_id(&self) -> u32 {
        match self {
            Prototype::Mesh(_) => CLASSID_MESH,
            Prototype::HLod(_) => agg_def::CLASSID_HLOD,
            Prototype::Collection(_) => CLASSID_COLLECTION,
            Prototype::Box(b) => b.class_id(),
            Prototype::Aggregate(a) => a.class_id(),
            Prototype::Null(_) => CLASSID_NULL,
            Prototype::Dazzle(_) => CLASSID_DAZZLE,
            Prototype::Sphere(s) => s.class_id(),
            Prototype::Ring(r) => r.class_id(),
        }
    }
}

/// A hierarchy animation (`HAnimClass`), by its loader.
#[derive(Clone, Debug, PartialEq)]
pub enum HAnim {
    /// `HRawAnimClass` (`W3D_CHUNK_ANIMATION`).
    Raw(HRawAnim),
    /// `HCompressedAnimClass` (`W3D_CHUNK_COMPRESSED_ANIMATION`).
    Compressed(HCompressedAnim),
    /// `HMorphAnimClass` (`W3D_CHUNK_MORPH_ANIMATION`).
    Morph(HMorphAnim),
}

impl HAnim {
    /// `Get_Name()` (`Get_Key()` in the hash table): `"<hierarchy>.<anim>"`.
    pub fn name(&self) -> &[u8] {
        let n: &[u8; 2 * W3D_NAME_LEN] = match self {
            HAnim::Raw(a) => &a.name,
            HAnim::Compressed(a) => &a.name,
            HAnim::Morph(a) => &a.name,
        };
        c_name(n)
    }

    /// `Get_Num_Pivots()`.
    pub fn get_num_pivots(&self) -> i32 {
        match self {
            HAnim::Raw(a) => a.get_num_pivots(),
            HAnim::Compressed(a) => a.get_num_pivots(),
            HAnim::Morph(a) => a.get_num_pivots(),
        }
    }

    /// `Get_Num_Frames()`.
    pub fn get_num_frames(&self) -> i32 {
        match self {
            HAnim::Raw(a) => a.get_num_frames(),
            HAnim::Compressed(a) => a.get_num_frames(),
            HAnim::Morph(a) => a.get_num_frames(),
        }
    }

    pub(crate) fn translation_in<P: PoseLookup>(&self, poses: &P, pividx: i32, frame: f32, chain: &mut MorphChain) -> W3dResult<Vector3> {
        match self {
            HAnim::Raw(a) => a.get_translation(pividx, frame),
            HAnim::Compressed(a) => a.get_translation(pividx, frame),
            HAnim::Morph(a) => a.translation_in(poses, pividx, frame, chain),
        }
    }

    pub(crate) fn orientation_in<P: PoseLookup>(
        &self,
        poses: &P,
        pividx: i32,
        frame: f32,
        chain: &mut MorphChain,
    ) -> W3dResult<Option<Quaternion>> {
        match self {
            HAnim::Raw(a) => a.get_orientation(pividx, frame).map(Some),
            HAnim::Compressed(a) => a.get_orientation(pividx, frame),
            HAnim::Morph(a) => a.orientation_in(poses, pividx, frame, chain).map(Some),
        }
    }

    pub(crate) fn transform_in<P: PoseLookup>(
        &self,
        poses: &P,
        pividx: i32,
        frame: f32,
        chain: &mut MorphChain,
    ) -> W3dResult<Option<Matrix3D>> {
        match self {
            HAnim::Raw(a) => a.get_transform(pividx, frame).map(Some),
            HAnim::Compressed(a) => a.get_transform(pividx, frame),
            HAnim::Morph(a) => a.transform_in(poses, pividx, frame, chain).map(Some),
        }
    }

    /// The virtual `Get_Translation(trans, pividx, frame)`.
    pub fn get_translation<P: PoseLookup>(&self, poses: &P, pividx: i32, frame: f32) -> W3dResult<Vector3> {
        self.translation_in(poses, pividx, frame, &mut MorphChain::new())
    }

    /// The virtual `Get_Orientation(q, pividx, frame)`: `None` where upstream
    /// leaves `q` as it was (a compressed animation of an unknown flavor).
    pub fn get_orientation<P: PoseLookup>(&self, poses: &P, pividx: i32, frame: f32) -> W3dResult<Option<Quaternion>> {
        self.orientation_in(poses, pividx, frame, &mut MorphChain::new())
    }

    /// The virtual `Get_Transform(mtx, pividx, frame)`: `None` where upstream
    /// leaves `mtx` as it was.
    pub fn get_transform<P: PoseLookup>(&self, poses: &P, pividx: i32, frame: f32) -> W3dResult<Option<Matrix3D>> {
        self.transform_in(poses, pividx, frame, &mut MorphChain::new())
    }

    /// The virtual `Get_Visibility(pividx, frame)`.
    pub fn get_visibility(&self, pividx: i32, frame: f32) -> W3dResult<bool> {
        match self {
            HAnim::Raw(a) => a.get_visibility(pividx, frame),
            HAnim::Compressed(a) => a.get_visibility(pividx, frame),
            HAnim::Morph(a) => Ok(a.get_visibility(pividx, frame)),
        }
    }
}

/// The asset manager gives a morph animation its poses: the newest
/// animation of the name (`Peek_Anim`; upstream resolved the name with
/// `Get_HAnim` when the morph animation was loaded, see
/// [`super::hmorphanim`]).
impl PoseLookup for AssetManager {
    fn pose(&self, name: &[u8]) -> Option<&HAnim> {
        self.peek_anim(name)
    }
}

/// Which asset manager: upstream's library class, or the game's subclass
/// (`W3DAssetManager`, which overrides `Load_3D_Assets(const char *)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagerKind {
    Ww3d,
    Game,
}

/// The game's `_TheFileFactory` (`W3DFileSystem`): `GameFileClass`'s path
/// search over `TheFileSystem`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct W3dFileFactory {
    /// `GetRegistryLanguage()` (`"english"` unless the registry says
    /// otherwise).
    pub language: Vec<u8>,
    /// `TheGlobalData->getPath_UserData()` (with its trailing separator), or
    /// `None` when there is no `TheGlobalData`.
    pub user_data_path: Option<Vec<u8>>,
}

impl Default for W3dFileFactory {
    fn default() -> Self {
        W3dFileFactory { language: b"english".to_vec(), user_data_path: None }
    }
}

/// `GameFileType`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GameFileType {
    Unknown,
    W3d,
    Tga,
    Dds,
}

impl W3dFileFactory {
    /// `GameFileClass::Set_Name(filename)` and `Is_Available()`: the path the
    /// file is opened from, or `None` when no candidate exists.
    pub fn resolve<D: LocalDevice>(&self, fs: &FileSystem<D>, filename: &[u8]) -> W3dResult<Option<Vec<u8>>> {
        let filename = c_name(filename);
        // strcpy(name, filename) into char[_MAX_PATH]
        if filename.len() + 1 > MAX_PATH {
            return Err(W3dUndefined::StringOverflow.into());
        }
        // the extension: the last '.' at index 1 or more, within 32 bytes of the end
        let mut extension: &[u8] = b"";
        let mut i = filename.len() as isize - 1;
        let mut ext_len = 1;
        while i > 0 && ext_len < 32 {
            if filename[i as usize] == b'.' {
                extension = &filename[i as usize..];
                break;
            }
            i -= 1;
            ext_len += 1;
        }
        // (the copy with spaces stripped is computed and never used)
        let file_type = if stricmp_eq(extension, b".w3d") {
            GameFileType::W3d
        } else if stricmp_eq(extension, b".tga") {
            GameFileType::Tga
        } else if stricmp_eq(extension, b".dds") {
            GameFileType::Dds
        } else {
            GameFileType::Unknown
        };
        let is_image = matches!(file_type, GameFileType::Tga | GameFileType::Dds);
        let join = |a: &[u8], b: &[u8]| -> W3dResult<Vec<u8>> {
            let mut p = a.to_vec();
            p.extend_from_slice(b);
            if p.len() + 1 > MAX_PATH {
                return Err(W3dUndefined::StringOverflow.into());
            }
            Ok(p)
        };
        let lang = |dir: &[u8]| -> W3dResult<Vec<u8>> {
            let mut prefix = b"Data/".to_vec();
            prefix.extend_from_slice(&self.language);
            prefix.extend_from_slice(dir);
            join(&prefix, filename)
        };
        // m_filePath starts empty
        let mut path: Vec<u8> = Vec::new();
        if file_type == GameFileType::W3d {
            path = lang(b"/Art/W3D/")?;
        }
        if is_image {
            path = lang(b"/Art/Textures/")?;
        }
        let mut exists = fs.does_file_exist(&path);
        if !exists {
            path = match file_type {
                GameFileType::W3d => join(b"Art/W3D/", filename)?,
                _ if is_image => join(b"Art/Textures/", filename)?,
                _ => filename.to_vec(),
            };
            exists = fs.does_file_exist(&path);
        }
        // LOAD_TEST_ASSETS
        if !exists {
            if file_type == GameFileType::W3d || is_image {
                path = join(b"../TestArt/", filename)?;
            }
            exists = fs.does_file_exist(&path);
        }
        if let Some(user) = &self.user_data_path {
            if !exists {
                if file_type == GameFileType::W3d {
                    path = join(&[user.as_slice(), b"W3D/"].concat(), filename)?;
                }
                if is_image {
                    path = join(&[user.as_slice(), b"Textures/"].concat(), filename)?;
                }
                exists = fs.does_file_exist(&path);
            }
            if !exists {
                if file_type == GameFileType::Tga {
                    path = join(&[user.as_slice(), b"MapPreviews/"].concat(), filename)?;
                }
                exists = fs.does_file_exist(&path);
            }
        }
        Ok(exists.then_some(path))
    }
}

/// A hash-table entry: the value, its name and when it was added.
#[derive(Clone, Debug, PartialEq)]
struct Entry<T> {
    serial: u64,
    name: Vec<u8>,
    value: T,
}

/// The number of entries of each kind: the manager's state while loading,
/// which only grows.
type LoadState = [usize; 4];

/// `WW3DAssetManager` (or the game's `W3DAssetManager`): the loading side.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetManager {
    /// Which class (see [`ManagerKind`]).
    pub kind: ManagerKind,
    /// `WW3D_Load_On_Demand`.
    pub load_on_demand: bool,
    /// What the mesh loader reads from `WW3D` (the pre-lit mode).
    pub mesh_options: MeshLoadOptions,
    /// `_TheFileFactory`.
    pub file_factory: W3dFileFactory,
    /// `PrototypeLoaders`.
    loaders: Vec<PrototypeLoader>,
    /// `Prototypes`, in order (the hash table is derived, see the module
    /// docs).
    prototypes: Vec<Entry<Prototype>>,
    /// `HTreeManagerClass::TreePtr[0..NumTrees]`, with their names.
    trees: Vec<(Vec<u8>, HTree)>,
    /// `HAnimManagerClass::AnimPtrTable`.
    anims: Vec<Entry<HAnim>>,
    /// `HAnimManagerClass::MissingAnimTable`.
    missing: Vec<Entry<()>>,
    serial: u64,
    /// `_NullPrototype`.
    null_prototype: Prototype,
    /// An error inside a hierarchy lookup made for a loader.
    pending: Option<W3dError>,
    /// The files being loaded on demand, outermost first, with the state
    /// each started from.
    loading: Vec<(Vec<u8>, LoadState)>,
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetManager {
    /// `WW3DAssetManager()`: the library's loaders, on-demand loading off.
    pub fn new() -> Self {
        AssetManager {
            kind: ManagerKind::Ww3d,
            load_on_demand: false,
            mesh_options: MeshLoadOptions::default(),
            file_factory: W3dFileFactory::default(),
            loaders: WW3D_LOADERS.to_vec(),
            prototypes: Vec::new(),
            trees: Vec::new(),
            anims: Vec::new(),
            missing: Vec::new(),
            serial: 0,
            null_prototype: Prototype::Null(NullPrototype::builtin()),
            pending: None,
            loading: Vec::new(),
        }
    }

    /// The game's asset manager as `W3DDisplay::init` sets it up: `NEW
    /// W3DAssetManager`, `Register_Prototype_Loader(&_ParticleEmitterLoader)`,
    /// `Register_Prototype_Loader(&_AggregateLoader)` (a second aggregate
    /// loader, never chosen: the first one matches first), and
    /// `Set_WW3D_Load_On_Demand(true)`.
    pub fn new_game(file_factory: W3dFileFactory) -> Self {
        let mut m = Self::new();
        m.kind = ManagerKind::Game;
        m.file_factory = file_factory;
        m.register_prototype_loader(PrototypeLoader::ParticleEmitter);
        m.register_prototype_loader(PrototypeLoader::Aggregate);
        m.load_on_demand = true;
        m
    }

    fn next_serial(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    fn state(&self) -> LoadState {
        [self.prototypes.len(), self.trees.len(), self.anims.len(), self.missing.len()]
    }

    /// `Register_Prototype_Loader(loader)`: appended to the list.
    pub fn register_prototype_loader(&mut self, loader: PrototypeLoader) {
        self.loaders.push(loader);
    }

    /// The registered loaders, in order.
    pub fn loaders(&self) -> &[PrototypeLoader] {
        &self.loaders
    }

    /// `Find_Prototype_Loader(chunk_id)`: the first loader of that chunk.
    pub fn find_prototype_loader(&self, chunk_id: u32) -> Option<PrototypeLoader> {
        self.loaders.iter().copied().find(|l| l.chunk_type() == chunk_id)
    }

    // ---- prototypes --------------------------------------------------------

    /// `Add_Prototype(newproto)`: no check for a duplicate name; the new one
    /// shadows any older one of that name.
    pub fn add_prototype(&mut self, proto: Prototype) -> W3dResult<()> {
        // CRC_Stringi(newproto->Get_Name())
        let name = proto.name()?;
        let serial = self.next_serial();
        self.prototypes.push(Entry { serial, name, value: proto });
        Ok(())
    }

    fn find_prototype_index(&self, name: &[u8]) -> Option<usize> {
        let mut best: Option<(u64, usize)> = None;
        for (i, e) in self.prototypes.iter().enumerate() {
            if stricmp_eq(&e.name, name) && best.is_none_or(|(s, _)| e.serial > s) {
                best = Some((e.serial, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// `Find_Prototype(name)`: the built-in null prototype for `"NULL"` (any
    /// case), else the newest prototype of that name.
    pub fn find_prototype(&self, name: &[u8]) -> Option<&Prototype> {
        let name = c_name(name);
        if stricmp_eq(name, b"NULL") {
            return Some(&self.null_prototype);
        }
        self.find_prototype_index(name).map(|i| &self.prototypes[i].value)
    }

    /// `Render_Obj_Exists(name)`.
    pub fn render_obj_exists(&self, name: &[u8]) -> bool {
        self.find_prototype(name).is_some()
    }

    /// `Remove_Prototype(name)`: removes the prototype `Find_Prototype`
    /// finds. Asking for `"NULL"` would delete the static null prototype
    /// (undefined upstream).
    pub fn remove_prototype(&mut self, name: &[u8]) -> W3dResult<bool> {
        let name = c_name(name);
        if stricmp_eq(name, b"NULL") {
            return Err(W3dUndefined::PointerOutOfRange.into());
        }
        match self.find_prototype_index(name) {
            Some(i) => {
                self.prototypes.remove(i);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// The prototypes in `Prototypes` order (`RObjIterator`).
    pub fn prototypes(&self) -> impl Iterator<Item = (&[u8], &Prototype)> {
        self.prototypes.iter().map(|e| (e.name.as_slice(), &e.value))
    }

    // ---- hierarchies -------------------------------------------------------

    /// `HTreeManagerClass::Get_Tree_ID(name)`.
    pub fn get_tree_id(&self, name: &[u8]) -> Option<usize> {
        let name = c_name(name);
        self.trees.iter().position(|(n, _)| stricmp_eq(n, name))
    }

    /// `HTreeManagerClass::Get_Tree(name)` (no on-demand loading).
    pub fn peek_htree(&self, name: &[u8]) -> Option<&HTree> {
        self.get_tree_id(name).map(|i| &self.trees[i].1)
    }

    /// The hierarchies in load order (`HTreeIterator`), with their names.
    pub fn htrees(&self) -> impl Iterator<Item = (&[u8], &HTree)> {
        self.trees.iter().map(|(n, t)| (n.as_slice(), t))
    }

    /// The name `Get_Name()` returns: 16 bytes run on into `NumPivots`.
    fn htree_name(t: &HTree) -> W3dResult<Vec<u8>> {
        let mut b = t.name.to_vec();
        b.extend_from_slice(&t.num_pivots.to_le_bytes());
        Ok(c_str_in(&b)?.to_vec())
    }

    /// `HTreeManagerClass::Load_Tree(cload)`: 0 if the tree was added, 1 if
    /// its load failed or its name exists.
    fn load_tree(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<i32> {
        let Some(tree) = htree::load_w3d(cload)? else { return Ok(1) };
        let name = Self::htree_name(&tree)?;
        if self.get_tree_id(&name).is_some() {
            return Ok(1);
        }
        if self.trees.len() >= MAX_TREES {
            return Err(W3dUndefined::IndexOutOfRange.into());
        }
        self.trees.push((name, tree));
        Ok(0)
    }

    // ---- animations --------------------------------------------------------

    fn peek_anim_index(&self, name: &[u8]) -> Option<usize> {
        let mut best: Option<(u64, usize)> = None;
        for (i, e) in self.anims.iter().enumerate() {
            if stricmp_eq(&e.name, name) && best.is_none_or(|(s, _)| e.serial > s) {
                best = Some((e.serial, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// `HAnimManagerClass::Peek_Anim(name)`: the newest animation of that
    /// name (no on-demand loading).
    pub fn peek_anim(&self, name: &[u8]) -> Option<&HAnim> {
        self.peek_anim_index(c_name(name)).map(|i| &self.anims[i].value)
    }

    /// `Add_Anim(new_anim)`: added without a check, shadowing any older one.
    pub fn add_anim(&mut self, anim: HAnim) {
        let serial = self.next_serial();
        let name = anim.name().to_vec();
        self.anims.push(Entry { serial, name, value: anim });
    }

    /// `HAnimManagerClass::Is_Missing(name)`.
    pub fn is_missing(&self, name: &[u8]) -> bool {
        let name = c_name(name);
        self.missing.iter().any(|e| stricmp_eq(&e.name, name))
    }

    /// `HAnimManagerClass::Register_Missing(name)`.
    pub fn register_missing(&mut self, name: &[u8]) {
        let serial = self.next_serial();
        self.missing.push(Entry { serial, name: c_name(name).to_vec(), value: () });
    }

    /// `HAnimManagerClass::Reset_Missing()` (upstream: the destructor only).
    pub fn reset_missing(&mut self) {
        self.missing.clear();
    }

    /// The animation names in `HAnimManagerIterator` order: bucket
    /// `CRC_Stringi(name) & 2047` ascending, the newest first in a bucket.
    pub fn anim_names(&self) -> Vec<&[u8]> {
        let mut v: Vec<(u32, u64, &[u8])> = self
            .anims
            .iter()
            .map(|e| (crc_stringi(&e.name, 0) & (ANIM_HASH_TABLE_SIZE - 1), e.serial, e.name.as_slice()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        v.into_iter().map(|(_, _, n)| n).collect()
    }

    /// The hierarchy lookup the animation loaders make
    /// (`WW3DAssetManager::Get_Instance()->Get_HTree(HierarchyName)`):
    /// errors are kept for the caller in `pending`.
    fn htree_for_anim<D: LocalDevice>(&mut self, fs: &FileSystem<D>, hierarchy_name: &[u8; W3D_NAME_LEN]) -> Option<i32> {
        if self.pending.is_some() {
            return None;
        }
        let name = match c_str_in(hierarchy_name) {
            Ok(n) => n.to_vec(),
            Err(e) => {
                self.pending = Some(e);
                return None;
            }
        };
        match self.get_htree_index(fs, &name) {
            Ok(i) => i.map(|i| self.trees[i].1.num_pivots),
            Err(e) => {
                self.pending = Some(e);
                None
            }
        }
    }

    /// `HAnimManagerClass::Load_Anim(cload)`: 0 if the animation was added
    /// (or the chunk is none of the three), 1 if its load failed or its
    /// name exists.
    fn load_anim<D: LocalDevice>(&mut self, fs: &FileSystem<D>, cload: &mut ChunkLoad<'_>) -> W3dResult<i32> {
        let id = cload.cur_chunk_id()?;
        let loaded = {
            let mut lookup = |n: &[u8; W3D_NAME_LEN]| self.htree_for_anim(fs, n);
            match id {
                W3D_CHUNK_ANIMATION => HRawAnim::load_w3d(cload, &mut lookup).map(|a| a.map(HAnim::Raw)),
                W3D_CHUNK_COMPRESSED_ANIMATION => HCompressedAnim::load_w3d(cload, &mut lookup).map(|a| a.map(HAnim::Compressed)),
                W3D_CHUNK_MORPH_ANIMATION => hmorphanim::load_w3d(cload, &mut lookup).map(|a| a.map(HAnim::Morph)),
                _ => return Ok(0),
            }
        };
        if let Some(e) = self.pending.take() {
            return Err(e);
        }
        let Some(anim) = loaded? else { return Ok(1) };
        // read_channel's Get_HAnim(anim_name) of each pose, in channel order
        // (upstream makes them during the load; nothing the load reads
        // afterwards depends on them)
        if let HAnim::Morph(m) = &anim {
            for c in &m.channels {
                if let Some(pose) = &c.pose_name {
                    self.get_hanim(fs, pose)?;
                }
            }
        }
        if self.peek_anim_index(anim.name()).is_some() {
            return Ok(1);
        }
        self.add_anim(anim);
        Ok(0)
    }

    // ---- loading -----------------------------------------------------------

    /// `Load_Prototype(cload)`: whether a new prototype was added.
    fn load_prototype(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        let chunk_id = cload.cur_chunk_id()?;
        let Some(loader) = self.find_prototype_loader(chunk_id) else {
            // "Unknown chunk type encountered!"
            return Ok(false);
        };
        let proto = match loader {
            PrototypeLoader::Mesh => mesh::load_w3d(cload, &self.mesh_options)?.map(Prototype::Mesh),
            PrototypeLoader::Collection => collect::load_w3d(cload)?.map(Prototype::Collection),
            PrototypeLoader::Box => Some(Prototype::Box(boxrobj::load_w3d(cload)?)),
            PrototypeLoader::HLod => hlod::load_w3d(cload)?.map(Prototype::HLod),
            PrototypeLoader::Aggregate => agg_def::load_w3d(cload)?.map(Prototype::Aggregate),
            PrototypeLoader::Null => Some(Prototype::Null(load_null(cload)?)),
            PrototypeLoader::Dazzle => Some(Prototype::Dazzle(load_dazzle(cload)?)),
            PrototypeLoader::Ring => Some(Prototype::Ring(ringobj::load_w3d(cload)?)),
            PrototypeLoader::Sphere => Some(Prototype::Sphere(sphereobj::load_w3d(cload)?)),
            PrototypeLoader::HModel => return Err(W3dError::Unsupported("W3D_CHUNK_HMODEL (hmdldef.cpp)")),
            PrototypeLoader::DistLod => return Err(W3dError::Unsupported("W3D_CHUNK_LODMODEL (distlod.cpp)")),
            PrototypeLoader::ParticleEmitter => return Err(W3dError::Unsupported("W3D_CHUNK_EMITTER (part_ldr.cpp)")),
        };
        let Some(proto) = proto else {
            // "Could not generate prototype!"
            return Ok(false);
        };
        let name = proto.name()?;
        if self.render_obj_exists(&name) {
            // "Render Object Name Collision"
            return Ok(false);
        }
        self.add_prototype(proto)?;
        Ok(true)
    }

    /// `Load_3D_Assets(FileClass &)` on the file's bytes: every top-level
    /// chunk to its loader. True once the file is open (always, here).
    pub fn load_3d_assets<D: LocalDevice>(&mut self, fs: &FileSystem<D>, data: &[u8]) -> W3dResult<bool> {
        let Some(file) = GameFile::new(data) else { return Err(W3dUndefined::Allocation.into()) };
        let mut cload = ChunkLoad::new(file);
        while cload.open_chunk()? {
            match cload.cur_chunk_id()? {
                W3D_CHUNK_HIERARCHY => {
                    self.load_tree(&mut cload)?;
                }
                W3D_CHUNK_ANIMATION | W3D_CHUNK_COMPRESSED_ANIMATION | W3D_CHUNK_MORPH_ANIMATION => {
                    self.load_anim(fs, &mut cload)?;
                }
                _ => {
                    self.load_prototype(&mut cload)?;
                }
            }
            cload.close_chunk()?;
        }
        Ok(true)
    }

    /// `Load_3D_Assets(const char * filename)`: the game's override (for
    /// [`ManagerKind::Game`]) and then the library's: the file found through
    /// [`W3dFileFactory`] and read through `fs`. False when there is no such
    /// file.
    pub fn load_3d_assets_file<D: LocalDevice>(&mut self, fs: &FileSystem<D>, filename: &[u8]) -> W3dResult<bool> {
        let filename = c_name(filename);
        if self.kind == ManagerKind::Game {
            // strcpy(basename, filename); drop the extension from the last '.'
            if filename.len() + 1 > GAME_BASENAME_LEN {
                return Err(W3dUndefined::StringOverflow.into());
            }
            let base = match filename.iter().rposition(|&c| c == b'.') {
                Some(i) => &filename[..i],
                None => filename,
            };
            if self.find_prototype(base).is_some() {
                return Ok(true); // "this file has already been loaded"
            }
        }
        // _TheFileFactory->Get_File(filename); Is_Available(); Open()
        let Some(path) = self.file_factory.resolve(fs, filename)? else {
            return Ok(false); // "Missing asset"
        };
        let Some(data) = fs.open_file(&path) else { return Ok(false) };
        let key: Vec<u8> = path.iter().map(|c| c.to_ascii_lowercase()).collect();
        let state = self.state();
        if self.loading.iter().any(|(k, s)| *k == key && *s == state) {
            return Err(W3dError::NonTerminating);
        }
        self.loading.push((key, state));
        let r = self.load_3d_assets(fs, &data);
        self.loading.pop();
        r
    }

    /// A file load, then `..\<filename>` if it returned false.
    fn load_with_parent<D: LocalDevice>(&mut self, fs: &FileSystem<D>, filename: &[u8]) -> W3dResult<()> {
        if !self.load_3d_assets_file(fs, filename)? {
            let mut parent = b"..\\".to_vec();
            parent.extend_from_slice(filename);
            self.load_3d_assets_file(fs, &parent)?;
        }
        Ok(())
    }

    /// The prototype `Create_Render_Obj(name)` would create from (the game's
    /// `Create_Render_Obj(name)` is the library's): found, or loaded on
    /// demand from `<name before its first '.'>.w3d`. `None` is upstream's
    /// "Failed to create Render Object".
    pub fn create_render_obj<D: LocalDevice>(&mut self, fs: &FileSystem<D>, name: &[u8]) -> W3dResult<Option<&Prototype>> {
        let name = c_name(name);
        if self.load_on_demand && self.find_prototype(name).is_none() {
            let mut filename = match name.iter().position(|&c| c == b'.') {
                // lstrcpyn(filename, name, dot + 1): the dot characters and a 0
                Some(dot) => name[..dot].to_vec(),
                None => name.to_vec(),
            };
            filename.extend_from_slice(b".w3d");
            if filename.len() + 1 > MAX_PATH {
                return Err(W3dUndefined::StringOverflow.into());
            }
            self.load_with_parent(fs, &filename)?;
        }
        Ok(self.find_prototype(name))
    }

    fn get_htree_index<D: LocalDevice>(&mut self, fs: &FileSystem<D>, name: &[u8]) -> W3dResult<Option<usize>> {
        let name = c_name(name);
        let mut i = self.get_tree_id(name);
        if self.load_on_demand && i.is_none() {
            // sprintf(filename, "%s.w3d", name)
            let mut filename = name.to_vec();
            filename.extend_from_slice(b".w3d");
            if filename.len() + 1 > MAX_PATH {
                return Err(W3dUndefined::StringOverflow.into());
            }
            self.load_with_parent(fs, &filename)?;
            i = self.get_tree_id(name);
        }
        Ok(i)
    }

    /// `Get_HTree(name)`: the hierarchy, loaded on demand from
    /// `<name>.w3d`.
    pub fn get_htree<D: LocalDevice>(&mut self, fs: &FileSystem<D>, name: &[u8]) -> W3dResult<Option<&HTree>> {
        Ok(self.get_htree_index(fs, name)?.map(|i| &self.trees[i].1))
    }

    /// `Get_HAnim(name)` (the game's override calls the library's for W3D
    /// animations): the animation `"H.A"`, loaded on demand from `A.w3d`.
    pub fn get_hanim<D: LocalDevice>(&mut self, fs: &FileSystem<D>, name: &[u8]) -> W3dResult<Option<&HAnim>> {
        let name = c_name(name);
        let mut i = self.peek_anim_index(name);
        if self.load_on_demand && i.is_none() && !self.is_missing(name) {
            let Some(dot) = name.iter().position(|&c| c == b'.') else {
                // "Animation %s has no . in the name"; WWASSERT(0)
                return Ok(None);
            };
            let mut filename = name[dot + 1..].to_vec();
            filename.extend_from_slice(b".w3d");
            if filename.len() + 1 > MAX_PATH {
                return Err(W3dUndefined::StringOverflow.into());
            }
            self.load_with_parent(fs, &filename)?;
            i = self.peek_anim_index(name);
            if i.is_none() {
                self.register_missing(name);
            }
        }
        Ok(i.map(|i| &self.anims[i].value))
    }

    /// `Free_Assets()`: the prototypes, animations and hierarchies (not the
    /// missing-animation table, the loaders or the settings).
    pub fn free_assets(&mut self) {
        self.prototypes.clear();
        self.anims.clear();
        self.trees.clear();
    }
}

/// `NullLoaderClass::Load_W3D(cload)`: the chunk's first 48 bytes as a
/// `W3dNullObjectStruct`, read without a check (a short read leaves the
/// struct on the stack uninitialised: undefined).
pub fn load_null(cload: &mut ChunkLoad<'_>) -> W3dResult<NullPrototype> {
    let mut b = [0u8; W3dNullObjectStruct::SIZE];
    if cload.read_into(&mut b)? != W3dNullObjectStruct::SIZE as u32 {
        return Err(W3dUndefined::UninitialisedRead.into());
    }
    Ok(NullPrototype { definition: W3dNullObjectStruct::from_bytes(&b) })
}

/// A `StringClass` that `READ_WWSTRING_CHUNK` reads into: its allocated
/// length and its buffer, held only as far as the file could fill it (the
/// rest of the allocation is the allocator's zeros).
#[derive(Default)]
struct WwString {
    allocated: usize,
    bytes: Vec<u8>,
}

impl WwString {
    /// `cload.Read(var.Get_Buffer(len), len)`: `Uninitialised_Grow(len)`
    /// (a new zeroed buffer of exactly `len` bytes when `len` is above the
    /// allocated length, else the old buffer with its bytes), then the read,
    /// which copies what the file has (all of it or, cut short, the rest).
    fn read_chunk(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<()> {
        let len = cload.cur_chunk_length()?;
        if len as i32 > 0 && len as usize > self.allocated {
            if u64::from(len) + STRING_HEADER_SIZE > WIN32_MAX_ALLOCATION {
                return Err(W3dUndefined::Allocation.into());
            }
            let f = cload.file();
            let left = f.file_size().wrapping_sub(f.position()).max(0) as usize;
            self.allocated = len as usize;
            self.bytes = vec![0u8; self.allocated.min(left)];
        }
        cload.read(&mut self.bytes, len)?;
        Ok(())
    }

    /// The C string: up to its first 0; `StringOverrun` when the buffer is
    /// full without one (`Allocate_Buffer` leaves no room for a terminator).
    fn c_str(&self) -> W3dResult<Vec<u8>> {
        match self.bytes.iter().position(|&c| c == 0) {
            Some(n) => Ok(self.bytes[..n].to_vec()),
            None if self.bytes.len() < self.allocated || self.allocated == 0 => Ok(self.bytes.clone()),
            None => Err(W3dUndefined::StringOverrun.into()),
        }
    }
}

/// `DazzleLoaderClass::Load_W3D(cload)` (`DazzlePrototypeClass::Load_W3D`):
/// the name and type-name chunks, each read into its `StringClass`; other
/// chunks skipped. Never fails.
pub fn load_dazzle(cload: &mut ChunkLoad<'_>) -> W3dResult<DazzlePrototype> {
    let mut name = WwString::default();
    let mut type_name = WwString::default();
    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            W3D_CHUNK_DAZZLE_NAME => name.read_chunk(cload)?,
            W3D_CHUNK_DAZZLE_TYPENAME => type_name.read_chunk(cload)?,
            _ => {}
        }
        cload.close_chunk()?;
    }
    // DazzleRenderObjClass::Get_Type_ID(dazzle_type) reads the type name
    let type_name = type_name.c_str()?;
    Ok(DazzlePrototype { name: name.c_str()?, type_name })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::file_system::MemoryDevice;

    fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    /// A container chunk (the sub-chunk flag set).
    fn chunk_sub(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32 | 0x8000_0000).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn name16(s: &[u8]) -> [u8; W3D_NAME_LEN] {
        let mut n = [0u8; W3D_NAME_LEN];
        n[..s.len()].copy_from_slice(s);
        n
    }

    /// A mesh with only its header: `container.mesh` (or `mesh`).
    pub(crate) fn mesh_chunk(container: &[u8], mesh: &[u8]) -> Vec<u8> {
        let mut h = W3dMeshHeader3Struct::from_bytes(&[0; W3dMeshHeader3Struct::SIZE]);
        h.version = w3d_make_version(4, 2);
        h.mesh_name = name16(mesh);
        h.container_name = name16(container);
        chunk_sub(W3D_CHUNK_MESH, &chunk(W3D_CHUNK_MESH_HEADER3, &h.to_bytes()))
    }

    /// A hierarchy of `n` pivots (all roots).
    pub(crate) fn htree_chunk(name: &[u8], n: u32) -> Vec<u8> {
        let mut h = W3dHierarchyStruct::from_bytes(&[0; W3dHierarchyStruct::SIZE]);
        h.version = w3d_make_version(4, 1);
        h.name = name16(name);
        h.num_pivots = n;
        let mut body = chunk(W3D_CHUNK_HIERARCHY_HEADER, &h.to_bytes());
        let mut pivots = Vec::new();
        for i in 0..n {
            let mut p = W3dPivotStruct::from_bytes(&[0; W3dPivotStruct::SIZE]);
            p.name = name16(format!("B{i}").as_bytes());
            p.parent_idx = 0xFFFF_FFFF;
            p.rotation.q = [0.0, 0.0, 0.0, 1.0];
            pivots.extend(p.to_bytes());
        }
        body.extend(chunk(W3D_CHUNK_PIVOTS, &pivots));
        chunk_sub(W3D_CHUNK_HIERARCHY, &body)
    }

    /// A raw animation `hier.name` with no channels.
    pub(crate) fn anim_chunk(hier: &[u8], name: &[u8]) -> Vec<u8> {
        let mut h = W3dAnimHeaderStruct::from_bytes(&[0; W3dAnimHeaderStruct::SIZE]);
        h.version = w3d_make_version(4, 1);
        h.name = name16(name);
        h.hierarchy_name = name16(hier);
        h.num_frames = 10;
        h.frame_rate = 30;
        chunk_sub(W3D_CHUNK_ANIMATION, &chunk(W3D_CHUNK_ANIMATION_HEADER, &h.to_bytes()))
    }

    /// An HLOD `name` over `hier` with one LOD of one sub-object.
    pub(crate) fn hlod_chunk(name: &[u8], hier: &[u8], sub: &[u8]) -> Vec<u8> {
        let mut h = W3dHLodHeaderStruct::from_bytes(&[0; W3dHLodHeaderStruct::SIZE]);
        h.version = w3d_make_version(1, 0);
        h.lod_count = 1;
        h.name = name16(name);
        h.hierarchy_name = name16(hier);
        let mut a = W3dHLodArrayHeaderStruct::from_bytes(&[0; W3dHLodArrayHeaderStruct::SIZE]);
        a.model_count = 1;
        a.max_screen_size = 0.0;
        let mut s = W3dHLodSubObjectStruct::from_bytes(&[0; W3dHLodSubObjectStruct::SIZE]);
        s.name[..sub.len()].copy_from_slice(sub);
        let mut arr = chunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, &a.to_bytes());
        arr.extend(chunk(W3D_CHUNK_HLOD_SUB_OBJECT, &s.to_bytes()));
        let mut body = chunk(W3D_CHUNK_HLOD_HEADER, &h.to_bytes());
        body.extend(chunk_sub(W3D_CHUNK_HLOD_LOD_ARRAY, &arr));
        chunk_sub(W3D_CHUNK_HLOD, &body)
    }

    /// A box named `name`.
    pub(crate) fn box_chunk(name: &[u8]) -> Vec<u8> {
        let mut b = W3dBoxStruct::from_bytes(&[0; W3dBoxStruct::SIZE]);
        b.name[..name.len()].copy_from_slice(name);
        chunk(W3D_CHUNK_BOX, &b.to_bytes())
    }

    /// A null object named `name`.
    pub(crate) fn null_chunk(name: &[u8]) -> Vec<u8> {
        let mut b = W3dNullObjectStruct::from_bytes(&[0; W3dNullObjectStruct::SIZE]);
        b.name[..name.len()].copy_from_slice(name);
        chunk(W3D_CHUNK_NULL_OBJECT, &b.to_bytes())
    }

    fn fs_with(files: &[(&str, Vec<u8>)]) -> FileSystem<MemoryDevice> {
        let mut d = MemoryDevice::new();
        for (p, data) in files {
            d.insert(p.as_bytes(), data.clone());
        }
        FileSystem::new(d)
    }

    fn proto_names(m: &AssetManager) -> Vec<Vec<u8>> {
        m.prototypes().map(|(n, _)| n.to_vec()).collect()
    }

    /// One file with a hierarchy, a mesh, an HLOD, an animation, a box and a
    /// null object.
    ///
    /// Predicted before the run: prototypes `CONT.MESHA`, `CONT`, `CONT.BOX`
    /// and `NUL1` registered in file order (the hierarchy and the animation
    /// go to their managers instead); hierarchy `SKL` with 3 pivots;
    /// animation `SKL.WALK` with 3 nodes (found through the manager's own
    /// hierarchy lookup); `load_3d_assets` true; class ids mesh 0, HLOD 25,
    /// box 26, null 22.
    #[test]
    fn a_file_registers_each_asset_by_name() {
        let fs = fs_with(&[]);
        let mut data = htree_chunk(b"SKL", 3);
        data.extend(mesh_chunk(b"CONT", b"MESHA"));
        data.extend(hlod_chunk(b"CONT", b"SKL", b"CONT.MESHA"));
        data.extend(anim_chunk(b"SKL", b"WALK"));
        data.extend(box_chunk(b"CONT.BOX"));
        data.extend(null_chunk(b"NUL1"));
        let mut m = AssetManager::new();
        assert!(m.load_3d_assets(&fs, &data).unwrap());
        assert_eq!(proto_names(&m), vec![b"CONT.MESHA".to_vec(), b"CONT".to_vec(), b"CONT.BOX".to_vec(), b"NUL1".to_vec()]);
        let ids: Vec<u32> = m.prototypes().map(|(_, p)| p.class_id()).collect();
        assert_eq!(ids, vec![0, 25, 26, 22]);
        assert_eq!(m.htrees().map(|(n, t)| (n.to_vec(), t.num_pivots)).collect::<Vec<_>>(), vec![(b"SKL".to_vec(), 3)]);
        match m.peek_anim(b"SKL.WALK") {
            Some(HAnim::Raw(a)) => assert_eq!(a.num_nodes, 3),
            other => panic!("{other:?}"),
        }
    }

    /// Lookups ignore ASCII case everywhere; `"NULL"` is built in.
    ///
    /// Predicted before the run: `cont.mesha`, `Skl`, `skl.walk` all found;
    /// `null`/`Null` find the built-in null prototype (name `NULL`, class
    /// 22) with nothing loaded; a null object named `null` in a file is
    /// dropped as a collision; a non-ASCII byte is compared exactly
    /// (`\xC9` is not `\xE9`).
    #[test]
    fn lookups_ignore_ascii_case() {
        let fs = fs_with(&[]);
        let mut data = htree_chunk(b"SKL", 1);
        data.extend(mesh_chunk(b"CONT", b"MESHA"));
        data.extend(anim_chunk(b"SKL", b"WALK"));
        data.extend(null_chunk(b"null"));
        data.extend(box_chunk(b"\xC9BOX"));
        let mut m = AssetManager::new();
        m.load_3d_assets(&fs, &data).unwrap();
        assert!(m.render_obj_exists(b"cont.mesha"));
        assert!(m.peek_htree(b"Skl").is_some());
        assert!(m.peek_anim(b"skl.walk").is_some());
        let n = m.find_prototype(b"Null").unwrap();
        assert_eq!(n.name().unwrap(), b"NULL");
        assert_eq!(n.class_id(), CLASSID_NULL);
        assert_eq!(proto_names(&m), vec![b"CONT.MESHA".to_vec(), b"\xC9BOX".to_vec()]);
        assert!(m.render_obj_exists(b"\xC9box"));
        assert!(!m.render_obj_exists(b"\xE9BOX"));
    }

    /// Duplicate names.
    ///
    /// Predicted before the run: from files, the first wins: the second mesh
    /// `cont.mesha` (other case) is dropped (one prototype), the second
    /// hierarchy `skl` (5 pivots) is dropped (3 kept), the second animation
    /// `skl.walk` is dropped. Through `add_prototype` the last wins: both
    /// entries kept, the lookup finds the newer (a box); removing it by name
    /// uncovers the older (the mesh); removing `NULL` is undefined.
    #[test]
    fn duplicate_names() {
        let fs = fs_with(&[]);
        let mut data = htree_chunk(b"SKL", 3);
        data.extend(mesh_chunk(b"CONT", b"MESHA"));
        data.extend(htree_chunk(b"skl", 5));
        data.extend(mesh_chunk(b"cont", b"mesha"));
        data.extend(anim_chunk(b"SKL", b"WALK"));
        data.extend(anim_chunk(b"skl", b"walk"));
        let mut m = AssetManager::new();
        m.load_3d_assets(&fs, &data).unwrap();
        assert_eq!(proto_names(&m), vec![b"CONT.MESHA".to_vec()]);
        assert_eq!(m.htrees().count(), 1);
        assert_eq!(m.peek_htree(b"SKL").unwrap().num_pivots, 3);
        assert_eq!(m.anim_names(), vec![&b"SKL.WALK"[..]]);

        let mut b = BoxPrototype { definition: W3dBoxStruct::from_bytes(&[0; W3dBoxStruct::SIZE]) };
        b.definition.name[..10].copy_from_slice(b"cont.mesha");
        m.add_prototype(Prototype::Box(b)).unwrap();
        assert_eq!(m.prototypes().count(), 2);
        assert_eq!(m.find_prototype(b"CONT.MESHA").unwrap().class_id(), boxrobj::CLASSID_AABOX);
        assert!(m.remove_prototype(b"Cont.Mesha").unwrap());
        assert_eq!(m.find_prototype(b"CONT.MESHA").unwrap().class_id(), CLASSID_MESH);
        assert!(m.remove_prototype(b"NULL").is_err());
    }

    /// On-demand loading of a render object, through the game's file paths.
    ///
    /// Predicted before the run: with load on demand off, `TANK.BODY` is not
    /// found and nothing is read; with it on (the game manager), `TANK.BODY`
    /// loads `Art/W3D/TANK.w3d` (found as `art\w3d\tank.w3d` in memory) and
    /// is found; `TANK` (the HLOD in the same file) is then found without a
    /// load; `GHOST` (no file) is not found, and no file is registered for
    /// it; a name of 256 characters overflows `MAX_PATH` (`+ ".w3d"`).
    #[test]
    fn render_objects_load_on_demand() {
        let mut file = mesh_chunk(b"TANK", b"BODY");
        file.extend(hlod_chunk(b"TANK", b"TANKSKL", b"TANK.BODY"));
        let fs = fs_with(&[("art/w3d/tank.w3d", file)]);
        let mut m = AssetManager::new();
        assert!(m.create_render_obj(&fs, b"TANK.BODY").unwrap().is_none());
        let mut g = AssetManager::new_game(W3dFileFactory::default());
        assert_eq!(g.create_render_obj(&fs, b"TANK.BODY").unwrap().unwrap().class_id(), CLASSID_MESH);
        assert_eq!(g.create_render_obj(&fs, b"tank").unwrap().unwrap().class_id(), 25);
        assert_eq!(proto_names(&g), vec![b"TANK.BODY".to_vec(), b"TANK".to_vec()]);
        assert!(g.create_render_obj(&fs, b"GHOST").unwrap().is_none());
        assert_eq!(g.prototypes().count(), 2);
        let long = vec![b'A'; 256];
        assert_eq!(g.create_render_obj(&fs, &long).unwrap_err(), W3dError::Undefined(W3dUndefined::StringOverflow));
    }

    /// Path search order and the parent-directory retry.
    ///
    /// Predicted before the run: the localized copy
    /// (`Data/german/Art/W3D/X.w3d`, a box `LOCAL`) wins over `Art/W3D/`
    /// (a box `MAIN`); `Y` is found only under `../TestArt/` (box `TEST`);
    /// `Z` only under the user data directory (box `USER`); `W.w3d` only as
    /// `Art/W3D/..\W.w3d` (the `..\` retry, box `PARENT`); a `.tga` resolves
    /// under `Art/Textures/`; a name with no extension is looked up as is.
    #[test]
    fn file_paths_follow_game_file_class() {
        let fs = fs_with(&[
            ("Data/german/Art/W3D/X.w3d", box_chunk(b"LOCAL")),
            ("Art/W3D/X.w3d", box_chunk(b"MAIN")),
            ("../TestArt/Y.w3d", box_chunk(b"TEST")),
            ("User/W3D/Z.w3d", box_chunk(b"USER")),
            ("Art/W3D/..\\W.w3d", box_chunk(b"PARENT")),
            ("Art/Textures/t.tga", vec![]),
            ("plain", vec![]),
        ]);
        let f = W3dFileFactory { language: b"german".to_vec(), user_data_path: Some(b"User/".to_vec()) };
        let mut g = AssetManager::new_game(f.clone());
        for n in [&b"X"[..], b"Y", b"Z", b"W"] {
            g.create_render_obj(&fs, n).unwrap();
        }
        assert_eq!(proto_names(&g), vec![b"LOCAL".to_vec(), b"TEST".to_vec(), b"USER".to_vec(), b"PARENT".to_vec()]);
        assert_eq!(f.resolve(&fs, b"t.tga").unwrap(), Some(b"Art/Textures/t.tga".to_vec()));
        assert_eq!(f.resolve(&fs, b"plain").unwrap(), Some(b"plain".to_vec()));
        assert_eq!(f.resolve(&fs, b"none.w3d").unwrap(), None);
    }

    /// Animations by `"H.A"`, with on-demand hierarchy and animation files.
    ///
    /// Predicted before the run: `SKL.RUN` loads `RUN.w3d` (the part after
    /// the dot), whose animation names hierarchy `SKL`, which is not loaded:
    /// the animation loader's lookup loads `SKL.w3d` (nested), then the
    /// animation loads with its 4 nodes. `SKL.JUMP` (no file) returns none
    /// and is registered missing; asked again it is not looked for (still
    /// missing, still none, even after `JUMP.w3d` appears in a new file
    /// system); after `free_assets` it stays missing. A name with no dot is
    /// none and not registered.
    #[test]
    fn animations_load_on_demand() {
        let fs = fs_with(&[("Art/W3D/RUN.w3d", anim_chunk(b"SKL", b"RUN")), ("Art/W3D/SKL.w3d", htree_chunk(b"SKL", 4))]);
        let mut g = AssetManager::new_game(W3dFileFactory::default());
        match g.get_hanim(&fs, b"SKL.RUN").unwrap() {
            Some(HAnim::Raw(a)) => assert_eq!(a.num_nodes, 4),
            other => panic!("{other:?}"),
        }
        assert_eq!(g.htrees().count(), 1);
        assert!(g.get_hanim(&fs, b"SKL.JUMP").unwrap().is_none());
        assert!(g.is_missing(b"skl.jump"));
        let fs2 = fs_with(&[("Art/W3D/JUMP.w3d", anim_chunk(b"SKL", b"JUMP"))]);
        assert!(g.get_hanim(&fs2, b"SKL.JUMP").unwrap().is_none());
        g.free_assets();
        assert!(g.is_missing(b"SKL.JUMP"));
        assert!(g.get_hanim(&fs, b"NODOT").unwrap().is_none());
        assert!(!g.is_missing(b"NODOT"));
    }

    /// The game's shortcut: a file whose base name is a prototype is not
    /// read.
    ///
    /// Predicted before the run: with a prototype `RUN` loaded (a box),
    /// `Get_HAnim("SKL.RUN")` asks for `RUN.w3d`, the shortcut returns true
    /// without reading it, the animation is not found and is registered
    /// missing. The library manager (on demand on) reads the file and finds
    /// it.
    #[test]
    fn game_shortcut_skips_files_named_like_prototypes() {
        let mut run = htree_chunk(b"SKL", 2);
        run.extend(anim_chunk(b"SKL", b"RUN"));
        let fs = fs_with(&[("Art/W3D/RUN.w3d", run)]);
        let mut g = AssetManager::new_game(W3dFileFactory::default());
        g.load_3d_assets(&fs, &box_chunk(b"RUN")).unwrap();
        assert!(g.get_hanim(&fs, b"SKL.RUN").unwrap().is_none());
        assert!(g.is_missing(b"SKL.RUN"));
        let mut w = AssetManager::new();
        w.load_on_demand = true;
        w.load_3d_assets(&fs, &box_chunk(b"RUN")).unwrap();
        assert!(w.get_hanim(&fs, b"SKL.RUN").unwrap().is_some());
    }

    /// Unsupported loaders, chunks without a loader, recursion.
    ///
    /// Predicted before the run: an unknown chunk id (0x9999) is skipped and
    /// the next box loads; an emitter chunk is `Unsupported` in the game
    /// manager and skipped (no loader) in the library one; `Get_HAnim("A.A")`
    /// loads `A.w3d`, whose animation's hierarchy `A` is not in it: the
    /// lookup loads `A.w3d` again with nothing loaded in between:
    /// `NonTerminating`.
    #[test]
    fn unknown_unsupported_and_recursive() {
        let fs = fs_with(&[("Art/W3D/A.w3d", anim_chunk(b"A", b"A"))]);
        let mut data = chunk(0x9999, &[1, 2, 3]);
        data.extend(box_chunk(b"B1"));
        let mut m = AssetManager::new();
        m.load_3d_assets(&fs, &data).unwrap();
        assert_eq!(proto_names(&m), vec![b"B1".to_vec()]);
        let em = chunk(W3D_CHUNK_EMITTER, &[]);
        assert!(m.load_3d_assets(&fs, &em).unwrap());
        let mut g = AssetManager::new_game(W3dFileFactory::default());
        assert!(matches!(g.load_3d_assets(&fs, &em), Err(W3dError::Unsupported(_))));
        assert_eq!(g.get_hanim(&fs, b"A.A").unwrap_err(), W3dError::NonTerminating);
    }

    /// Dazzles and anim iteration order.
    ///
    /// Predicted before the run: name chunks `XYZ` (3 bytes, no 0) then `AB`
    /// reuse the 3-byte buffer: `ABZ` with no terminator, undefined
    /// (`StringOverrun`). With `XYZ\0` first, the 4-byte buffer becomes
    /// `ABZ\0`: the dazzle registers as `ABZ` with type `SUN` (class 31).
    /// The animation names come out in CRC bucket order.
    #[test]
    fn dazzles_and_anim_order() {
        let fs = fs_with(&[]);
        let mut body = chunk(W3D_CHUNK_DAZZLE_NAME, b"XYZ");
        body.extend(chunk(W3D_CHUNK_DAZZLE_NAME, b"AB"));
        body.extend(chunk(W3D_CHUNK_DAZZLE_TYPENAME, b"SUN\0"));
        let mut m = AssetManager::new();
        m.load_3d_assets(&fs, &chunk_sub(W3D_CHUNK_DAZZLE, &body)).unwrap_err();
        let mut body = chunk(W3D_CHUNK_DAZZLE_NAME, b"XYZ\0");
        body.extend(chunk(W3D_CHUNK_DAZZLE_NAME, b"AB"));
        body.extend(chunk(W3D_CHUNK_DAZZLE_TYPENAME, b"SUN\0"));
        m.load_3d_assets(&fs, &chunk_sub(W3D_CHUNK_DAZZLE, &body)).unwrap();
        match m.find_prototype(b"abz") {
            Some(Prototype::Dazzle(d)) => assert_eq!(d.type_name, b"SUN"),
            other => panic!("{other:?}"),
        }
        let mut data = htree_chunk(b"S", 1);
        for a in [&b"A"[..], b"B", b"C", b"D"] {
            data.extend(anim_chunk(b"S", a));
        }
        m.load_3d_assets(&fs, &data).unwrap();
        let mut want: Vec<&[u8]> = vec![b"S.A", b"S.B", b"S.C", b"S.D"];
        want.sort_by_key(|n| crc_stringi(n, 0) & 2047);
        assert_eq!(m.anim_names(), want);
    }
}
