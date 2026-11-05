// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hlod.cpp
//   (HLodDefClass::Load_W3D, read_header, read_proxy_array, Free;
//   HLodDefClass::SubObjectArrayClass's constructor, Reset and Load_W3D;
//   ProxyRecordClass and ProxyArrayClass) and hlod.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, hlod by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D hierarchical LOD models (`W3D_CHUNK_HLOD`): `HLodDefClass::Load_W3D`,
//! which reads the definition an `HLodClass` is later built from: a name, the
//! hierarchy (HTree) it uses, one array of sub-objects (model name and bone)
//! per level of detail, an array of "aggregates" and an array of "proxies".
//! Only the loading; ~~the render object (`HLodClass`),~~ the saving
//! (`Save*`) and `Initialize` from a live `HLodClass` are not translated.
//! **Corrected 2026-11-05 (#10, step 4):** the part of `HLodClass`'s
//! constructor that decides what is drawn (each LOD's and the aggregates'
//! sub-objects created by name and bound to their bones) is translated in
//! [`super::render::RenderModel`]; LOD switching, proxies and the rest of the
//! render object are not.
//!
//! # As upstream reads it
//!
//! Inside the HLOD chunk: a first chunk that must be `W3D_CHUNK_HLOD_HEADER`
//! with a whole [`W3dHLodHeaderStruct`]; then `LodCount` chunks that must each
//! be `W3D_CHUNK_HLOD_LOD_ARRAY`; then any chunks, of which
//! `W3D_CHUNK_HLOD_AGGREGATE_ARRAY` and `W3D_CHUNK_HLOD_PROXY_ARRAY` are read
//! and the others skipped. Each LOD array and the aggregate array are read by
//! `SubObjectArrayClass::Load_W3D` ([`SubObjectArray::load_w3d`]): a first
//! chunk that must be `W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER` with a whole
//! [`W3dHLodArrayHeaderStruct`] (`ModelCount`, `MaxScreenSize`), then
//! `ModelCount` chunks that must each be `W3D_CHUNK_HLOD_SUB_OBJECT` with a
//! whole [`W3dHLodSubObjectStruct`] (`BoneIndex`, `Name`). The proxy array
//! has the same layout and is read by `read_proxy_array` into
//! [`ProxyRecord`]s. The load fails ([`Ok(None)`](load_w3d)) only on the
//! header and on a missing or mis-typed LOD array chunk.
//!
//! # Upstream quirks that are kept
//!
//! - The result of each LOD's and the aggregates' `Load_W3D`, and of
//!   `read_proxy_array`, is ignored: a sub-object array that fails part-way
//!   is kept as far as it got and the load goes on. Its `ModelCount` is the
//!   header's; the entries not read keep the allocator's zeros (the game's
//!   allocator zero-fills `new[]`): a null name and bone 0 (see
//!   [`SubObjectArray::sub_object`]).
//! - On a failure the chunks opened so far stay open (see
//!   [`super::htree`]). Inside a LOD array this is not a load failure:
//!   `Load_W3D`'s `Close_Chunk` then closes the inner chunk left open, the
//!   LOD array chunk stays open, and the following chunks are read *from
//!   inside the LOD array* (its remaining chunks are taken as the next LOD
//!   array, or as aggregates or proxies). The chunks after it in the HLOD
//!   chunk are then never read, and the load still succeeds.
//! - A second aggregate array is read over the first without `Reset`: if
//!   it fails before its array header is read the first one's data is kept,
//!   otherwise its count, screen size and (new) sub-objects replace it.
//!   `read_proxy_array` instead releases the previous proxy array first, so
//!   a later proxy chunk that fails before its header leaves no proxies.
//! - `Name` is `strdup` of the header's 16-byte `Name`: a 16-character name
//!   has no terminator and runs on into `HierarchyName` (up to 31
//!   characters). `HierarchyTreeName` and the sub-object names are `strdup`
//!   too (any length up to their field). Proxy names are `strncpy`'d into a
//!   32-byte buffer: zero-padded after the first 0, unterminated at 32
//!   characters.
//! - `LodCount`, `ModelCount` and `BoneIndex` are the file's `uint32`s as
//!   `int`: a bone index of 2^31 or more is negative.
//! - `MaxScreenSize` is the file's float as is; a LOD (or the aggregates)
//!   whose header is never read keeps [`NO_MAX_SCREEN_SIZE`].
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: `HierarchyName` (or a 16-character `Name` running into
//! it) or a sub-object `Name` without a 0 byte before the end of its struct
//! (`strdup` reads past the struct on the stack:
//! [`W3dUndefined::StringOverrun`]); a negative count (`LodCount`,
//! `ModelCount`, or a non-zero proxy count), or one whose array the 32-bit
//! original cannot allocate ([`W3dUndefined::Allocation`]: `LodCount * 16 +
//! 4` bytes for the LOD arrays, with the array cookie of a class with a
//! destructor; `ModelCount * 4` for the name-pointer and bone arrays;
//! proxies `* 36`; over 2^31 - 1 bytes).
//!
//! # Representation
//!
//! [`HLodDef::lods`] holds the `LodCount` sub-object arrays (all of them,
//! after a successful load). In a [`SubObjectArray`], `model_count` is
//! `ModelCount` and [`SubObjectArray::sub_objects`] holds only the entries
//! actually read (at most `model_count`); the rest are the zeroed default
//! ([`SubObject::default`]), which [`SubObjectArray::sub_object`] returns.
//! [`ProxyArray`] does the same for its `Length()` records, whose
//! constructor (`ProxyRecordClass()`) gives bone 0 and an all-zero name.
//! So memory grows with the chunks in the file, not with the header counts.
//! Names are the bytes before the terminator (no `\0`); a null name is
//! `None`.

use super::w3d_file::*;
use super::{c_string, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::chunkio::ChunkLoad;

/// `sizeof(HLodDefClass::SubObjectArrayClass)` on Win32 (`float`, `int`
/// and two pointers).
pub const WIN32_SIZEOF_SUB_OBJECT_ARRAY: u64 = 16;
/// The count `new[]` stores before an array of a class with a destructor
/// (MSVC, Win32).
pub const WIN32_ARRAY_COOKIE: u64 = 4;
/// `sizeof(char *)` and `sizeof(int)` on Win32: the element size of
/// `ModelName` and `BoneIndex`.
pub const WIN32_SIZEOF_POINTER_OR_INT: u64 = 4;
/// `sizeof(ProxyRecordClass)` on Win32 (`int` and `char[2*W3D_NAME_LEN]`;
/// no destructor, so no array cookie).
pub const WIN32_SIZEOF_PROXY_RECORD: u64 = 36;

/// `new T[count]` with `count` an `int`: undefined for a negative count or
/// an array the 32-bit original cannot allocate.
fn check_allocation(count: i32, elem: u64, cookie: u64) -> W3dResult<()> {
    if count < 0 || count as u64 * elem + cookie > WIN32_MAX_ALLOCATION {
        return Err(W3dUndefined::Allocation.into());
    }
    Ok(())
}

/// One entry of a `SubObjectArrayClass`: `ModelName[i]` and `BoneIndex[i]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubObject {
    /// `ModelName[i]` (`strdup` of the file's name, without the `\0`);
    /// `None` for a null pointer (an entry not read).
    pub name: Option<Vec<u8>>,
    /// `BoneIndex[i]`.
    pub bone_index: i32,
}

/// `HLodDefClass::SubObjectArrayClass`: one level of detail, or the
/// aggregates.
#[derive(Clone, Debug, PartialEq)]
pub struct SubObjectArray {
    /// `MaxScreenSize`.
    pub max_screen_size: f32,
    /// `ModelCount`.
    pub model_count: i32,
    /// The entries read, in order (at most `model_count`; see the module
    /// docs).
    pub sub_objects: Vec<SubObject>,
}

impl Default for SubObjectArray {
    /// `SubObjectArrayClass()` (and the state `Reset()` leaves).
    fn default() -> Self {
        SubObjectArray { max_screen_size: NO_MAX_SCREEN_SIZE, model_count: 0, sub_objects: Vec::new() }
    }
}

impl SubObjectArray {
    /// Entry `i` (`ModelName[i]`, `BoneIndex[i]`): as read, or the zeroed
    /// default for an entry not read; `None` at or past `model_count`.
    pub fn sub_object(&self, i: usize) -> Option<SubObject> {
        if i >= self.model_count.max(0) as usize {
            return None;
        }
        Some(self.sub_objects.get(i).cloned().unwrap_or_default())
    }

    /// `SubObjectArrayClass::Load_W3D(cload)`, with `cload` inside the LOD
    /// (or aggregate) array chunk, on `self` as it is (no `Reset`). Returns
    /// upstream's `bool`; on `false` whatever was read so far stays.
    pub fn load_w3d(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        // Open the first chunk, it should be a Lod Array Header
        let Some(header) = read_array_header(cload)? else {
            return Ok(false);
        };
        let model_count = header.model_count as i32;
        // ModelName = new char*[ModelCount]; BoneIndex = new int[ModelCount]
        check_allocation(model_count, WIN32_SIZEOF_POINTER_OR_INT, 0)?;
        self.model_count = model_count;
        self.max_screen_size = header.max_screen_size;
        self.sub_objects = Vec::new();

        // Read each sub object definition
        for _ in 0..model_count {
            let Some(sb) = read_sub_object(cload)? else {
                return Ok(false);
            };
            let sub = W3dHLodSubObjectStruct::from_bytes(&sb);
            let name = c_string(&sb, 4)?.to_vec();
            self.sub_objects.push(SubObject { name: Some(name), bone_index: sub.bone_index as i32 });
        }
        Ok(true)
    }
}

/// `ProxyRecordClass`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProxyRecord {
    /// `BoneIndex`.
    pub bone_index: i32,
    /// `Name` (32 bytes, as `strncpy` leaves them).
    pub name: [u8; 2 * W3D_NAME_LEN],
}

impl Default for ProxyRecord {
    /// `ProxyRecordClass()`: bone 0, name zeroed.
    fn default() -> Self {
        ProxyRecord { bone_index: 0, name: [0; 2 * W3D_NAME_LEN] }
    }
}

impl ProxyRecord {
    /// `Init(w3d_data)`: `BoneIndex = w3d_data.BoneIndex; strncpy(Name,
    /// w3d_data.Name, sizeof(Name))`.
    fn init(&mut self, w3d_data: &W3dHLodSubObjectStruct) {
        self.bone_index = w3d_data.bone_index as i32;
        let mut name = [0u8; 2 * W3D_NAME_LEN];
        for (dst, &c) in name.iter_mut().zip(w3d_data.name.iter()) {
            if c == 0 {
                break;
            }
            *dst = c;
        }
        self.name = name;
    }

    /// `Get_Name()` up to its terminator (all 32 bytes if there is none).
    pub fn name_str(&self) -> &[u8] {
        let n = self.name.iter().position(|&c| c == 0).unwrap_or(self.name.len());
        &self.name[..n]
    }
}

/// `ProxyArrayClass` (a `VectorClass<ProxyRecordClass>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyArray {
    /// `Length()`: the header's `ModelCount`.
    pub length: i32,
    /// The records read, in order (at most `length`; the rest are
    /// [`ProxyRecord::default`]).
    pub records: Vec<ProxyRecord>,
}

impl ProxyArray {
    /// `(*ProxyArray)[i]`: as read, or the default record; `None` at or past
    /// `Length()`.
    pub fn record(&self, i: usize) -> Option<ProxyRecord> {
        if i >= self.length.max(0) as usize {
            return None;
        }
        Some(self.records.get(i).copied().unwrap_or_default())
    }
}

/// `class HLodDefClass`: what `Load_W3D` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct HLodDef {
    /// `Name` (`strdup` of the header's `Name`, see the module docs).
    pub name: Vec<u8>,
    /// `HierarchyTreeName`.
    pub hierarchy_tree_name: Vec<u8>,
    /// `LodCount`.
    pub lod_count: i32,
    /// `Lod[0..LodCount]`.
    pub lods: Vec<SubObjectArray>,
    /// `Aggregates`.
    pub aggregates: SubObjectArray,
    /// `ProxyArray` (`None` for a null pointer).
    pub proxy_array: Option<ProxyArray>,
}

impl HLodDef {
    /// `Get_Name()`.
    pub fn name(&self) -> &[u8] {
        &self.name
    }
}

/// Opens the next chunk, which must be `W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER`,
/// reads the header and closes it; `None` (chunk left open) on a failure.
fn read_array_header(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<W3dHLodArrayHeaderStruct>> {
    if !cload.open_chunk()? {
        return Ok(None);
    }
    if cload.cur_chunk_id()? != W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER {
        return Ok(None);
    }
    let mut hb = [0u8; W3dHLodArrayHeaderStruct::SIZE];
    if cload.read_into(&mut hb)? != W3dHLodArrayHeaderStruct::SIZE as u32 {
        return Ok(None);
    }
    if !cload.close_chunk()? {
        return Ok(None);
    }
    Ok(Some(W3dHLodArrayHeaderStruct::from_bytes(&hb)))
}

/// Opens the next chunk, which must be `W3D_CHUNK_HLOD_SUB_OBJECT`, reads
/// the struct's bytes and closes it; `None` (chunk left open) on a failure.
fn read_sub_object(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<[u8; W3dHLodSubObjectStruct::SIZE]>> {
    if !cload.open_chunk()? {
        return Ok(None);
    }
    if cload.cur_chunk_id()? != W3D_CHUNK_HLOD_SUB_OBJECT {
        return Ok(None);
    }
    let mut sb = [0u8; W3dHLodSubObjectStruct::SIZE];
    if cload.read_into(&mut sb)? != W3dHLodSubObjectStruct::SIZE as u32 {
        return Ok(None);
    }
    if !cload.close_chunk()? {
        return Ok(None);
    }
    Ok(Some(sb))
}

/// `HLodDefClass::Load_W3D(cload)`, with `cload` inside the HLOD chunk.
/// `Ok(None)` is upstream's `WW3D_ERROR_LOAD_FAILED`.
pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<HLodDef>> {
    // (Free() of a new definition does nothing.)
    let Some(mut def) = read_header(cload)? else {
        return Ok(None);
    };

    // Loop through all the LODs and read the info from its chunk
    for _ in 0..def.lod_count {
        // Open the next chunk, it should be a LOD struct
        if !cload.open_chunk()? {
            return Ok(None);
        }
        if cload.cur_chunk_id()? != W3D_CHUNK_HLOD_LOD_ARRAY {
            // ERROR: Expected LOD struct!
            return Ok(None);
        }
        let mut lod = SubObjectArray::default();
        // The result is ignored upstream.
        let _ = lod.load_w3d(cload)?;
        def.lods.push(lod);
        // Close-out the chunk (the innermost one open, see the module docs)
        cload.close_chunk()?;
    }

    // Parse the rest of the chunks
    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            W3D_CHUNK_HLOD_AGGREGATE_ARRAY => {
                let _ = def.aggregates.load_w3d(cload)?;
            }
            W3D_CHUNK_HLOD_PROXY_ARRAY => {
                let _ = read_proxy_array(cload, &mut def)?;
            }
            _ => {}
        }
        cload.close_chunk()?;
    }
    Ok(Some(def))
}

/// `HLodDefClass::read_header(cload)`: the definition with its header
/// fields set and no LODs read yet, or `None` for upstream's `false`.
fn read_header(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<HLodDef>> {
    // Open the first chunk, it should be the LOD header
    if !cload.open_chunk()? {
        return Ok(None);
    }
    if cload.cur_chunk_id()? != W3D_CHUNK_HLOD_HEADER {
        // ERROR: Expected HLOD Header!
        return Ok(None);
    }
    let mut hb = [0u8; W3dHLodHeaderStruct::SIZE];
    if cload.read_into(&mut hb)? != W3dHLodHeaderStruct::SIZE as u32 {
        return Ok(None);
    }
    cload.close_chunk()?;
    let header = W3dHLodHeaderStruct::from_bytes(&hb);

    // Copy the name into our internal variable (strdup from the struct on
    // the stack: Name is at offset 8, HierarchyName at 24)
    let name = c_string(&hb, 8)?.to_vec();
    let hierarchy_tree_name = c_string(&hb, 24)?.to_vec();
    let lod_count = header.lod_count as i32;
    // Lod = new SubObjectArrayClass[LodCount]
    check_allocation(lod_count, WIN32_SIZEOF_SUB_OBJECT_ARRAY, WIN32_ARRAY_COOKIE)?;
    Ok(Some(HLodDef {
        name,
        hierarchy_tree_name,
        lod_count,
        lods: Vec::new(),
        aggregates: SubObjectArray::default(),
        proxy_array: None,
    }))
}

/// `HLodDefClass::read_proxy_array(cload)`, with `cload` inside the proxy
/// array chunk. Returns upstream's `bool`.
fn read_proxy_array(cload: &mut ChunkLoad<'_>, def: &mut HLodDef) -> W3dResult<bool> {
    // REF_PTR_RELEASE(ProxyArray): before anything is read
    def.proxy_array = None;

    // Open the first chunk, it should be a Lod Array Header
    let Some(header) = read_array_header(cload)? else {
        return Ok(false);
    };
    let length = header.model_count as i32;
    // VectorClass(size) allocates only for a non-zero size
    if length != 0 {
        check_allocation(length, WIN32_SIZEOF_PROXY_RECORD, 0)?;
    }
    let proxies = def.proxy_array.insert(ProxyArray { length, records: Vec::new() });

    // Read each sub object definition
    for _ in 0..length {
        let Some(sb) = read_sub_object(cload)? else {
            return Ok(false);
        };
        let mut rec = ProxyRecord::default();
        rec.init(&W3dHLodSubObjectStruct::from_bytes(&sb));
        proxies.records.push(rec);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ww::chunkio::GameFile;

    fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn header(name: &[u8], hier: &[u8], lod_count: u32) -> Vec<u8> {
        let mut h = W3dHLodHeaderStruct::from_bytes(&[0; 40]);
        h.version = W3D_CURRENT_HLOD_VERSION;
        h.lod_count = lod_count;
        h.name[..name.len()].copy_from_slice(name);
        h.hierarchy_name[..hier.len()].copy_from_slice(hier);
        chunk(W3D_CHUNK_HLOD_HEADER, &h.to_bytes())
    }

    fn array_header(model_count: u32, max_screen_size: f32) -> Vec<u8> {
        let mut h = W3dHLodArrayHeaderStruct::from_bytes(&[0; 8]);
        h.model_count = model_count;
        h.max_screen_size = max_screen_size;
        chunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, &h.to_bytes())
    }

    fn sub_object(bone: u32, name: &[u8]) -> Vec<u8> {
        let mut s = W3dHLodSubObjectStruct::from_bytes(&[0; 36]);
        s.bone_index = bone;
        s.name[..name.len()].copy_from_slice(name);
        chunk(W3D_CHUNK_HLOD_SUB_OBJECT, &s.to_bytes())
    }

    /// An array chunk `id` with its header and the given sub-objects.
    fn array(id: u32, model_count: u32, max: f32, subs: &[(u32, &[u8])]) -> Vec<u8> {
        let mut body = array_header(model_count, max);
        for &(bone, name) in subs {
            body.extend(sub_object(bone, name));
        }
        chunk(id, &body)
    }

    /// Loads the HLOD chunk whose body is `body`; also returns the chunk depth
    /// the reader is left at (1 = only the HLOD chunk open, as normal).
    fn load(body: &[u8]) -> (W3dResult<Option<HLodDef>>, i32) {
        let data = chunk(W3D_CHUNK_HLOD, body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let r = load_w3d(&mut c);
        (r, c.cur_chunk_depth())
    }

    fn so(name: &[u8], bone: i32) -> Option<SubObject> {
        Some(SubObject { name: Some(name.to_vec()), bone_index: bone })
    }

    /// A well-formed HLOD: header, two LODs, an aggregate array and a proxy
    /// array, plus an unknown chunk that is skipped.
    ///
    /// Predicted before the run: name `TANK`, hierarchy `TANKSKL`, 2 LODs;
    /// LOD 0 max screen size `NO_MAX_SCREEN_SIZE` (the file's value, f32::MAX)
    /// with `TANK.BODY` on bone 1 and `TANK.TURRET` on bone 2; LOD 1 max 0.25
    /// with `TANKLO` on bone 0 and a bone index `0xFFFFFFFF` read as -1;
    /// aggregates max 1.0 with `FXGLOW` on bone 3; one proxy `SPAWN` on bone 5;
    /// the reader left inside the HLOD chunk only (depth 1).
    #[test]
    fn loads_header_lods_aggregates_and_proxies() {
        let mut body = header(b"TANK", b"TANKSKL", 2);
        body.extend(array(
            W3D_CHUNK_HLOD_LOD_ARRAY,
            2,
            NO_MAX_SCREEN_SIZE,
            &[(1, b"TANK.BODY"), (2, b"TANK.TURRET")],
        ));
        body.extend(array(W3D_CHUNK_HLOD_LOD_ARRAY, 2, 0.25, &[(0, b"TANKLO"), (0xFFFF_FFFF, b"X")]));
        body.extend(chunk(0x0000_0799, &[1, 2, 3]));
        body.extend(array(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, 1, 1.0, &[(3, b"FXGLOW")]));
        body.extend(array(W3D_CHUNK_HLOD_PROXY_ARRAY, 1, 0.0, &[(5, b"SPAWN")]));
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(d.name(), b"TANK");
        assert_eq!(d.hierarchy_tree_name, b"TANKSKL");
        assert_eq!(d.lod_count, 2);
        assert_eq!(d.lods.len(), 2);
        assert_eq!(d.lods[0].max_screen_size, NO_MAX_SCREEN_SIZE);
        assert_eq!(d.lods[0].model_count, 2);
        assert_eq!(d.lods[0].sub_object(0), so(b"TANK.BODY", 1));
        assert_eq!(d.lods[0].sub_object(1), so(b"TANK.TURRET", 2));
        assert_eq!(d.lods[0].sub_object(2), None);
        assert_eq!(d.lods[1].max_screen_size, 0.25);
        assert_eq!(d.lods[1].sub_object(0), so(b"TANKLO", 0));
        assert_eq!(d.lods[1].sub_object(1), so(b"X", -1));
        assert_eq!(d.aggregates.max_screen_size, 1.0);
        assert_eq!(d.aggregates.model_count, 1);
        assert_eq!(d.aggregates.sub_object(0), so(b"FXGLOW", 3));
        let p = d.proxy_array.unwrap();
        assert_eq!(p.length, 1);
        assert_eq!(p.record(0).unwrap().bone_index, 5);
        assert_eq!(p.record(0).unwrap().name_str(), b"SPAWN");
        assert_eq!(p.record(1), None);
    }

    /// Quirk: a LOD array whose header promises 3 sub-objects but holds one.
    /// `SubObjectArrayClass::Load_W3D` returns false, which `Load_W3D`
    /// ignores; a proxy array promising 2 with one likewise.
    ///
    /// Predicted before the run: the load succeeds; the LOD keeps
    /// `ModelCount` 3 and max 50.0, entry 0 is `A` on bone 4, entries 1 and 2
    /// are the allocator's zeros (null name, bone 0); the proxy array has
    /// length 2, record 0 `P` on bone 9, record 1 the default (bone 0, empty
    /// name). Depth 1: the sub-object loop fails on `Open_Chunk` at the end of
    /// its parent, which leaves nothing open.
    #[test]
    fn short_arrays_keep_the_header_count_and_the_load_succeeds() {
        let mut body = header(b"H", b"", 1);
        body.extend(array(W3D_CHUNK_HLOD_LOD_ARRAY, 3, 50.0, &[(4, b"A")]));
        body.extend(array(W3D_CHUNK_HLOD_PROXY_ARRAY, 2, 0.0, &[(9, b"P")]));
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(d.hierarchy_tree_name, b"");
        let lod = &d.lods[0];
        assert_eq!((lod.model_count, lod.max_screen_size, lod.sub_objects.len()), (3, 50.0, 1));
        assert_eq!(lod.sub_object(0), so(b"A", 4));
        assert_eq!(lod.sub_object(1), Some(SubObject { name: None, bone_index: 0 }));
        assert_eq!(lod.sub_object(2), Some(SubObject::default()));
        let p = d.proxy_array.unwrap();
        assert_eq!(p.length, 2);
        assert_eq!((p.record(0).unwrap().bone_index, p.record(0).unwrap().name_str()), (9, &b"P"[..]));
        assert_eq!(p.record(1), Some(ProxyRecord::default()));
    }

    /// Quirk: a LOD array whose first chunk is a sub-object instead of the
    /// array header. The LOD's load fails with that chunk open; `Load_W3D`'s
    /// `Close_Chunk` closes it, not the LOD array, so the rest of the file is
    /// read from inside the LOD array.
    ///
    /// Predicted before the run: the load succeeds with the LOD at its
    /// defaults (max `NO_MAX_SCREEN_SIZE`, count 0); the proxy array nested
    /// in the LOD array is read as the HLOD's (bone 4, `INNER`); the
    /// top-level aggregate array after the LOD array is never read
    /// (aggregates at defaults); the reader is left at depth 2 (the HLOD and
    /// the LOD array chunks open).
    #[test]
    fn failed_lod_header_reads_the_rest_from_inside_the_lod_array() {
        let mut body = header(b"H", b"S", 1);
        let mut lod = sub_object(7, b"WRONG");
        lod.extend(array(W3D_CHUNK_HLOD_PROXY_ARRAY, 1, 0.0, &[(4, b"INNER")]));
        body.extend(chunk(W3D_CHUNK_HLOD_LOD_ARRAY, &lod));
        body.extend(array(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, 1, 5.0, &[(2, b"AGG")]));
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 2);
        assert_eq!(d.lods, vec![SubObjectArray::default()]);
        assert_eq!(d.lods[0].max_screen_size, NO_MAX_SCREEN_SIZE);
        assert_eq!(d.aggregates, SubObjectArray::default());
        let p = d.proxy_array.unwrap();
        assert_eq!((p.length, p.record(0).unwrap().bone_index), (1, 4));
        assert_eq!(p.record(0).unwrap().name_str(), b"INNER");
    }

    /// Quirk: a second aggregate array and a second proxy array, each
    /// failing before its array header (an empty chunk).
    ///
    /// Predicted before the run: the aggregates keep the first array (`G` on
    /// bone 1, max 2.0), since `Load_W3D` changes nothing before its header;
    /// the proxies are gone (`None`), since `read_proxy_array` releases the
    /// old array first. Each empty chunk's `Open_Chunk` fails at its end, so
    /// depth 1.
    #[test]
    fn second_failed_aggregate_keeps_first_but_second_failed_proxy_drops_it() {
        let mut body = header(b"H", b"S", 0);
        body.extend(array(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, 1, 2.0, &[(1, b"G")]));
        body.extend(array(W3D_CHUNK_HLOD_PROXY_ARRAY, 1, 0.0, &[(3, b"P")]));
        body.extend(chunk(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, &[]));
        body.extend(chunk(W3D_CHUNK_HLOD_PROXY_ARRAY, &[]));
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(d.lod_count, 0);
        assert!(d.lods.is_empty());
        assert_eq!((d.aggregates.model_count, d.aggregates.max_screen_size), (1, 2.0));
        assert_eq!(d.aggregates.sub_object(0), so(b"G", 1));
        assert_eq!(d.proxy_array, None);
    }

    /// Quirk and undefined cases of the names.
    ///
    /// Predicted before the run: a 16-character `Name` runs on into
    /// `HierarchyName` (`ABCDEFGHIJKLMNOPSKL`, hierarchy `SKL`); a
    /// 16-character `HierarchyName` has no 0 before the end of the header
    /// (`StringOverrun`); a 32-character proxy name is kept as 32 bytes
    /// (`strncpy`), while a 32-character sub-object name is `StringOverrun`.
    #[test]
    fn name_copying_semantics() {
        let (r, _) = load(&header(b"ABCDEFGHIJKLMNOP", b"SKL", 0));
        let d = r.unwrap().unwrap();
        assert_eq!(d.name, b"ABCDEFGHIJKLMNOPSKL");
        assert_eq!(d.hierarchy_tree_name, b"SKL");

        let (r, _) = load(&header(b"N", b"ABCDEFGHIJKLMNOP", 0));
        assert_eq!(r, Err(W3dUndefined::StringOverrun.into()));

        let long = [b'Q'; 32];
        let mut body = header(b"N", b"S", 0);
        body.extend(array(W3D_CHUNK_HLOD_PROXY_ARRAY, 1, 0.0, &[(1, &long)]));
        let (r, _) = load(&body);
        assert_eq!(r.unwrap().unwrap().proxy_array.unwrap().record(0).unwrap().name, long);

        let mut body = header(b"N", b"S", 1);
        body.extend(array(W3D_CHUNK_HLOD_LOD_ARRAY, 1, 0.0, &[(1, &long)]));
        let (r, _) = load(&body);
        assert_eq!(r, Err(W3dUndefined::StringOverrun.into()));
    }

    /// Load failures and allocations.
    ///
    /// Predicted before the run: a first chunk other than the header, a
    /// short header, and fewer LOD array chunks than `LodCount` (or a
    /// mis-typed one) are load failures (`Ok(None)`), the failing chunk left
    /// open (depth 2 for the mis-typed ones); `LodCount` 2^31 (negative) and
    /// 2^27 (`* 16 + 4` over 2^31 - 1) are `Allocation`, 2^27 - 1 is not (it
    /// fails on the missing LOD chunk instead); a `ModelCount` of 2^29 (`* 4`
    /// = 2^31) is `Allocation`; a proxy count of 0 allocates nothing.
    #[test]
    fn load_failures_and_allocations() {
        let (r, depth) = load(&array(W3D_CHUNK_HLOD_LOD_ARRAY, 0, 0.0, &[]));
        assert_eq!((r, depth), (Ok(None), 2));
        let (r, _) = load(&chunk(W3D_CHUNK_HLOD_HEADER, &[0; 39]));
        assert_eq!(r, Ok(None));

        let mut body = header(b"H", b"S", 2);
        body.extend(array(W3D_CHUNK_HLOD_LOD_ARRAY, 0, 0.0, &[]));
        let (r, depth) = load(&body);
        assert_eq!((r, depth), (Ok(None), 1));
        body.extend(array(W3D_CHUNK_HLOD_AGGREGATE_ARRAY, 0, 0.0, &[]));
        let (r, depth) = load(&body);
        assert_eq!((r, depth), (Ok(None), 2));

        for (count, alloc) in [(0x8000_0000u32, true), (0x0800_0000, true), (0x07FF_FFFF, false)] {
            let (r, _) = load(&header(b"H", b"S", count));
            let want = if alloc { Err(W3dUndefined::Allocation.into()) } else { Ok(None) };
            assert_eq!(r, want, "{count:#x}");
        }

        let mut body = header(b"H", b"S", 1);
        body.extend(array(W3D_CHUNK_HLOD_LOD_ARRAY, 0x2000_0000, 0.0, &[]));
        assert_eq!(load(&body).0, Err(W3dUndefined::Allocation.into()));

        let mut body = header(b"H", b"S", 0);
        body.extend(array(W3D_CHUNK_HLOD_PROXY_ARRAY, 0, 0.0, &[]));
        let p = load(&body).0.unwrap().unwrap().proxy_array.unwrap();
        assert_eq!((p.length, p.records.len()), (0, 0));
    }
}
