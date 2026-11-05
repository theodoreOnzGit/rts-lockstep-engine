// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/collect.cpp
//   (CollectionDefClass: constructor, Free, Load)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/snappts.cpp, snapPts.h
//   (SnapPointsClass::Load_W3D)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/proxy.h (ProxyClass's data)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, collect and snappts by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D collections (`W3D_CHUNK_COLLECTION`): `CollectionDefClass::Load`,
//! which reads the definition a `CollectionClass` is later built from: a
//! name, the names of the render objects it holds, placeholders ("proxies":
//! a name and a transform) and snap points. Only the loading; the render
//! object (`CollectionClass`) is not translated.
//!
//! # As upstream reads it
//!
//! Inside the collection chunk: a first chunk that must be
//! `W3D_CHUNK_COLLECTION_HEADER` with a whole [`W3dCollectionHeaderStruct`],
//! then any chunks: each `W3D_CHUNK_COLLECTION_OBJ_NAME` is an object name
//! (the whole chunk), each `W3D_CHUNK_PLACEHOLDER` a [`W3dPlaceholderStruct`]
//! followed by `name_len` name bytes, each `W3D_CHUNK_POINTS` a new set of
//! snap points (`SnapPointsClass::Load_W3D`: `length / 12`
//! [`W3dVectorStruct`]s). Anything else is skipped, including
//! `W3D_CHUNK_TRANSFORM_NODE`, which `collect.cpp` does not read. The load
//! fails ([`Ok(None)`](load_w3d)) only on the header.
//!
//! # Upstream quirks that are kept
//!
//! - On a header failure the header chunk stays open (see [`super::htree`]).
//! - `Name` is the header's 16 bytes through `strncpy`: zero-padded after
//!   the first 0, unterminated at 16 characters.
//! - An object name is the chunk's bytes as read into a buffer of the
//!   chunk's length ([`ObjectName`]): not terminated unless the file put a
//!   0 in it. The read is not checked: a chunk longer than what is left of
//!   the file keeps what the file had, then the allocator's zeros.
//! - A placeholder's name read is not checked either: its buffer is
//!   `name_len + 1` zeroed bytes, of which the file fills what it can (none
//!   if `name_len` passes the end of the chunk). Its transform is the
//!   struct's `transform[4][3]` transposed (row `r` is `transform[0..4][r]`).
//! - Every `W3D_CHUNK_POINTS` replaces the snap points; a set whose read
//!   fails part-way keeps the points read (its load error is ignored).
//! - The header's `RenderObjectCount` only sizes the name array; the names
//!   are the chunks found.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: a placeholder chunk too short for its struct (the
//! uninitialised struct is used: [`W3dUndefined::UninitialisedRead`]); a
//! `name_len` of `0xFFFFFFFF` (`new char[0]`, then `name[0xFFFFFFFF] = 0`:
//! [`W3dUndefined::IndexOutOfRange`]); arrays the 32-bit original cannot
//! allocate, or a negative count ([`W3dUndefined::Allocation`]): a
//! `RenderObjectCount` that is negative as an `int` or whose `* 4` passes
//! 2^31 - 1, an object-name chunk or a placeholder name (`name_len + 1`)
//! over 2^31 - 1 bytes, snap points whose `* 12` passes 2^31 - 1.
//!
//! # Representation
//!
//! [`CollectionDef`] holds what `Load` leaves: the name, the object names in
//! order, the placeholders in order (`ProxyList`) and the last snap points
//! (`None` if no points chunk was read). Buffers keep only the bytes the
//! file supplied ([`ObjectName::bytes`]); the rest of their length is zeros.

use super::w3d_file::*;
use super::{read_partial, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::chunkio::ChunkLoad;

/// One of `ObjectNames`: a `char` array of the chunk's length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectName {
    /// The array's length (`Cur_Chunk_Length()`).
    pub len: u32,
    /// Its first bytes, as read (at most `len`; the rest are zeros).
    pub bytes: Vec<u8>,
}

impl ObjectName {
    /// The name as a C string: up to the first 0 in the array, or `None` if
    /// the array has none (the string would run past it).
    pub fn c_str(&self) -> Option<&[u8]> {
        match self.bytes.iter().position(|&c| c == 0) {
            Some(n) => Some(&self.bytes[..n]),
            None if (self.bytes.len() as u64) < u64::from(self.len) => Some(&self.bytes),
            None => None,
        }
    }
}

/// `ProxyClass`: a placeholder.
#[derive(Clone, Debug, PartialEq)]
pub struct Proxy {
    /// `Name` (a `StringClass` copy of the C string).
    pub name: Vec<u8>,
    /// `Transform`: the three rows of the `Matrix3D`.
    pub transform: [[f32; 4]; 3],
}

/// `class CollectionDefClass`: what `Load` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectionDef {
    /// `Name` (16 bytes, as `strncpy` leaves them).
    pub name: [u8; W3D_NAME_LEN],
    /// `ObjectNames`.
    pub object_names: Vec<ObjectName>,
    /// `SnapPoints` (`None` for a null pointer): the points, as read.
    pub snap_points: Option<Vec<[f32; 3]>>,
    /// `ProxyList`.
    pub proxies: Vec<Proxy>,
}

impl CollectionDef {
    /// `Get_Name()` up to its terminator (16 bytes if there is none).
    pub fn name_str(&self) -> &[u8] {
        let n = self.name.iter().position(|&c| c == 0).unwrap_or(W3D_NAME_LEN);
        &self.name[..n]
    }
}

/// `CollectionDefClass::Load(cload)`, with `cload` inside the collection
/// chunk. `Ok(None)` is upstream's `WW3D_ERROR_LOAD_FAILED`.
pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<CollectionDef>> {
    // open the header chunk and read it in
    if !cload.open_chunk()? {
        return Ok(None);
    }
    if cload.cur_chunk_id()? != W3D_CHUNK_COLLECTION_HEADER {
        return Ok(None);
    }
    let mut hb = [0u8; W3dCollectionHeaderStruct::SIZE];
    if cload.read_into(&mut hb)? != W3dCollectionHeaderStruct::SIZE as u32 {
        return Ok(None);
    }
    if !cload.close_chunk()? {
        return Ok(None);
    }
    let header = W3dCollectionHeaderStruct::from_bytes(&hb);

    // strncpy(Name, header.Name, W3D_NAME_LEN)
    let mut name = [0u8; W3D_NAME_LEN];
    for (d, &c) in name.iter_mut().zip(header.name.iter()) {
        if c == 0 {
            break;
        }
        *d = c;
    }
    // ObjectNames.Resize(header.RenderObjectCount): new char*[n] for n != 0
    let count = header.render_object_count as i32;
    if count < 0 || count as u64 * 4 > WIN32_MAX_ALLOCATION {
        return Err(W3dUndefined::Allocation.into());
    }
    let mut def = CollectionDef { name, object_names: Vec::new(), snap_points: None, proxies: Vec::new() };

    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            W3D_CHUNK_COLLECTION_OBJ_NAME => {
                let len = cload.cur_chunk_length()?;
                if u64::from(len) > WIN32_MAX_ALLOCATION {
                    return Err(W3dUndefined::Allocation.into());
                }
                // the read is not checked
                let (_, bytes) = read_partial(cload, len)?;
                def.object_names.push(ObjectName { len, bytes });
            }
            W3D_CHUNK_PLACEHOLDER => {
                // Read the placeholder information from the chunk (unchecked)
                let mut ib = [0u8; W3dPlaceholderStruct::SIZE];
                if cload.read_into(&mut ib)? != W3dPlaceholderStruct::SIZE as u32 {
                    return Err(W3dUndefined::UninitialisedRead.into());
                }
                let info = W3dPlaceholderStruct::from_bytes(&ib);
                // char *name = new char[info.name_len + 1]
                let name_len = info.name_len;
                if name_len == u32::MAX {
                    return Err(W3dUndefined::IndexOutOfRange.into());
                }
                if u64::from(name_len) + 1 > WIN32_MAX_ALLOCATION {
                    return Err(W3dUndefined::Allocation.into());
                }
                let (_, bytes) = read_partial(cload, name_len)?;
                let n = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
                let t = info.transform;
                let transform = [0, 1, 2].map(|r| [t[0][r], t[1][r], t[2][r], t[3][r]]);
                def.proxies.push(Proxy { name: bytes[..n].to_vec(), transform });
            }
            W3D_CHUNK_POINTS => {
                def.snap_points = Some(load_snap_points(cload)?);
            }
            _ => {}
        }
        cload.close_chunk()?;
    }
    Ok(Some(def))
}

/// `SnapPointsClass::Load_W3D(cload)` on a new set, with `cload` inside the
/// points chunk: the points read (its result is ignored by the caller).
fn load_snap_points(cload: &mut ChunkLoad<'_>) -> W3dResult<Vec<[f32; 3]>> {
    // int size = Cur_Chunk_Length(); int count = size / sizeof(W3dVectorStruct)
    // (sizeof is an unsigned int on Win32: the division is unsigned)
    let count = cload.cur_chunk_length()? / W3dVectorStruct::SIZE as u32;
    if u64::from(count) * W3dVectorStruct::SIZE as u64 > WIN32_MAX_ALLOCATION {
        return Err(W3dUndefined::Allocation.into());
    }
    let mut points = Vec::new();
    for _ in 0..count {
        let mut vb = [0u8; W3dVectorStruct::SIZE];
        if cload.read_into(&mut vb)? != W3dVectorStruct::SIZE as u32 {
            break;
        }
        let v = W3dVectorStruct::from_bytes(&vb);
        points.push([v.x, v.y, v.z]);
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::W3dError;
    use crate::ww::chunkio::GameFile;

    fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn header(name: &[u8], count: u32) -> Vec<u8> {
        let mut h = W3dCollectionHeaderStruct::from_bytes(&[0; 32]);
        h.version = W3D_CURRENT_COLLECTION_VERSION;
        h.name[..name.len()].copy_from_slice(name);
        h.render_object_count = count;
        chunk(W3D_CHUNK_COLLECTION_HEADER, &h.to_bytes())
    }

    fn placeholder(transform: [[f32; 3]; 4], name_len: u32, name: &[u8]) -> Vec<u8> {
        let mut p = W3dPlaceholderStruct::from_bytes(&[0; 56]);
        p.version = W3D_CURRENT_PLACEHOLDER_VERSION;
        p.transform = transform;
        p.name_len = name_len;
        let mut b = p.to_bytes();
        b.extend_from_slice(name);
        chunk(W3D_CHUNK_PLACEHOLDER, &b)
    }

    fn points(p: &[[f32; 3]]) -> Vec<u8> {
        let b: Vec<u8> = p.iter().flat_map(|v| v.iter().flat_map(|f| f.to_le_bytes())).collect();
        chunk(W3D_CHUNK_POINTS, &b)
    }

    fn load(body: &[u8]) -> (W3dResult<Option<CollectionDef>>, i32) {
        let data = chunk(W3D_CHUNK_COLLECTION, body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let r = load_w3d(&mut c);
        (r, c.cur_chunk_depth())
    }

    /// A collection with two object names, a placeholder, a transform node
    /// (skipped) and snap points.
    ///
    /// Predicted before the run: name `COLL`; object names `MESH1\0` (len 6)
    /// and `MESH2\0` (len 6); one proxy `PH` whose transform rows are the
    /// struct's columns (row 0 = 1, 4, 7, 10; row 1 = 2, 5, 8, 11; row 2 = 3,
    /// 6, 9, 12); snap points (1,2,3) and (4,5,6); depth 1.
    #[test]
    fn loads_names_placeholders_and_points() {
        let mut body = header(b"COLL", 2);
        body.extend(chunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"MESH1\0"));
        body.extend(chunk(W3D_CHUNK_TRANSFORM_NODE, &[0; 60]));
        body.extend(chunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"MESH2\0"));
        let t = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0], [10.0, 11.0, 12.0]];
        body.extend(placeholder(t, 2, b"PH"));
        body.extend(points(&[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]));
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(d.name_str(), b"COLL");
        assert_eq!(d.object_names.len(), 2);
        assert_eq!(d.object_names[0], ObjectName { len: 6, bytes: b"MESH1\0".to_vec() });
        assert_eq!(d.object_names[1].c_str(), Some(&b"MESH2"[..]));
        assert_eq!(d.proxies.len(), 1);
        assert_eq!(d.proxies[0].name, b"PH");
        assert_eq!(
            d.proxies[0].transform,
            [[1.0, 4.0, 7.0, 10.0], [2.0, 5.0, 8.0, 11.0], [3.0, 6.0, 9.0, 12.0]]
        );
        assert_eq!(d.snap_points, Some(vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]));
    }

    /// Quirks: an unterminated object name, a placeholder whose `name_len`
    /// passes the end of its chunk, a second points chunk with a trailing
    /// partial point, a 16-character name, and a `RenderObjectCount` that
    /// does not match the names.
    ///
    /// Predicted before the run: name all 16 bytes; the object name `ABC`
    /// (len 3) has no C string (`None`); the placeholder's name read fails
    /// the chunk bound, so its name is empty; the second points chunk (14
    /// bytes: one point and 2 extra bytes) replaces the first: one point;
    /// three object names despite a count of 1. All with depth 1.
    #[test]
    fn unchecked_reads_and_replaced_points() {
        let mut body = header(b"ABCDEFGHIJKLMNOP", 1);
        body.extend(chunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"ABC"));
        body.extend(chunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"X\0"));
        body.extend(chunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b""));
        body.extend(placeholder([[0.0; 3]; 4], 10, b"SHORT"));
        body.extend(points(&[[1.0, 1.0, 1.0], [2.0, 2.0, 2.0]]));
        let mut p2 = points(&[[7.0, 8.0, 9.0]]);
        p2[4] = 14;
        p2.extend_from_slice(&[0xAA, 0xBB]);
        body.extend(p2);
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(&d.name, b"ABCDEFGHIJKLMNOP");
        assert_eq!(d.name_str(), b"ABCDEFGHIJKLMNOP");
        assert_eq!(d.object_names.len(), 3);
        assert_eq!(d.object_names[0].c_str(), None);
        assert_eq!(d.object_names[1].c_str(), Some(&b"X"[..]));
        assert_eq!(d.object_names[2], ObjectName { len: 0, bytes: vec![] });
        assert_eq!(d.object_names[2].c_str(), None);
        assert_eq!(d.proxies[0].name, b"");
        assert_eq!(d.snap_points, Some(vec![[7.0, 8.0, 9.0]]));
    }

    /// Quirk: an object-name chunk longer than the file. The read fails but
    /// the file's bytes stay in the buffer.
    ///
    /// Predicted before the run: the load succeeds with one object name of
    /// len 100 holding the 4 bytes `ABCD` (then zeros), so its C string is
    /// `ABCD`; the name chunk is closed as usual (depth 1).
    #[test]
    fn short_file_keeps_partial_name() {
        let mut body = header(b"C", 0);
        let mut n = chunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"ABCD");
        n[4] = 100;
        body.extend(n);
        let mut data = W3D_CHUNK_COLLECTION.to_le_bytes().to_vec();
        data.extend_from_slice(&(body.len() as u32 + 96).to_le_bytes());
        data.extend(body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let d = load_w3d(&mut c).unwrap().unwrap();
        assert_eq!(c.cur_chunk_depth(), 1);
        assert_eq!(d.object_names, vec![ObjectName { len: 100, bytes: b"ABCD".to_vec() }]);
        assert_eq!(d.object_names[0].c_str(), Some(&b"ABCD"[..]));
    }

    /// Load failures and undefined inputs.
    ///
    /// Predicted before the run: a first chunk other than the header and a
    /// short header are load failures with the chunk left open (depth 2); a
    /// placeholder chunk shorter than its 56-byte struct is
    /// `UninitialisedRead`; `name_len` 0xFFFFFFFF is `IndexOutOfRange`;
    /// `RenderObjectCount` 0x80000000 and 0x20000000 are `Allocation`,
    /// 0x1FFFFFFF is not.
    #[test]
    fn load_failures_and_undefined() {
        assert_eq!(load(&chunk(W3D_CHUNK_COLLECTION_OBJ_NAME, b"A\0")), (Ok(None), 2));
        assert_eq!(load(&chunk(W3D_CHUNK_COLLECTION_HEADER, &[0; 31])), (Ok(None), 2));
        let mut body = header(b"C", 0);
        body.extend(chunk(W3D_CHUNK_PLACEHOLDER, &[0; 55]));
        assert_eq!(load(&body).0, Err(W3dError::Undefined(W3dUndefined::UninitialisedRead)));
        let mut body = header(b"C", 0);
        body.extend(placeholder([[0.0; 3]; 4], u32::MAX, b""));
        assert_eq!(load(&body).0, Err(W3dError::Undefined(W3dUndefined::IndexOutOfRange)));
        for (count, undefined) in [(0x8000_0000u32, true), (0x2000_0000, true), (0x1FFF_FFFF, false)] {
            let r = load(&header(b"C", count)).0;
            assert_eq!(r.is_err(), undefined, "{count:#x}");
        }
    }
}
