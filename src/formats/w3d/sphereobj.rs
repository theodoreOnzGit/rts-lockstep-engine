// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/sphereobj.cpp
//   (SphereLoaderClass::Load_W3D; SpherePrototypeClass: constructor, Load,
//   Get_Name, Get_Class_ID) and sphereobj.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, sphereobj by Jason Andersen)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D spheres (`W3D_CHUNK_SPHERE`): `SphereLoaderClass::Load_W3D`, which
//! reads a `SpherePrototypeClass`: the definition ([`W3dSphereStruct`]) and
//! four keyframe channels ([`super::prim_anim`]). Only the loading; the
//! sphere render object and its mesh are not translated.
//!
//! # As upstream reads it
//!
//! The prototype starts with its definition `memset` to zeros and empty
//! channels. Inside the sphere chunk, any chunks: `CHUNKID_SPHERE_DEF` (1)
//! is read into the definition with `cload.Read(&Definition,
//! sizeof(Definition))`; `CHUNKID_COLOR_CHANNEL` (2, `Vector3`),
//! `CHUNKID_ALPHA_CHANNEL` (3, `float`), `CHUNKID_SCALE_CHANNEL` (4,
//! `Vector3`) and `CHUNKID_VECTOR_CHANNEL` (5, `AlphaVectorStruct`) are
//! loaded into their channels; others are skipped. The load never fails.
//! The class is `CLASSID_SPHERE` ([`CLASSID_SPHERE`]).
//!
//! # Upstream quirks that are kept
//!
//! - The definition read is not checked: a definition chunk shorter than
//!   the struct reads nothing (the definition stays as it was: zeros, or an
//!   earlier chunk's); one cut short by the end of the file overwrites only
//!   the bytes the file had. A later definition chunk overwrites an earlier
//!   one.
//! - A repeated channel chunk replaces the channel ([`super::prim_anim`]).
//! - The definition's `DefaultVector` is zeros, not `AlphaVectorStruct()`'s
//!   identity, until a definition is read (the `memset` comes after the
//!   constructor).
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! Only the channels' ([`super::prim_anim`]).

use super::prim_anim::Channel;
use super::w3d_file::*;
use super::W3dResult;
use crate::ww::chunkio::ChunkLoad;

/// `RenderObjClass::CLASSID_SPHERE` (rendobj.h).
pub const CLASSID_SPHERE: u32 = 29;
/// `CHUNKID_SPHERE_DEF` .. `CHUNKID_VECTOR_CHANNEL` (sphereobj.cpp).
pub const CHUNKID_SPHERE_DEF: u32 = 1;
pub const CHUNKID_COLOR_CHANNEL: u32 = 2;
pub const CHUNKID_ALPHA_CHANNEL: u32 = 3;
pub const CHUNKID_SCALE_CHANNEL: u32 = 4;
pub const CHUNKID_VECTOR_CHANNEL: u32 = 5;

/// `SpherePrototypeClass`: what `Load` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct SpherePrototype {
    /// `Definition`.
    pub definition: W3dSphereStruct,
    /// `ColorChannel` (`LERPAnimationChannelClass<Vector3>`).
    pub color_channel: Channel<[f32; 3]>,
    /// `AlphaChannel` (`LERPAnimationChannelClass<float>`).
    pub alpha_channel: Channel<f32>,
    /// `ScaleChannel` (`LERPAnimationChannelClass<Vector3>`).
    pub scale_channel: Channel<[f32; 3]>,
    /// `VectorChannel` (`AlphaVectorChannel`).
    pub vector_channel: Channel<AlphaVectorStruct>,
}

impl SpherePrototype {
    /// `Get_Class_ID()`.
    pub fn class_id(&self) -> u32 {
        CLASSID_SPHERE
    }
}

/// `SphereLoaderClass::Load_W3D(cload)` (`SpherePrototypeClass()` and
/// `Load`), with `cload` inside the sphere chunk.
pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<SpherePrototype> {
    let mut def = [0u8; W3dSphereStruct::SIZE];
    let mut p = SpherePrototype {
        definition: W3dSphereStruct::from_bytes(&def),
        color_channel: Channel::default(),
        alpha_channel: Channel::default(),
        scale_channel: Channel::default(),
        vector_channel: Channel::default(),
    };
    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            CHUNKID_SPHERE_DEF => {
                // unchecked: the bytes not copied stay as they were
                cload.read_into(&mut def)?;
                p.definition = W3dSphereStruct::from_bytes(&def);
            }
            CHUNKID_COLOR_CHANNEL => p.color_channel = Channel::load(cload)?,
            CHUNKID_ALPHA_CHANNEL => p.alpha_channel = Channel::load(cload)?,
            CHUNKID_SCALE_CHANNEL => p.scale_channel = Channel::load(cload)?,
            CHUNKID_VECTOR_CHANNEL => p.vector_channel = Channel::load(cload)?,
            _ => {}
        }
        cload.close_chunk()?;
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::prim_anim::tests::{channel, chunk, key};
    use crate::formats::w3d::prim_anim::Key;
    use crate::ww::chunkio::GameFile;

    fn sphere_def(name: &[u8]) -> W3dSphereStruct {
        let mut s = W3dSphereStruct::from_bytes(&[0; 164]);
        s.version = 0x0001_0000;
        s.attributes = 3;
        s.name[..name.len()].copy_from_slice(name);
        s.extent = W3dVectorStruct { x: 2.0, y: 2.0, z: 2.0 };
        s.anim_duration = 1.5;
        s.default_alpha = 0.5;
        s.default_vector = AlphaVectorStruct { angle: [0.0, 0.0, 0.0, 1.0], intensity: 2.0 };
        s.texture_name[..7].copy_from_slice(b"FX1.TGA");
        s
    }

    fn load(body: &[u8]) -> (SpherePrototype, i32) {
        let data = chunk(W3D_CHUNK_SPHERE, body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let p = load_w3d(&mut c).unwrap();
        (p, c.cur_chunk_depth())
    }

    /// A sphere with its definition and the four channels, and an unknown
    /// chunk.
    ///
    /// Predicted before the run: the definition as written; color keys
    /// (1,0,0)@0 and (0,1,0)@1; alpha 0.25@0.5; scale (2,2,2)@0; vector
    /// (identity, 3)@0.75; depth 1.
    #[test]
    fn loads_definition_and_channels() {
        let def = sphere_def(b"CONT.SPHERE");
        let mut body = chunk(CHUNKID_SPHERE_DEF, &def.to_bytes());
        body.extend(channel(CHUNKID_COLOR_CHANNEL, &[key(&[1.0, 0.0, 0.0], 0.0), key(&[0.0, 1.0, 0.0], 1.0)]));
        body.extend(channel(CHUNKID_ALPHA_CHANNEL, &[key(&[0.25], 0.5)]));
        body.extend(chunk(99, &[1, 2, 3]));
        body.extend(channel(CHUNKID_SCALE_CHANNEL, &[key(&[2.0, 2.0, 2.0], 0.0)]));
        body.extend(channel(CHUNKID_VECTOR_CHANNEL, &[key(&[0.0, 0.0, 0.0, 1.0, 3.0], 0.75)]));
        let (p, depth) = load(&body);
        assert_eq!(depth, 1);
        assert_eq!(p.definition, def);
        assert_eq!(p.class_id(), CLASSID_SPHERE);
        assert_eq!(p.color_channel.keys, vec![Key { value: [1.0, 0.0, 0.0], time: 0.0 }, Key { value: [0.0, 1.0, 0.0], time: 1.0 }]);
        assert_eq!(p.alpha_channel.keys, vec![Key { value: 0.25, time: 0.5 }]);
        assert_eq!(p.scale_channel.keys, vec![Key { value: [2.0, 2.0, 2.0], time: 0.0 }]);
        assert_eq!(
            p.vector_channel.keys,
            vec![Key { value: AlphaVectorStruct { angle: [0.0, 0.0, 0.0, 1.0], intensity: 3.0 }, time: 0.75 }]
        );
    }

    /// Quirks: no definition chunk; a short definition chunk after a whole
    /// one; a repeated channel.
    ///
    /// Predicted before the run: with no definition the struct is all zeros
    /// (including `DefaultVector`); a 100-byte definition chunk after a
    /// whole one reads nothing, so the whole one stays; the second color
    /// channel replaces the first (one key (5,5,5)@2).
    #[test]
    fn unchecked_definition_reads_and_replaced_channels() {
        let (p, _) = load(&[]);
        assert_eq!(p.definition, W3dSphereStruct::from_bytes(&[0; 164]));
        assert!(p.color_channel.keys.is_empty());

        let def = sphere_def(b"S");
        let mut body = chunk(CHUNKID_SPHERE_DEF, &def.to_bytes());
        body.extend(chunk(CHUNKID_SPHERE_DEF, &[0xEE; 100]));
        body.extend(channel(CHUNKID_COLOR_CHANNEL, &[key(&[1.0, 1.0, 1.0], 0.0)]));
        body.extend(channel(CHUNKID_COLOR_CHANNEL, &[key(&[5.0, 5.0, 5.0], 2.0)]));
        let (p, _) = load(&body);
        assert_eq!(p.definition, def);
        assert_eq!(p.color_channel.keys, vec![Key { value: [5.0, 5.0, 5.0], time: 2.0 }]);
    }

    /// Quirk: a definition chunk cut short by the end of the file.
    ///
    /// Predicted before the run: the 10 bytes the file has overwrite the
    /// start of the zeroed definition (version 0x10000, attributes 3, the
    /// first two name bytes `AB`), the rest stays zero.
    #[test]
    fn definition_cut_by_end_of_file() {
        let def = sphere_def(b"ABCD");
        let mut data = chunk(W3D_CHUNK_SPHERE, &chunk(CHUNKID_SPHERE_DEF, &def.to_bytes()));
        data.truncate(16 + 10);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let p = load_w3d(&mut c).unwrap();
        let mut want = [0u8; 164];
        want[..10].copy_from_slice(&def.to_bytes()[..10]);
        assert_eq!(p.definition, W3dSphereStruct::from_bytes(&want));
        assert_eq!(&p.definition.name[..3], b"AB\0");
    }
}
