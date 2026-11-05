// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/prim_anim.h
//   (PrimitiveAnimationChannelClass: KeyClass, Reset, Load, Load_Variables)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, prim_anim by Patrick Smith)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The keyframe channels of the sphere and ring primitives
//! ([`super::sphereobj`], [`super::ringobj`]):
//! `PrimitiveAnimationChannelClass<T>::Load`. Only the loading; evaluating a
//! channel is not translated.
//!
//! # As upstream reads it
//!
//! `Load` empties the channel (`Reset`), then reads the chunks inside the
//! channel's chunk: each `CHUNKID_VARIABLES` (0x03150809) holds micro-chunks,
//! of which each `VARID_KEY` (1) is one `KeyClass` (the value, then the time:
//! [`Key`]) read with `cload.Read(&value, sizeof(value))` and appended. Other
//! chunks and micro-chunks are skipped.
//!
//! # Upstream quirks that are kept
//!
//! - The key read is not checked. The key starts as `KeyClass()`: time 0,
//!   and the value's default constructor; the read overwrites what it copies
//!   (nothing if the key passes the end of the micro-chunk or chunk, the
//!   file's bytes if the file is short), and the key is appended anyway. For
//!   [`AlphaVectorStruct`] the default is the identity quaternion and
//!   intensity 1, so such a key is defined.
//! - Every `CHUNKID_VARIABLES` chunk appends to the same channel; a second
//!   load of the channel (a repeated channel chunk) starts it again.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined::UninitialisedRead`]: a key read that does not fill the
//! value of a `float`, `Vector2` or `Vector3` channel, whose default
//! constructors leave the value uninitialised.

use super::w3d_file::*;
use super::{W3dResult, W3dUndefined};
use crate::ww::chunkio::ChunkLoad;

/// `PrimitiveAnimationChannelClass::CHUNKID_VARIABLES`.
pub const CHUNKID_VARIABLES: u32 = 0x0315_0809;
/// `PrimitiveAnimationChannelClass::VARID_KEY`.
pub const VARID_KEY: u32 = 1;

/// A value type of a channel.
pub trait KeyValue: Copy + std::fmt::Debug + PartialEq {
    /// `sizeof(T)` on Win32.
    const SIZE: usize;
    /// The bytes `T()` leaves, or `None` if it leaves them uninitialised.
    fn default_bytes() -> Option<Vec<u8>>;
    /// The value from its `SIZE` bytes.
    fn from_le(b: &[u8]) -> Self;
}

impl KeyValue for f32 {
    const SIZE: usize = 4;
    fn default_bytes() -> Option<Vec<u8>> {
        None
    }
    fn from_le(b: &[u8]) -> Self {
        <f32 as W3dPod>::read_le(b)
    }
}

/// `Vector2` (`X`, `Y`) and `Vector3` (`X`, `Y`, `Z`).
impl<const N: usize> KeyValue for [f32; N] {
    const SIZE: usize = 4 * N;
    fn default_bytes() -> Option<Vec<u8>> {
        None
    }
    fn from_le(b: &[u8]) -> Self {
        <[f32; N] as W3dPod>::read_le(b)
    }
}

impl KeyValue for AlphaVectorStruct {
    const SIZE: usize = <AlphaVectorStruct as W3dPod>::SIZE;
    /// `AlphaVectorStruct()`: `angle(true)` (the identity quaternion),
    /// `intensity(1.0F)`.
    fn default_bytes() -> Option<Vec<u8>> {
        AlphaVectorStruct { angle: [0.0, 0.0, 0.0, 1.0], intensity: 1.0 }.to_bytes().into()
    }
    fn from_le(b: &[u8]) -> Self {
        <AlphaVectorStruct as W3dPod>::read_le(b)
    }
}

/// `PrimitiveAnimationChannelClass<T>::KeyClass`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key<T> {
    /// `m_Value`.
    pub value: T,
    /// `m_Time`.
    pub time: f32,
}

/// `PrimitiveAnimationChannelClass<T>`: its keys (`m_Data`; `m_LastIndex`
/// is 0 after a load).
#[derive(Clone, Debug, PartialEq)]
pub struct Channel<T> {
    pub keys: Vec<Key<T>>,
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Channel { keys: Vec::new() }
    }
}

impl<T: KeyValue> Channel<T> {
    /// `Load(cload)`, with `cload` inside the channel's chunk.
    pub fn load(cload: &mut ChunkLoad<'_>) -> W3dResult<Self> {
        // Reset ()
        let mut ch = Channel::default();
        while cload.open_chunk()? {
            if cload.cur_chunk_id()? == CHUNKID_VARIABLES {
                ch.load_variables(cload)?;
            }
            cload.close_chunk()?;
        }
        Ok(ch)
    }

    /// `Load_Variables(cload)`.
    fn load_variables(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<()> {
        while cload.open_micro_chunk()? {
            if cload.cur_micro_chunk_id() == VARID_KEY {
                // KeyClass value; cload.Read (&value, sizeof (value));
                let size = T::SIZE + 4;
                let default = T::default_bytes();
                let mut buf = default.clone().unwrap_or_else(|| vec![0; T::SIZE]);
                buf.extend_from_slice(&0f32.to_le_bytes());
                let before = cload.file().position();
                cload.read(&mut buf, size as u32)?;
                let copied = cload.file().position().wrapping_sub(before).max(0) as usize;
                if default.is_none() && copied < T::SIZE {
                    return Err(W3dUndefined::UninitialisedRead.into());
                }
                self.keys.push(Key { value: T::from_le(&buf), time: <f32 as W3dPod>::read_le(&buf[T::SIZE..]) });
            }
            cload.close_micro_chunk()?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::formats::w3d::W3dError;
    use crate::ww::chunkio::GameFile;

    pub(crate) fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    /// A channel chunk `id` with one variables chunk holding the given key
    /// micro-chunks (each the bytes of one key).
    pub(crate) fn channel(id: u32, keys: &[Vec<u8>]) -> Vec<u8> {
        let mut vars = Vec::new();
        for k in keys {
            vars.push(VARID_KEY as u8);
            vars.push(k.len() as u8);
            vars.extend_from_slice(k);
        }
        chunk(id, &chunk(CHUNKID_VARIABLES, &vars))
    }

    pub(crate) fn key(values: &[f32], time: f32) -> Vec<u8> {
        values.iter().chain([&time]).flat_map(|f| f.to_le_bytes()).collect()
    }

    fn load<T: KeyValue>(data: &[u8]) -> W3dResult<Channel<T>> {
        let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
        assert!(c.open_chunk().unwrap());
        Channel::<T>::load(&mut c)
    }

    /// Keys of each value type, an unknown micro-chunk and a second
    /// variables chunk.
    ///
    /// Predicted before the run: a `Vector3` channel with keys (1,2,3)@0.5
    /// and, from the second variables chunk, (4,5,6)@1; the micro-chunk of id
    /// 2 skipped.
    #[test]
    fn loads_keys_from_every_variables_chunk() {
        let mut data = channel(9, &[key(&[1.0, 2.0, 3.0], 0.5)]);
        let mut extra = vec![2u8, 3, 0, 0, 0];
        extra.extend([VARID_KEY as u8, 16]);
        extra.extend(key(&[4.0, 5.0, 6.0], 1.0));
        let more = chunk(CHUNKID_VARIABLES, &extra);
        let len = u32::from_le_bytes(data[4..8].try_into().unwrap()) + more.len() as u32;
        data[4..8].copy_from_slice(&len.to_le_bytes());
        data.extend(more);
        let ch = load::<[f32; 3]>(&data).unwrap();
        assert_eq!(
            ch.keys,
            vec![Key { value: [1.0, 2.0, 3.0], time: 0.5 }, Key { value: [4.0, 5.0, 6.0], time: 1.0 }]
        );
    }

    /// Quirk and undefined: keys in micro-chunks too short for them.
    ///
    /// Predicted before the run: an `AlphaVectorStruct` key in a 4-byte
    /// micro-chunk reads nothing and is the default (identity, intensity 1,
    /// time 0); a `float` key in a 4-byte micro-chunk is undefined
    /// (`UninitialisedRead`), as is a `Vector2` key whose micro-chunk is 4
    /// bytes.
    #[test]
    fn short_key_reads() {
        let mut vars = vec![VARID_KEY as u8, 4];
        vars.extend(9f32.to_le_bytes());
        let data = chunk(7, &chunk(CHUNKID_VARIABLES, &vars));
        let ch = load::<AlphaVectorStruct>(&data).unwrap();
        assert_eq!(
            ch.keys,
            vec![Key { value: AlphaVectorStruct { angle: [0.0, 0.0, 0.0, 1.0], intensity: 1.0 }, time: 0.0 }]
        );
        assert_eq!(load::<f32>(&data), Err(W3dError::Undefined(W3dUndefined::UninitialisedRead)));
        assert_eq!(load::<[f32; 2]>(&data), Err(W3dError::Undefined(W3dUndefined::UninitialisedRead)));
    }
}
