// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hmdldef.cpp, hmdldef.H
//   (HModelDefClass: constructor, Free, Load_W3D, read_connection;
//   HmdlNodeDefStruct)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/proto.cpp
//   (HModelLoaderClass::Load_W3D, HModelPrototypeClass's Get_Name and
//   Get_Class_ID)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, hmdldef by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The obsolete hierarchical models (`W3D_CHUNK_HMODEL`):
//! `HModelDefClass::Load_W3D`, which reads the definition the asset
//! manager's `HModelLoaderClass` turns into a prototype (upstream builds an
//! `HLodClass` from it; that render object is not translated here). GitHub
//! issue #17.
//!
//! # As upstream reads it
//!
//! Inside the hmodel chunk: a first chunk that must be
//! `W3D_CHUNK_HMODEL_HEADER` with a whole [`W3dHModelHeaderStruct`]. The
//! model name (`ModelName`) and the hierarchy name (`BasePoseName`) are its
//! two names through `strncpy(.., 16)` with the last byte forced to 0;
//! `Name` is a copy of the model name. `NumConnections` sizes the node array
//! (`SubObjects`). Then any chunks: `W3D_CHUNK_NODE`,
//! `W3D_CHUNK_COLLISION_NODE` and `W3D_CHUNK_SKIN_NODE` are each a
//! connection (`read_connection`: a [`W3dHModelNodeStruct`], named
//! `"<model>.<render object>"`, with its pivot), filling the array in order;
//! `W3D_CHUNK_POINTS` is a set of snap points
//! ([`super::collect::load_snap_points`]); anything else is skipped. Files
//! older than version 3.0 had no root pivot: their pivot indices are moved up
//! by one (`65535`, "no pivot", becomes the root, 0).
//!
//! # Upstream quirks that are kept
//!
//! - **An hmodel chunk with no sub-chunk loads.** `Load_W3D` returns
//!   `false` when the first `Open_Chunk` fails, and `false` is `OK` (0), so
//!   the prototype is made with the zeroed name (the game's allocator
//!   zero-fills the definition) and no nodes.
//! - On a header failure, or a connection chunk too short for its struct,
//!   the chunk stays open (see [`super::htree`]).
//! - The header's `NumConnections` only sizes the array: connections not
//!   found keep the allocator's zeros (an empty name, pivot 0).
//! - Every points chunk replaces the snap points (the old set leaks
//!   upstream); a set whose read fails part-way keeps the points read.
//! - Version 3.0 and later keep `PivotIdx` as it is, `65535` included (the
//!   `assert` is compiled out of the release).
//! - A connection's render-object name runs on into `PivotIdx`'s two bytes
//!   when its 16 bytes have no 0.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: more connection chunks than `NumConnections` (the node
//! is written past the array: [`W3dUndefined::IndexOutOfRange`]); a
//! connection name with no 0 in the struct's 18 bytes (`strcat` reads past
//! the struct: [`W3dUndefined::StringOverrun`]); a model name and
//! render-object name longer than the 32-byte `RenderObjName` holds
//! ([`W3dUndefined::StringOverflow`]).
//!
//! # Representation
//!
//! [`HModelDef`] holds what `Load_W3D` leaves; names are the bytes of the C
//! strings (before their terminator).

use super::collect::load_snap_points;
use super::w3d_file::*;
use super::{c_string, W3dResult, W3dUndefined};
use crate::ww::chunkio::ChunkLoad;

/// `RenderObjClass::CLASSID_HLOD`, `HModelPrototypeClass::Get_Class_ID()`.
pub const CLASSID_HLOD: u32 = 24;

/// `struct HmdlNodeDefStruct`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HmdlNodeDef {
    /// `RenderObjName` (`char[32]`): `"<model>.<render object>"`.
    pub render_obj_name: Vec<u8>,
    /// `PivotID`.
    pub pivot_id: i32,
}

/// `class HModelDefClass`: what `Load_W3D` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct HModelDef {
    /// `Name` (`Get_Name()`): the model name.
    pub name: Vec<u8>,
    /// `ModelName` (16 bytes, the last one 0).
    pub model_name: [u8; W3D_NAME_LEN],
    /// `BasePoseName` (16 bytes, the last one 0): the hierarchy.
    pub base_pose_name: [u8; W3D_NAME_LEN],
    /// `SubObjectCount`.
    pub sub_object_count: i32,
    /// `SubObjects`: the connections read, in order (at most
    /// `sub_object_count`; the others are [`HmdlNodeDef::default`]).
    pub sub_objects: Vec<HmdlNodeDef>,
    /// `SnapPoints` (`None` for a null pointer).
    pub snap_points: Option<Vec<[f32; 3]>>,
}

impl HModelDef {
    /// `HModelDefClass()` on the game's zeroed memory.
    fn new() -> Self {
        HModelDef {
            name: Vec::new(),
            model_name: [0; W3D_NAME_LEN],
            base_pose_name: [0; W3D_NAME_LEN],
            sub_object_count: 0,
            sub_objects: Vec::new(),
            snap_points: None,
        }
    }

    /// Node `i` (`SubObjects[i]`), `None` past `sub_object_count`.
    pub fn sub_object(&self, i: usize) -> Option<HmdlNodeDef> {
        if i >= self.sub_object_count.max(0) as usize {
            return None;
        }
        Some(self.sub_objects.get(i).cloned().unwrap_or_default())
    }

    /// `HModelPrototypeClass::Get_Class_ID()`.
    pub fn class_id(&self) -> u32 {
        CLASSID_HLOD
    }
}

/// `strncpy(dst, src, W3D_NAME_LEN); dst[W3D_NAME_LEN - 1] = 0`.
fn name16(src: &[u8; W3D_NAME_LEN]) -> [u8; W3D_NAME_LEN] {
    let mut d = [0u8; W3D_NAME_LEN];
    for (d, &c) in d.iter_mut().zip(src.iter()) {
        if c == 0 {
            break;
        }
        *d = c;
    }
    d[W3D_NAME_LEN - 1] = 0;
    d
}

fn c_str16(n: &[u8; W3D_NAME_LEN]) -> &[u8] {
    &n[..n.iter().position(|&c| c == 0).unwrap_or(W3D_NAME_LEN)]
}

/// `HModelDefClass::Load_W3D(cload)` (as `HModelLoaderClass::Load_W3D`
/// calls it on a new definition), with `cload` inside the hmodel chunk.
/// `Ok(None)` is upstream's `LOAD_ERROR`.
pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<HModelDef>> {
    let mut def = HModelDef::new();
    // Read the first chunk, it should be the header
    if !cload.open_chunk()? {
        return Ok(Some(def)); // `return false`, which is OK
    }
    if cload.cur_chunk_id()? != W3D_CHUNK_HMODEL_HEADER {
        return Ok(None);
    }
    let mut hb = [0u8; W3dHModelHeaderStruct::SIZE];
    if cload.read_into(&mut hb)? != W3dHModelHeaderStruct::SIZE as u32 {
        return Ok(None);
    }
    cload.close_chunk()?;
    let header = W3dHModelHeaderStruct::from_bytes(&hb);

    // process the header info
    def.model_name = name16(&header.name);
    def.base_pose_name = name16(&header.hierarchy_name);
    def.name = c_str16(&def.model_name).to_vec();
    // SubObjects = new HmdlNodeDefStruct[SubObjectCount] (at most 65535)
    def.sub_object_count = i32::from(header.num_connections);
    let pre30 = header.version < w3d_make_version(3, 0);

    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            W3D_CHUNK_NODE | W3D_CHUNK_COLLISION_NODE | W3D_CHUNK_SKIN_NODE => {
                let Some(node) = read_connection(cload, &def.model_name, pre30, def.sub_objects.len(), def.sub_object_count)? else {
                    return Ok(None);
                };
                def.sub_objects.push(node);
            }
            W3D_CHUNK_POINTS => def.snap_points = Some(load_snap_points(cload)?),
            _ => {}
        }
        cload.close_chunk()?;
    }
    Ok(Some(def))
}

/// `HModelDefClass::read_connection(cload, &SubObjects[index], pre30)`:
/// `None` for upstream's `false`.
fn read_connection(
    cload: &mut ChunkLoad<'_>,
    model_name: &[u8; W3D_NAME_LEN],
    pre30: bool,
    index: usize,
    count: i32,
) -> W3dResult<Option<HmdlNodeDef>> {
    let mut cb = [0u8; W3dHModelNodeStruct::SIZE];
    if cload.read_into(&mut cb)? != W3dHModelNodeStruct::SIZE as u32 {
        return Ok(None);
    }
    let con = W3dHModelNodeStruct::from_bytes(&cb);
    // the node is SubObjects[subobjcounter]
    if index >= count.max(0) as usize {
        return Err(W3dUndefined::IndexOutOfRange.into());
    }
    // strcpy(RenderObjName, ModelName); strcat("."); strcat(con.RenderObjName)
    let mut name = c_str16(model_name).to_vec();
    name.push(b'.');
    name.extend_from_slice(c_string(&cb, 0)?);
    if name.len() + 1 > 2 * W3D_NAME_LEN {
        return Err(W3dUndefined::StringOverflow.into());
    }
    let pivot_id = if pre30 {
        if con.pivot_idx == 65535 {
            0
        } else {
            i32::from(con.pivot_idx) + 1
        }
    } else {
        i32::from(con.pivot_idx)
    };
    Ok(Some(HmdlNodeDef { render_obj_name: name, pivot_id }))
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

    fn header(version: u32, name: &[u8], hier: &[u8], n: u16) -> Vec<u8> {
        let mut h = W3dHModelHeaderStruct::from_bytes(&[0; 40]);
        h.version = version;
        h.name[..name.len()].copy_from_slice(name);
        h.hierarchy_name[..hier.len()].copy_from_slice(hier);
        h.num_connections = n;
        chunk(W3D_CHUNK_HMODEL_HEADER, &h.to_bytes())
    }

    fn node(id: u32, name: &[u8], pivot: u16) -> Vec<u8> {
        let mut n = W3dHModelNodeStruct::from_bytes(&[0; 18]);
        n.render_obj_name[..name.len()].copy_from_slice(name);
        n.pivot_idx = pivot;
        chunk(id, &n.to_bytes())
    }

    fn load(body: &[u8]) -> (W3dResult<Option<HModelDef>>, i32) {
        let data = chunk(W3D_CHUNK_HMODEL, body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let r = load_w3d(&mut c);
        (r, c.cur_chunk_depth())
    }

    /// A current (4.2) model with the three node kinds, a points chunk and
    /// an unknown chunk.
    ///
    /// Predicted before the run: name `TANK`, base pose `TANKSKL`; nodes
    /// `TANK.BODY` pivot 1, `TANK.TURRET` pivot 65535 (kept, no `+1`),
    /// `TANK.BOX` pivot 3; one snap point; count 3; depth 1.
    #[test]
    fn loads_current_model() {
        let mut body = header(W3D_CURRENT_HMODEL_VERSION, b"TANK", b"TANKSKL", 3);
        body.extend(node(W3D_CHUNK_NODE, b"BODY", 1));
        body.extend(chunk(0x1234, &[1, 2, 3]));
        body.extend(node(W3D_CHUNK_SKIN_NODE, b"TURRET", 65535));
        body.extend(node(W3D_CHUNK_COLLISION_NODE, b"BOX", 3));
        body.extend(chunk(W3D_CHUNK_POINTS, &[0, 0, 128, 63, 0, 0, 0, 64, 0, 0, 64, 64]));
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(d.name, b"TANK");
        assert_eq!(c_str16(&d.base_pose_name), b"TANKSKL");
        assert_eq!(d.sub_object_count, 3);
        let names: Vec<(&[u8], i32)> = d.sub_objects.iter().map(|n| (&n.render_obj_name[..], n.pivot_id)).collect();
        assert_eq!(names, vec![(&b"TANK.BODY"[..], 1), (&b"TANK.TURRET"[..], 65535), (&b"TANK.BOX"[..], 3)]);
        assert_eq!(d.snap_points, Some(vec![[1.0, 2.0, 3.0]]));
        assert_eq!(d.class_id(), CLASSID_HLOD);
    }

    /// Quirks: a pre-3.0 file (pivots moved up, 65535 to the root), a
    /// 16-character model name cut to 15, fewer nodes than
    /// `NumConnections`, and an empty hmodel chunk.
    ///
    /// Predicted before the run: name `ABCDEFGHIJKLMNO` (15); the one node
    /// `ABCDEFGHIJKLMNO.X` (17 characters) pivot 0 (from 65535), a second
    /// `...Y` pivot 5 (from 4); entry 2 of 3 is the zeroed default; the
    /// empty chunk loads (`false` is `OK`) with an empty name and no nodes.
    #[test]
    fn pre30_short_and_empty() {
        let mut body = header(w3d_make_version(2, 0), b"ABCDEFGHIJKLMNOP", b"S", 3);
        body.extend(node(W3D_CHUNK_NODE, b"X", 65535));
        body.extend(node(W3D_CHUNK_NODE, b"Y", 4));
        let d = load(&body).0.unwrap().unwrap();
        assert_eq!(d.name, b"ABCDEFGHIJKLMNO");
        assert_eq!(d.sub_object(0).unwrap(), HmdlNodeDef { render_obj_name: b"ABCDEFGHIJKLMNO.X".to_vec(), pivot_id: 0 });
        assert_eq!(d.sub_object(1).unwrap().pivot_id, 5);
        assert_eq!(d.sub_object(2), Some(HmdlNodeDef::default()));
        assert_eq!(d.sub_object(3), None);
        let (r, depth) = load(&[]);
        let d = r.unwrap().unwrap();
        assert_eq!((d.name.len(), d.sub_object_count, depth), (0, 0, 1));
    }

    /// Load failures and undefined inputs.
    ///
    /// Predicted before the run: a first chunk other than the header, a
    /// short header and a short node are load errors with the chunk left
    /// open (depth 2); a fourth node for `NumConnections` 3 is
    /// `IndexOutOfRange`, as is any node with `NumConnections` 0; a
    /// 16-character node name runs into `PivotIdx`: with pivot 0x4241 (two
    /// nonzero bytes, then the end of the struct) it is `StringOverrun`,
    /// with pivot 0x0041 the name is the 16 letters and `A`; a
    /// 15-character model name with a 15-character node name
    /// (32 bytes with the terminator) fits, 16 + `A` does not
    /// (`StringOverflow`).
    #[test]
    fn failures_and_undefined() {
        assert_eq!(load(&node(W3D_CHUNK_NODE, b"A", 0)), (Ok(None), 2));
        assert_eq!(load(&chunk(W3D_CHUNK_HMODEL_HEADER, &[0; 39])), (Ok(None), 2));
        let mut body = header(W3D_CURRENT_HMODEL_VERSION, b"M", b"S", 1);
        body.extend(chunk(W3D_CHUNK_NODE, &[0; 17]));
        assert_eq!(load(&body), (Ok(None), 2));
        let mut body = header(W3D_CURRENT_HMODEL_VERSION, b"M", b"S", 0);
        body.extend(node(W3D_CHUNK_NODE, b"A", 0));
        assert_eq!(load(&body).0, Err(W3dError::Undefined(W3dUndefined::IndexOutOfRange)));
        let mut body = header(W3D_CURRENT_HMODEL_VERSION, b"M", b"S", 1);
        body.extend(node(W3D_CHUNK_NODE, b"ABCDEFGHIJKLMNOP", 0x4241));
        assert_eq!(load(&body).0, Err(W3dError::Undefined(W3dUndefined::StringOverrun)));
        let mut body = header(W3D_CURRENT_HMODEL_VERSION, b"M", b"S", 1);
        body.extend(node(W3D_CHUNK_NODE, b"ABCDEFGHIJKLMNOP", 0x0041));
        let d = load(&body).0.unwrap().unwrap();
        assert_eq!(d.sub_objects[0].render_obj_name, b"M.ABCDEFGHIJKLMNOPA");
        let mut body = header(W3D_CURRENT_HMODEL_VERSION, b"ABCDEFGHIJKLMNO", b"S", 2);
        body.extend(node(W3D_CHUNK_NODE, b"ABCDEFGHIJKLMNO", 0));
        assert!(load(&body).0.unwrap().is_some());
        body.extend(node(W3D_CHUNK_NODE, b"ABCDEFGHIJKLMNOP", 0x0041));
        assert_eq!(load(&body).0, Err(W3dError::Undefined(W3dUndefined::StringOverflow)));
    }
}
