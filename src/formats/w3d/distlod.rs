// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/distlod.cpp, distlod.h
//   (DistLODLoaderClass::Load_W3D; DistLODDefClass: constructor, Free,
//   Load_W3D, read_header; DistLODNodeDefStruct; DistLODPrototypeClass's
//   Get_Name and Get_Class_ID)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/nstrdup.cpp (nstrdup)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, distlod by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The obsolete distance LOD models (`W3D_CHUNK_LODMODEL`):
//! `DistLODDefClass::Load_W3D`, which reads the definition the asset
//! manager's `DistLODLoaderClass` keeps in a `DistLODPrototypeClass`
//! (whose `Create` turns it into an `HLodClass`, "OBSOLETE Dist-LOD model
//! found! Please re-export"; not translated here). GitHub issue #17.
//!
//! # As upstream reads it
//!
//! Inside the LOD-model chunk: a first chunk that must be
//! `W3D_CHUNK_LODMODEL_HEADER` with a whole [`W3dLODModelHeaderStruct`]
//! (`read_header`): its name (`nstrdup`) and `NumLODs`, which sizes the LOD
//! array. Then exactly `NumLODs` chunks, each of which must be
//! `W3D_CHUNK_LOD` with a whole [`W3dLODStruct`]: the render object's name
//! and two distances. Chunks after them are not read.
//!
//! # Upstream quirks that are kept
//!
//! - `ResUpDist` is the struct's `LODMin` and `ResDownDist` its `LODMax`.
//! - On a failure the chunks opened so far stay open (see
//!   [`super::htree`]): a header or LOD chunk of the wrong type or too
//!   short is left open.
//! - Names run on past their fields: a 16-character model name continues
//!   into `NumLODs` and the header's two padding bytes, a 32-character LOD
//!   name into `LODMin` and `LODMax` (all bytes the file supplied), to the
//!   first 0.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined::StringOverrun`]: a name with no 0 before the end of its
//! struct (`nstrdup` reads past it).
//!
//! # Representation
//!
//! [`DistLodDef`] holds what a successful `Load_W3D` leaves (on a failure the
//! definition is deleted); names are the C strings' bytes.

use super::w3d_file::*;
use super::{c_string, W3dResult};
use crate::ww::chunkio::ChunkLoad;

/// `RenderObjClass::CLASSID_DISTLOD`, `DistLODPrototypeClass::Get_Class_ID()`.
pub const CLASSID_DISTLOD: u32 = 2;

/// `struct DistLODNodeDefStruct`.
#[derive(Clone, Debug, PartialEq)]
pub struct DistLodNodeDef {
    /// `Name`.
    pub name: Vec<u8>,
    /// `ResDownDist` (the struct's `LODMax`).
    pub res_down_dist: f32,
    /// `ResUpDist` (the struct's `LODMin`).
    pub res_up_dist: f32,
}

/// `class DistLODDefClass`.
#[derive(Clone, Debug, PartialEq)]
pub struct DistLodDef {
    /// `Name` (`Get_Name()`).
    pub name: Vec<u8>,
    /// `LodCount`.
    pub lod_count: i32,
    /// `Lods`, all `lod_count` of them.
    pub lods: Vec<DistLodNodeDef>,
}

impl DistLodDef {
    /// `DistLODPrototypeClass::Get_Class_ID()`.
    pub fn class_id(&self) -> u32 {
        CLASSID_DISTLOD
    }
}

/// `DistLODDefClass::Load_W3D(cload)` on a new definition, with `cload`
/// inside the LOD-model chunk. `Ok(None)` is upstream's
/// `WW3D_ERROR_LOAD_FAILED`.
pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<DistLodDef>> {
    // read_header: open the first chunk, it should be the LOD header
    if !cload.open_chunk()? {
        return Ok(None);
    }
    if cload.cur_chunk_id()? != W3D_CHUNK_LODMODEL_HEADER {
        return Ok(None);
    }
    let mut hb = [0u8; W3dLODModelHeaderStruct::SIZE];
    if cload.read_into(&mut hb)? != W3dLODModelHeaderStruct::SIZE as u32 {
        return Ok(None);
    }
    cload.close_chunk()?;
    let header = W3dLODModelHeaderStruct::from_bytes(&hb);
    // Name = nstrdup(lodHeader.Name); Lods = new DistLODNodeDefStruct[LodCount]
    let name = c_string(&hb, 4)?.to_vec();
    let lod_count = i32::from(header.num_lods);
    let mut lods = Vec::new();

    // Loop through all the LODs and read the info from its chunk
    for _ in 0..lod_count {
        if !cload.open_chunk()? {
            return Ok(None);
        }
        if cload.cur_chunk_id()? != W3D_CHUNK_LOD {
            return Ok(None); // ERROR: Expected LOD struct!
        }
        let mut lb = [0u8; W3dLODStruct::SIZE];
        if cload.read_into(&mut lb)? != W3dLODStruct::SIZE as u32 {
            return Ok(None);
        }
        let lod = W3dLODStruct::from_bytes(&lb);
        lods.push(DistLodNodeDef { name: c_string(&lb, 0)?.to_vec(), res_down_dist: lod.lod_max, res_up_dist: lod.lod_min });
        cload.close_chunk()?;
    }
    Ok(Some(DistLodDef { name, lod_count, lods }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::{W3dError, W3dUndefined};
    use crate::ww::chunkio::GameFile;

    fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn header(name: &[u8], n: u16) -> Vec<u8> {
        let mut h = W3dLODModelHeaderStruct::from_bytes(&[0; 24]);
        h.name[..name.len()].copy_from_slice(name);
        h.num_lods = n;
        chunk(W3D_CHUNK_LODMODEL_HEADER, &h.to_bytes())
    }

    fn lod(name: &[u8], min: f32, max: f32) -> Vec<u8> {
        let mut l = W3dLODStruct::from_bytes(&[0; 40]);
        l.render_obj_name[..name.len()].copy_from_slice(name);
        l.lod_min = min;
        l.lod_max = max;
        chunk(W3D_CHUNK_LOD, &l.to_bytes())
    }

    fn load(body: &[u8]) -> (W3dResult<Option<DistLodDef>>, i32) {
        let data = chunk(W3D_CHUNK_LODMODEL, body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let r = load_w3d(&mut c);
        (r, c.cur_chunk_depth())
    }

    /// Two LODs and a trailing chunk that is not read.
    ///
    /// Predicted before the run: name `TREE`; LODs `TREE_HI` (up 0, down
    /// 50) and `TREE_LO` (up 50, down 1000), the struct's min as `ResUpDist`;
    /// depth 1.
    #[test]
    fn loads_two_lods() {
        let mut body = header(b"TREE", 2);
        body.extend(lod(b"TREE_HI", 0.0, 50.0));
        body.extend(lod(b"TREE_LO", 50.0, 1000.0));
        body.extend(chunk(0x99, &[1, 2]));
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(d.name, b"TREE");
        assert_eq!(d.lod_count, 2);
        assert_eq!(d.lods[0], DistLodNodeDef { name: b"TREE_HI".to_vec(), res_down_dist: 50.0, res_up_dist: 0.0 });
        assert_eq!((d.lods[1].res_up_dist, d.lods[1].res_down_dist), (50.0, 1000.0));
        assert_eq!(d.class_id(), CLASSID_DISTLOD);
    }

    /// Run-on names, failures and the undefined overrun.
    ///
    /// Predicted before the run: a 16-character name with `NumLODs` 0
    /// stops at `NumLODs`' first byte (0): the 16 letters; with `NumLODs`
    /// 0x4141 it runs into the padding (0): 16 letters and `AA`, then the
    /// load fails for want of LOD chunks (depth 1, nothing open); a
    /// 32-character LOD name with `LODMin` 0 is the 32 letters; one whose
    /// floats have no 0 byte (both 0x41414141) is `StringOverrun`; a short LOD chunk
    /// and a wrong one fail with the chunk open (depth 2); a short header
    /// too.
    #[test]
    fn run_on_names_and_failures() {
        let d = load(&header(b"ABCDEFGHIJKLMNOP", 0)).0.unwrap().unwrap();
        assert_eq!(d.name, b"ABCDEFGHIJKLMNOP");
        assert_eq!(load(&header(b"ABCDEFGHIJKLMNOP", 0x4141)), (Ok(None), 1));
        let mut body = header(b"M", 1);
        body.extend(lod(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345", 0.0, 1.0));
        let d = load(&body).0.unwrap().unwrap();
        assert_eq!(d.lods[0].name, b"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345");
        let a = f32::from_bits(0x4141_4141);
        let mut body = header(b"M", 1);
        body.extend(lod(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345", a, a));
        assert_eq!(load(&body).0, Err(W3dError::Undefined(W3dUndefined::StringOverrun)));
        let mut body = header(b"M", 1);
        body.extend(chunk(W3D_CHUNK_LOD, &[0; 39]));
        assert_eq!(load(&body), (Ok(None), 2));
        let mut body = header(b"M", 1);
        body.extend(chunk(W3D_CHUNK_LODMODEL_HEADER, &[0; 40]));
        assert_eq!(load(&body), (Ok(None), 2));
        assert_eq!(load(&chunk(W3D_CHUNK_LODMODEL_HEADER, &[0; 23])), (Ok(None), 2));
    }
}
