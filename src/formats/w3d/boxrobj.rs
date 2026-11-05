// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/boxrobj.cpp
//   (BoxLoaderClass::Load_W3D; BoxPrototypeClass: constructor, Get_Name,
//   Get_Class_ID) and boxrobj.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, boxrobj by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D boxes (`W3D_CHUNK_BOX`): `BoxLoaderClass::Load_W3D`, which reads the
//! definition a `BoxPrototypeClass` keeps. Only the loading; the box render
//! objects (`AABoxRenderObjClass`, `OBBoxRenderObjClass`) are not translated.
//!
//! # As upstream reads it
//!
//! The box chunk's data is one [`W3dBoxStruct`], read with
//! `cload.Read(&box, sizeof(box))`; the prototype keeps a copy. The load
//! never fails. The prototype's class is `CLASSID_OBBOX` if the attributes
//! have `W3D_BOX_ATTRIBUTE_ORIENTED`, `CLASSID_AABOX` otherwise.
//!
//! # Upstream quirks that are kept
//!
//! - Nothing else in the chunk is read (bytes after the struct, sub-chunks).
//! - The name is the struct's 32 bytes as read: `Get_Name()` is not
//!   terminated at 32 characters ([`BoxPrototype::name_str`]).
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined::UninitialisedRead`]: a chunk (or file) too short for the
//! struct. The read is not checked, so the struct on the stack keeps
//! uninitialised bytes, which are copied into the prototype and used.

use super::w3d_file::*;
use super::{W3dResult, W3dUndefined};
use crate::ww::chunkio::ChunkLoad;

/// `RenderObjClass::CLASSID_AABOX` (rendobj.h).
pub const CLASSID_AABOX: u32 = 26;
/// `RenderObjClass::CLASSID_OBBOX` (rendobj.h).
pub const CLASSID_OBBOX: u32 = 27;

/// `BoxPrototypeClass`: its `Definition`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxPrototype {
    pub definition: W3dBoxStruct,
}

impl BoxPrototype {
    /// `Get_Name()` up to its terminator (all 32 bytes if there is none; the
    /// C string would run on into `Color`).
    pub fn name_str(&self) -> &[u8] {
        let n = self.definition.name.iter().position(|&c| c == 0).unwrap_or(self.definition.name.len());
        &self.definition.name[..n]
    }

    /// `Get_Class_ID()`.
    pub fn class_id(&self) -> u32 {
        if self.definition.attributes & W3D_BOX_ATTRIBUTE_ORIENTED != 0 {
            CLASSID_OBBOX
        } else {
            CLASSID_AABOX
        }
    }
}

/// `BoxLoaderClass::Load_W3D(cload)`, with `cload` inside the box chunk.
pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<BoxPrototype> {
    let mut b = [0u8; W3dBoxStruct::SIZE];
    if cload.read_into(&mut b)? != W3dBoxStruct::SIZE as u32 {
        return Err(W3dUndefined::UninitialisedRead.into());
    }
    Ok(BoxPrototype { definition: W3dBoxStruct::from_bytes(&b) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::prim_anim::tests::chunk;
    use crate::formats::w3d::W3dError;
    use crate::ww::chunkio::GameFile;

    fn load(body: &[u8]) -> (W3dResult<BoxPrototype>, i32) {
        let data = chunk(W3D_CHUNK_BOX, body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let r = load_w3d(&mut c);
        (r, c.cur_chunk_depth())
    }

    fn box_struct(attributes: u32, name: &[u8]) -> W3dBoxStruct {
        let mut b = W3dBoxStruct::from_bytes(&[0; 68]);
        b.version = W3D_BOX_CURRENT_VERSION;
        b.attributes = attributes;
        b.name[..name.len()].copy_from_slice(name);
        b.center = W3dVectorStruct { x: 1.0, y: 2.0, z: 3.0 };
        b.extent = W3dVectorStruct { x: 4.0, y: 5.0, z: 6.0 };
        b
    }

    /// An oriented and an aligned box, with trailing bytes in the chunk.
    ///
    /// Predicted before the run: the definitions read back as written, the
    /// oriented one `CLASSID_OBBOX` (27), the other `CLASSID_AABOX` (26),
    /// a 32-character name kept whole; the trailing bytes ignored (the
    /// reader stays in the box chunk: depth 1).
    #[test]
    fn loads_the_struct_and_its_class() {
        let ob = box_struct(W3D_BOX_ATTRIBUTE_ORIENTED, b"CONT.BOX");
        let mut body = ob.to_bytes();
        body.extend([1, 2, 3]);
        let (r, depth) = load(&body);
        let p = r.unwrap();
        assert_eq!(depth, 1);
        assert_eq!(p.definition, ob);
        assert_eq!((p.name_str(), p.class_id()), (&b"CONT.BOX"[..], CLASSID_OBBOX));
        let ab = box_struct(W3D_BOX_ATTRIBUTE_ALIGNED, &[b'N'; 32]);
        let p = load(&ab.to_bytes()).0.unwrap();
        assert_eq!((p.name_str(), p.class_id()), (&[b'N'; 32][..], CLASSID_AABOX));
    }

    /// Undefined: a chunk shorter than the struct.
    ///
    /// Predicted before the run: `UninitialisedRead`.
    #[test]
    fn short_box_is_undefined() {
        assert_eq!(load(&[0; 67]).0, Err(W3dError::Undefined(W3dUndefined::UninitialisedRead)));
    }
}
