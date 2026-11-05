// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/ringobj.cpp
//   (RingLoaderClass::Load_W3D; RingPrototypeClass: constructor, Load,
//   Get_Name, Get_Class_ID) and ringobj.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, ringobj by Jason Andersen)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D rings (`W3D_CHUNK_RING`): `RingLoaderClass::Load_W3D`, which reads a
//! `RingPrototypeClass`: the definition ([`W3dRingStruct`]) and four
//! keyframe channels ([`super::prim_anim`]). Only the loading; the ring
//! render object and its mesh are not translated.
//!
//! # As upstream reads it
//!
//! As [`super::sphereobj`]: a zeroed definition and empty channels, then
//! any chunks: `CHUNKID_RING_DEF` (1) read into the definition,
//! `CHUNKID_COLOR_CHANNEL` (2, `Vector3`), `CHUNKID_ALPHA_CHANNEL` (3,
//! `float`), `CHUNKID_INNER_SCALE_CHANNEL` (4, `Vector2`) and
//! `CHUNKID_OUTER_SCALE_CHANNEL` (5, `Vector2`) loaded into their channels,
//! the others skipped. The load never fails. The class is `CLASSID_RING`
//! ([`CLASSID_RING`]).
//!
//! # Upstream quirks that are kept
//!
//! Those of [`super::sphereobj`]: the definition read is unchecked (a short
//! chunk reads nothing, a file cut short overwrites what it had, a later
//! definition overwrites an earlier one); a repeated channel replaces the
//! channel.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! Only the channels' ([`super::prim_anim`]).

use super::prim_anim::Channel;
use super::w3d_file::*;
use super::W3dResult;
use crate::ww::chunkio::ChunkLoad;

/// `RenderObjClass::CLASSID_RING` (rendobj.h).
pub const CLASSID_RING: u32 = 30;
/// `CHUNKID_RING_DEF` .. `CHUNKID_OUTER_SCALE_CHANNEL` (ringobj.cpp).
pub const CHUNKID_RING_DEF: u32 = 1;
pub const CHUNKID_COLOR_CHANNEL: u32 = 2;
pub const CHUNKID_ALPHA_CHANNEL: u32 = 3;
pub const CHUNKID_INNER_SCALE_CHANNEL: u32 = 4;
pub const CHUNKID_OUTER_SCALE_CHANNEL: u32 = 5;

/// `RingPrototypeClass`: what `Load` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct RingPrototype {
    /// `Definition`.
    pub definition: W3dRingStruct,
    /// `ColorChannel` (`LERPAnimationChannelClass<Vector3>`).
    pub color_channel: Channel<[f32; 3]>,
    /// `AlphaChannel` (`LERPAnimationChannelClass<float>`).
    pub alpha_channel: Channel<f32>,
    /// `InnerScaleChannel` (`LERPAnimationChannelClass<Vector2>`).
    pub inner_scale_channel: Channel<[f32; 2]>,
    /// `OuterScaleChannel` (`LERPAnimationChannelClass<Vector2>`).
    pub outer_scale_channel: Channel<[f32; 2]>,
}

impl RingPrototype {
    /// `Get_Class_ID()`.
    pub fn class_id(&self) -> u32 {
        CLASSID_RING
    }
}

/// `RingLoaderClass::Load_W3D(cload)` (`RingPrototypeClass()` and `Load`),
/// with `cload` inside the ring chunk.
pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<RingPrototype> {
    let mut def = [0u8; W3dRingStruct::SIZE];
    let mut p = RingPrototype {
        definition: W3dRingStruct::from_bytes(&def),
        color_channel: Channel::default(),
        alpha_channel: Channel::default(),
        inner_scale_channel: Channel::default(),
        outer_scale_channel: Channel::default(),
    };
    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            CHUNKID_RING_DEF => {
                // unchecked: the bytes not copied stay as they were
                cload.read_into(&mut def)?;
                p.definition = W3dRingStruct::from_bytes(&def);
            }
            CHUNKID_COLOR_CHANNEL => p.color_channel = Channel::load(cload)?,
            CHUNKID_ALPHA_CHANNEL => p.alpha_channel = Channel::load(cload)?,
            CHUNKID_INNER_SCALE_CHANNEL => p.inner_scale_channel = Channel::load(cload)?,
            CHUNKID_OUTER_SCALE_CHANNEL => p.outer_scale_channel = Channel::load(cload)?,
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
    use crate::formats::w3d::{W3dError, W3dUndefined};
    use crate::ww::chunkio::GameFile;

    fn load(body: &[u8]) -> W3dResult<RingPrototype> {
        let data = chunk(W3D_CHUNK_RING, body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        load_w3d(&mut c)
    }

    /// A ring with its definition and the four channels.
    ///
    /// Predicted before the run: the definition as written (texture tiling
    /// -2 kept as an `int`); inner scale (1,1)@0 and (2,2)@1; outer scale
    /// (3,4)@0.5; color (1,1,1)@0; alpha 1@0; class `CLASSID_RING` (30).
    #[test]
    fn loads_definition_and_channels() {
        let mut def = W3dRingStruct::from_bytes(&[0; 168]);
        def.version = 0x0001_0000;
        def.name[..9].copy_from_slice(b"CONT.RING");
        def.inner_extent = [0.5, 0.5];
        def.outer_extent = [1.5, 1.5];
        def.texture_tile_count = -2;
        let mut body = chunk(CHUNKID_RING_DEF, &def.to_bytes());
        body.extend(channel(CHUNKID_INNER_SCALE_CHANNEL, &[key(&[1.0, 1.0], 0.0), key(&[2.0, 2.0], 1.0)]));
        body.extend(channel(CHUNKID_OUTER_SCALE_CHANNEL, &[key(&[3.0, 4.0], 0.5)]));
        body.extend(channel(CHUNKID_COLOR_CHANNEL, &[key(&[1.0, 1.0, 1.0], 0.0)]));
        body.extend(channel(CHUNKID_ALPHA_CHANNEL, &[key(&[1.0], 0.0)]));
        let p = load(&body).unwrap();
        assert_eq!(p.definition, def);
        assert_eq!(p.definition.texture_tile_count, -2);
        assert_eq!(p.inner_scale_channel.keys, vec![Key { value: [1.0, 1.0], time: 0.0 }, Key { value: [2.0, 2.0], time: 1.0 }]);
        assert_eq!(p.outer_scale_channel.keys, vec![Key { value: [3.0, 4.0], time: 0.5 }]);
        assert_eq!(p.color_channel.keys, vec![Key { value: [1.0, 1.0, 1.0], time: 0.0 }]);
        assert_eq!(p.alpha_channel.keys, vec![Key { value: 1.0, time: 0.0 }]);
        assert_eq!(p.class_id(), CLASSID_RING);
    }

    /// Undefined: a `Vector2` scale key whose micro-chunk is too short.
    ///
    /// Predicted before the run: `UninitialisedRead`.
    #[test]
    fn short_scale_key_is_undefined() {
        let body = channel(CHUNKID_INNER_SCALE_CHANNEL, &[key(&[1.0], 0.0)[..4].to_vec()]);
        assert_eq!(load(&body), Err(W3dError::Undefined(W3dUndefined::UninitialisedRead)));
    }
}
