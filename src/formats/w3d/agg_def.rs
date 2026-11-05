// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/agg_def.cpp
//   (AggregateDefClass: constructor, Load_W3D, Read_Header, Read_Info,
//   Read_Subobject, Add_Subobject, Read_Class_Info) and agg_def.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, agg_def by Patrick Smith)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D aggregates (`W3D_CHUNK_AGGREGATE`): `AggregateDefClass::Load_W3D`
//! (called by `AggregateLoaderClass::Load_W3D`), which reads the definition
//! of a model built from a base model with other models attached to its
//! bones. Only the loading; building the model (`Create`, `Attach_Subobjects`)
//! and saving are not translated.
//!
//! # As upstream reads it
//!
//! Inside the aggregate chunk, any chunks in any order:
//! `W3D_CHUNK_AGGREGATE_HEADER` ([`W3dAggregateHeaderStruct`]: version and
//! name), `W3D_CHUNK_AGGREGATE_INFO` ([`W3dAggregateInfoStruct`]: the base
//! model's name and `SubobjectCount`, then that many
//! [`W3dAggregateSubobjectStruct`]s in the same chunk),
//! `W3D_CHUNK_TEXTURE_REPLACER_INFO` (a 4-byte header that only prints a
//! debug message; obsolete) and `W3D_CHUNK_AGGREGATE_CLASS_INFO`
//! ([`W3dAggregateMiscInfo`]). Others are skipped. A short read in a header,
//! info, sub-object or class-info read fails the load
//! ([`Ok(None)`](load_w3d)); the chunk is closed first. No chunk is required:
//! an empty aggregate loads, with no name.
//!
//! # Upstream quirks that are kept
//!
//! - A second header replaces the name and version; a second info chunk
//!   replaces the info and *appends* its sub-objects to the first's; a
//!   second class-info chunk replaces it.
//! - The name is `_strdup` of the header's 16-byte `Name`; the sub-object
//!   and bone names are `lstrcpy`'d into zeroed 32-byte arrays.
//! - The class info starts as all zeros with `OriginalClassID` =
//!   `CLASSID_HLOD` ([`CLASSID_HLOD`]); `m_Version` starts at 0 (the
//!   allocator's zeros) until a header is read.
//! - A short texture-replacer header is not an error.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: a 16-character header `Name` (`_strdup` reads past the
//! struct: [`W3dUndefined::StringOverrun`]); a sub-object's `SubobjectName`
//! without a 0 in its 32 bytes (`lstrcpy` copies more than 32 bytes into
//! it: [`W3dUndefined::StringOverflow`]) or `BoneName` without one (reads
//! past the struct: [`W3dUndefined::StringOverrun`]).
//!
//! # Representation
//!
//! [`AggregateDef`] holds the members `Load_W3D` sets; names are the bytes
//! before their terminator.

use super::w3d_file::*;
use super::{c_string, W3dResult, W3dUndefined};
use crate::ww::chunkio::ChunkLoad;

/// `RenderObjClass::CLASSID_HLOD` (rendobj.h).
pub const CLASSID_HLOD: u32 = 25;

/// One of `m_SubobjectList`: a `W3dAggregateSubobjectStruct` as
/// `Add_Subobject` copies it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateSubobject {
    /// `SubobjectName` (up to its terminator).
    pub subobject_name: Vec<u8>,
    /// `BoneName` (up to its terminator).
    pub bone_name: Vec<u8>,
}

/// `class AggregateDefClass`: what `Load_W3D` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct AggregateDef {
    /// `m_pName` (`None` for a null pointer: no header read).
    pub name: Option<Vec<u8>>,
    /// `m_Version`.
    pub version: u32,
    /// `m_Info`.
    pub info: W3dAggregateInfoStruct,
    /// `m_MiscInfo`.
    pub misc_info: W3dAggregateMiscInfo,
    /// `m_SubobjectList`.
    pub subobjects: Vec<AggregateSubobject>,
}

impl Default for AggregateDef {
    /// `AggregateDefClass()` (on the game's zeroed memory).
    fn default() -> Self {
        let mut misc_info = W3dAggregateMiscInfo::from_bytes(&[0; W3dAggregateMiscInfo::SIZE]);
        misc_info.original_class_id = CLASSID_HLOD;
        AggregateDef {
            name: None,
            version: 0,
            info: W3dAggregateInfoStruct::from_bytes(&[0; W3dAggregateInfoStruct::SIZE]),
            misc_info,
            subobjects: Vec::new(),
        }
    }
}

impl AggregateDef {
    /// `Class_ID()`.
    pub fn class_id(&self) -> u32 {
        self.misc_info.original_class_id
    }
}

/// `AggregateDefClass::Load_W3D(chunk_load)` on a new definition, with
/// `cload` inside the aggregate chunk. `Ok(None)` is upstream's error
/// return (`WW3D_ERROR_LOAD_FAILED`).
pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<AggregateDef>> {
    let mut def = AggregateDef::default();
    while cload.open_chunk()? {
        let ok = match cload.cur_chunk_id()? {
            W3D_CHUNK_AGGREGATE_HEADER => read_header(cload, &mut def)?,
            W3D_CHUNK_AGGREGATE_INFO => read_info(cload, &mut def)?,
            W3D_CHUNK_TEXTURE_REPLACER_INFO => {
                // only a debug message when ReplacedTexturesCount > 0
                let mut b = [0u8; W3dTextureReplacerHeaderStruct::SIZE];
                cload.read_into(&mut b)?;
                true
            }
            W3D_CHUNK_AGGREGATE_CLASS_INFO => read_class_info(cload, &mut def)?,
            _ => true,
        };
        cload.close_chunk()?;
        if !ok {
            return Ok(None);
        }
    }
    Ok(Some(def))
}

/// `Read_Header`.
fn read_header(cload: &mut ChunkLoad<'_>, def: &mut AggregateDef) -> W3dResult<bool> {
    let mut b = [0u8; W3dAggregateHeaderStruct::SIZE];
    if cload.read_into(&mut b)? != W3dAggregateHeaderStruct::SIZE as u32 {
        return Ok(false);
    }
    // m_pName = _strdup(header.Name) (the previous one leaks)
    def.name = Some(c_string(&b, 4)?.to_vec());
    def.version = W3dAggregateHeaderStruct::from_bytes(&b).version;
    Ok(true)
}

/// `Read_Info` and its `Read_Subobject`s.
fn read_info(cload: &mut ChunkLoad<'_>, def: &mut AggregateDef) -> W3dResult<bool> {
    let mut b = [0u8; W3dAggregateInfoStruct::SIZE];
    if cload.read_into(&mut b)? != W3dAggregateInfoStruct::SIZE as u32 {
        // m_Info keeps what was copied; the load fails, so it is not kept
        return Ok(false);
    }
    def.info = W3dAggregateInfoStruct::from_bytes(&b);
    for _ in 0..def.info.subobject_count {
        let mut sb = [0u8; W3dAggregateSubobjectStruct::SIZE];
        if cload.read_into(&mut sb)? != W3dAggregateSubobjectStruct::SIZE as u32 {
            return Ok(false);
        }
        // Add_Subobject: lstrcpy into the 32-byte members of a zeroed struct
        let name = c_string(&sb, 0)?;
        if name.len() >= 2 * W3D_NAME_LEN {
            return Err(W3dUndefined::StringOverflow.into());
        }
        let bone = c_string(&sb, 2 * W3D_NAME_LEN)?;
        def.subobjects.push(AggregateSubobject { subobject_name: name.to_vec(), bone_name: bone.to_vec() });
    }
    Ok(true)
}

/// `Read_Class_Info`.
fn read_class_info(cload: &mut ChunkLoad<'_>, def: &mut AggregateDef) -> W3dResult<bool> {
    let mut b = [0u8; W3dAggregateMiscInfo::SIZE];
    if cload.read_into(&mut b)? != W3dAggregateMiscInfo::SIZE as u32 {
        return Ok(false);
    }
    def.misc_info = W3dAggregateMiscInfo::from_bytes(&b);
    Ok(true)
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

    fn header(version: u32, name: &[u8]) -> Vec<u8> {
        let mut h = W3dAggregateHeaderStruct::from_bytes(&[0; 20]);
        h.version = version;
        h.name[..name.len()].copy_from_slice(name);
        chunk(W3D_CHUNK_AGGREGATE_HEADER, &h.to_bytes())
    }

    fn info(base: &[u8], count: u32, subs: &[(&[u8], &[u8])]) -> Vec<u8> {
        let mut i = W3dAggregateInfoStruct::from_bytes(&[0; 36]);
        i.base_model_name[..base.len()].copy_from_slice(base);
        i.subobject_count = count;
        let mut b = i.to_bytes();
        for (n, bone) in subs {
            let mut s = W3dAggregateSubobjectStruct::from_bytes(&[0; 64]);
            s.subobject_name[..n.len()].copy_from_slice(n);
            s.bone_name[..bone.len()].copy_from_slice(bone);
            b.extend(s.to_bytes());
        }
        chunk(W3D_CHUNK_AGGREGATE_INFO, &b)
    }

    fn class_info(id: u32, flags: u32) -> Vec<u8> {
        let mut m = W3dAggregateMiscInfo::from_bytes(&[0; 20]);
        m.original_class_id = id;
        m.flags = flags;
        chunk(W3D_CHUNK_AGGREGATE_CLASS_INFO, &m.to_bytes())
    }

    fn load(body: &[u8]) -> (W3dResult<Option<AggregateDef>>, i32) {
        let data = chunk(W3D_CHUNK_AGGREGATE, body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let r = load_w3d(&mut c);
        (r, c.cur_chunk_depth())
    }

    /// A full aggregate: header, info with two sub-objects, a texture
    /// replacer header, class info, and an unknown chunk.
    ///
    /// Predicted before the run: name `AGG`, version 0x10003; base model
    /// `BASE`, `SubobjectCount` 2; sub-objects (`GUN`, `BONE01`) and
    /// (`FLAG`, `BONE02`); class id 0 (the class info's, replacing
    /// `CLASSID_HLOD`) and flags 3; depth 1.
    #[test]
    fn loads_header_info_subobjects_and_class_info() {
        let mut body = header(W3D_CURRENT_AGGREGATE_VERSION, b"AGG");
        body.extend(info(b"BASE", 2, &[(b"GUN", b"BONE01"), (b"FLAG", b"BONE02")]));
        body.extend(chunk(W3D_CHUNK_TEXTURE_REPLACER_INFO, &5u32.to_le_bytes()));
        body.extend(class_info(0, 3));
        body.extend(chunk(0x0000_06FF, &[9; 7]));
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(d.name.as_deref(), Some(&b"AGG"[..]));
        assert_eq!(d.version, 0x0001_0003);
        assert_eq!(&d.info.base_model_name[..5], b"BASE\0");
        assert_eq!(d.info.subobject_count, 2);
        assert_eq!(
            d.subobjects,
            vec![
                AggregateSubobject { subobject_name: b"GUN".to_vec(), bone_name: b"BONE01".to_vec() },
                AggregateSubobject { subobject_name: b"FLAG".to_vec(), bone_name: b"BONE02".to_vec() },
            ]
        );
        assert_eq!((d.class_id(), d.misc_info.flags), (0, 3));
    }

    /// Quirks: an empty aggregate; a second info chunk appending; a second
    /// header replacing; a short texture-replacer header.
    ///
    /// Predicted before the run: the empty aggregate loads with no name,
    /// version 0, class id `CLASSID_HLOD` (25); the other has name `TWO`
    /// (version 2), the second info's base model `B2` and count 1, and three
    /// sub-objects (`A`, `B` from the first info, then `C`); a 2-byte texture
    /// replacer chunk is not an error.
    #[test]
    fn empty_aggregate_and_repeated_chunks() {
        let (r, depth) = load(&[]);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!((d.name.as_deref(), d.version, d.class_id()), (None, 0, CLASSID_HLOD));
        assert!(d.subobjects.is_empty());

        let mut body = header(1, b"ONE");
        body.extend(info(b"B1", 2, &[(b"A", b"BA"), (b"B", b"BB")]));
        body.extend(chunk(W3D_CHUNK_TEXTURE_REPLACER_INFO, &[1, 2]));
        body.extend(header(2, b"TWO"));
        body.extend(info(b"B2", 1, &[(b"C", b"BC")]));
        let d = load(&body).0.unwrap().unwrap();
        assert_eq!((d.name.as_deref(), d.version), (Some(&b"TWO"[..]), 2));
        assert_eq!((&d.info.base_model_name[..3], d.info.subobject_count), (&b"B2\0"[..], 1));
        let names: Vec<&[u8]> = d.subobjects.iter().map(|s| &s.subobject_name[..]).collect();
        assert_eq!(names, vec![&b"A"[..], b"B", b"C"]);
        assert_eq!(d.class_id(), CLASSID_HLOD);
    }

    /// Load failures and undefined inputs.
    ///
    /// Predicted before the run: a short header, a short info, an info whose
    /// count promises more sub-objects than the chunk holds, and a short
    /// class info each fail the load with the chunk closed (depth 1); a
    /// 16-character header name is `StringOverrun`; a 32-character
    /// sub-object name `StringOverflow`; a 32-character bone name
    /// `StringOverrun`; a 31-character sub-object name loads.
    #[test]
    fn load_failures_and_undefined() {
        for body in [
            chunk(W3D_CHUNK_AGGREGATE_HEADER, &[0; 19]),
            chunk(W3D_CHUNK_AGGREGATE_INFO, &[0; 35]),
            info(b"B", 2, &[(b"A", b"BA")]),
            chunk(W3D_CHUNK_AGGREGATE_CLASS_INFO, &[0; 19]),
        ] {
            assert_eq!(load(&body), (Ok(None), 1));
        }
        let undefined = |w: W3dUndefined| Err(W3dError::Undefined(w));
        assert_eq!(load(&header(1, b"ABCDEFGHIJKLMNOP")).0, undefined(W3dUndefined::StringOverrun));
        assert_eq!(load(&info(b"B", 1, &[(&[b'N'; 32], b"BONE")])).0, undefined(W3dUndefined::StringOverflow));
        assert_eq!(load(&info(b"B", 1, &[(b"N", &[b'B'; 32])])).0, undefined(W3dUndefined::StringOverrun));
        let d = load(&info(b"B", 1, &[(&[b'N'; 31], b"BONE")])).0.unwrap().unwrap();
        assert_eq!(d.subobjects[0].subobject_name, vec![b'N'; 31]);
    }
}
