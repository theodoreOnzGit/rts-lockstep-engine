// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/motchan.cpp, motchan.h
//   (the Load_W3D of MotionChannelClass, BitChannelClass,
//   TimeCodedMotionChannelClass, TimeCodedBitChannelClass and
//   AdaptiveDeltaMotionChannelClass, and the members they set)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, motchan by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Animation channels: one per animated pivot and component, as the
//! `Load_W3D` of each channel class reads it from its chunk. Only the
//! loading; the evaluation (`Get_Vector`, the time-code search, the
//! adaptive-delta decompression) is not translated yet.
//!
//! # As upstream reads them (each from the start of its chunk)
//!
//! - [`MotionChannel`] (`W3D_CHUNK_ANIMATION_CHANNEL`): a
//!   [`W3dAnimChannelStruct`], then `(LastFrame - FirstFrame + 1) *
//!   VectorLen - 1` more floats (the first is the struct's `Data[0]`); the
//!   rest of the chunk is skipped with `Seek` ("a bug in the exporter which
//!   saved too much data").
//! - [`BitChannel`] (`W3D_CHUNK_BIT_CHANNEL`): a [`W3dBitChannelStruct`],
//!   then `ceil(numbits / 8) - 1` more bytes.
//! - [`TimeCodedMotionChannel`], [`AdaptiveDeltaMotionChannel`]
//!   (`W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL`): the struct, then the rest of
//!   the chunk as `uint32`s.
//! - [`TimeCodedBitChannel`] (`W3D_CHUNK_COMPRESSED_BIT_CHANNEL`): the
//!   struct, then `NumTimeCodes - 1` more `uint32`s.
//!
//! A failed read fails the load (the channel is freed).
//!
//! # Upstream quirks that are kept
//!
//! - The counts are computed in `unsigned` arithmetic from `int`s and wrap:
//!   `LastFrame < FirstFrame` gives a huge count (`num_floats` or `numbits`)
//!   that the read then fails on, or (times `VectorLen`, modulo 2^32) an
//!   ordinary one; `numbits` near 2^32 wraps `(numbits + 7) / 8` to a small
//!   byte count.
//! - `MotionChannelClass` seeks over `chunk length - 16 - datasize` bytes
//!   whenever that `unsigned` difference is not 0, including when it has
//!   wrapped (the seek then fails, harmlessly).
//! - `Do_Data_Compression` returns at once (its body is dead code), so the
//!   data stays as read.
//! - `AdaptiveDeltaMotionChannelClass`'s constructor fills the shared
//!   filter table from `WWMath::Sin`; that only matters for decompression.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: a count of 0 elements (`Data[0]` written into a
//! zero-length array); an array the 32-bit original cannot allocate (over
//! 2^31 - 1 bytes, where its `new[]` size computation also overflows); a
//! compressed motion channel whose chunk length is not a multiple of 4
//! (the read of the rest of the chunk overruns `Data` by the odd bytes).

use super::w3d_file::*;
use super::{read_array, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::chunkio::ChunkLoad;

fn check_alloc(count: u32, elem: u64) -> W3dResult<()> {
    if count == 0 {
        return Err(W3dUndefined::WriteToEmptyArray.into());
    }
    if u64::from(count) * elem > WIN32_MAX_ALLOCATION {
        return Err(W3dUndefined::Allocation.into());
    }
    Ok(())
}

fn words(b: &[u8]) -> impl Iterator<Item = u32> + '_ {
    b.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
}

/// `class MotionChannelClass` as loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionChannel {
    /// `PivotIdx` (the struct's `Pivot`; +1 for pre-3.0 files, set by the
    /// raw animation loader).
    pub pivot_idx: u32,
    /// `Type` (the struct's `Flags`: `ANIM_CHANNEL_X` ...).
    pub type_: u32,
    /// `VectorLen`.
    pub vector_len: i32,
    /// `FirstFrame`.
    pub first_frame: i32,
    /// `LastFrame`.
    pub last_frame: i32,
    /// `Data`: `num_floats` floats.
    pub data: Vec<f32>,
}

impl MotionChannel {
    /// `MotionChannelClass::Load_W3D(cload)`. `Ok(None)` is a failed load.
    pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<Self>> {
        let size = cload.cur_chunk_length()? as i32;
        let saved_datasize = (size as u32).wrapping_sub(W3dAnimChannelStruct::SIZE as u32);
        let mut cb = [0u8; W3dAnimChannelStruct::SIZE];
        if cload.read_into(&mut cb)? != W3dAnimChannelStruct::SIZE as u32 {
            return Ok(None);
        }
        let chan = W3dAnimChannelStruct::from_bytes(&cb);
        let first_frame = i32::from(chan.first_frame);
        let last_frame = i32::from(chan.last_frame);
        let vector_len = i32::from(chan.vector_len);
        let mut num_floats = (last_frame - first_frame + 1) as u32;
        num_floats = num_floats.wrapping_mul(vector_len as u32);
        let datasize = num_floats.wrapping_sub(1).wrapping_mul(4);
        check_alloc(num_floats, 4)?;
        // &Data[1] holds num_floats - 1 floats: exactly datasize bytes
        let rest = match read_array(cload, datasize, datasize as usize)? {
            Some(r) => r,
            None => return Ok(None),
        };
        let mut data = Vec::with_capacity(rest.len() / 4 + 1);
        data.push(chan.data[0]);
        data.extend(words(&rest).map(f32::from_bits));
        // Skip over the extra data at the end of the chunk
        if saved_datasize.wrapping_sub(datasize) > 0 {
            cload.seek(saved_datasize.wrapping_sub(datasize))?;
        }
        Ok(Some(MotionChannel {
            pivot_idx: u32::from(chan.pivot),
            type_: u32::from(chan.flags),
            vector_len,
            first_frame,
            last_frame,
            data,
        }))
    }

    /// `Get_Pivot()`.
    pub fn get_pivot(&self) -> i32 {
        self.pivot_idx as i32
    }
}

/// `class BitChannelClass` as loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct BitChannel {
    pub pivot_idx: u32,
    pub type_: u32,
    pub default_val: i32,
    pub first_frame: i32,
    pub last_frame: i32,
    /// `Bits`: `numbytes` bytes.
    pub bits: Vec<u8>,
}

impl BitChannel {
    /// `BitChannelClass::Load_W3D(cload)`. `Ok(None)` is a failed load.
    pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<Self>> {
        let mut cb = [0u8; W3dBitChannelStruct::SIZE];
        if cload.read_into(&mut cb)? != W3dBitChannelStruct::SIZE as u32 {
            return Ok(None);
        }
        let chan = W3dBitChannelStruct::from_bytes(&cb);
        let first_frame = i32::from(chan.first_frame);
        let last_frame = i32::from(chan.last_frame);
        let numbits = (last_frame - first_frame + 1) as u32;
        let numbytes = numbits.wrapping_add(7) / 8;
        let bytesleft = numbytes.wrapping_sub(1);
        check_alloc(numbytes, 1)?;
        let mut bits = vec![chan.data[0]];
        if bytesleft > 0 {
            match read_array(cload, bytesleft, bytesleft as usize)? {
                Some(r) => bits.extend_from_slice(&r),
                None => return Ok(None),
            }
        }
        Ok(Some(BitChannel {
            pivot_idx: u32::from(chan.pivot),
            type_: u32::from(chan.flags),
            default_val: i32::from(chan.default_val),
            first_frame,
            last_frame,
            bits,
        }))
    }

    /// `Get_Pivot()`.
    pub fn get_pivot(&self) -> i32 {
        self.pivot_idx as i32
    }
}

/// `class TimeCodedMotionChannelClass` as loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeCodedMotionChannel {
    pub pivot_idx: u32,
    pub type_: u32,
    pub vector_len: i32,
    /// `PacketSize` (`VectorLen + 1`).
    pub packet_size: u32,
    pub num_time_codes: u32,
    /// `LastTimeCodeIdx` (`(NumTimeCodes - 1) * PacketSize`, wrapping).
    pub last_time_code_idx: u32,
    /// `Data`: the rest of the chunk as `uint32`s, plus the struct's `Data[0]`
    /// first.
    pub data: Vec<u32>,
}

impl TimeCodedMotionChannel {
    /// `TimeCodedMotionChannelClass::Load_W3D(cload)`.
    pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<Self>> {
        let size = cload.cur_chunk_length()? as i32;
        let datasize = (size as u32).wrapping_sub(W3dTimeCodedAnimChannelStruct::SIZE as u32);
        let num_ints = datasize / 4 + 1;
        let mut cb = [0u8; W3dTimeCodedAnimChannelStruct::SIZE];
        if cload.read_into(&mut cb)? != W3dTimeCodedAnimChannelStruct::SIZE as u32 {
            return Ok(None);
        }
        let chan = W3dTimeCodedAnimChannelStruct::from_bytes(&cb);
        let vector_len = i32::from(chan.vector_len);
        let packet_size = (vector_len + 1) as u32;
        check_alloc(num_ints, 4)?;
        // &Data[1] holds num_ints - 1 words, datasize rounded down: a chunk
        // length that is not a multiple of 4 overruns it (undefined)
        let rest = match read_array(cload, datasize, (num_ints as usize - 1) * 4)? {
            Some(r) => r,
            None => return Ok(None),
        };
        let mut data = vec![chan.data[0]];
        data.extend(words(&rest));
        Ok(Some(TimeCodedMotionChannel {
            pivot_idx: u32::from(chan.pivot),
            type_: u32::from(chan.flags),
            vector_len,
            packet_size,
            num_time_codes: chan.num_time_codes,
            last_time_code_idx: chan.num_time_codes.wrapping_sub(1).wrapping_mul(packet_size),
            data,
        }))
    }

    /// `Get_Pivot()`.
    pub fn get_pivot(&self) -> i32 {
        self.pivot_idx as i32
    }
}

/// `class AdaptiveDeltaMotionChannelClass` as loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveDeltaMotionChannel {
    pub pivot_idx: u32,
    pub type_: u32,
    pub vector_len: i32,
    pub num_frames: u32,
    /// `Scale`.
    pub scale: f32,
    /// `Data`: the struct's `Data[0]`, then the rest of the chunk as `uint32`s.
    pub data: Vec<u32>,
}

impl AdaptiveDeltaMotionChannel {
    /// `AdaptiveDeltaMotionChannelClass::Load_W3D(cload)`.
    pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<Self>> {
        let size = cload.cur_chunk_length()? as i32;
        let datasize = (size as u32).wrapping_sub(W3dAdaptiveDeltaAnimChannelStruct::SIZE as u32);
        let num_ints = datasize / 4 + 1;
        let mut cb = [0u8; W3dAdaptiveDeltaAnimChannelStruct::SIZE];
        if cload.read_into(&mut cb)? != W3dAdaptiveDeltaAnimChannelStruct::SIZE as u32 {
            return Ok(None);
        }
        let chan = W3dAdaptiveDeltaAnimChannelStruct::from_bytes(&cb);
        let vector_len = i32::from(chan.vector_len);
        // CacheData = new float[VectorLen * 2]: at most 510 floats.
        check_alloc(num_ints, 4)?;
        let rest = match read_array(cload, datasize, (num_ints as usize - 1) * 4)? {
            Some(r) => r,
            None => return Ok(None),
        };
        let mut data = vec![chan.data[0]];
        data.extend(words(&rest));
        Ok(Some(AdaptiveDeltaMotionChannel {
            pivot_idx: u32::from(chan.pivot),
            type_: u32::from(chan.flags),
            vector_len,
            num_frames: chan.num_frames,
            scale: chan.scale,
            data,
        }))
    }

    /// `Get_Pivot()`.
    pub fn get_pivot(&self) -> i32 {
        self.pivot_idx as i32
    }
}

/// `class TimeCodedBitChannelClass` as loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeCodedBitChannel {
    pub pivot_idx: u32,
    pub type_: u32,
    pub default_val: i32,
    pub num_time_codes: u32,
    /// `Bits`: `NumTimeCodes` words.
    pub bits: Vec<u32>,
}

impl TimeCodedBitChannel {
    /// `TimeCodedBitChannelClass::Load_W3D(cload)`.
    pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<Self>> {
        let mut cb = [0u8; W3dTimeCodedBitChannelStruct::SIZE];
        if cload.read_into(&mut cb)? != W3dTimeCodedBitChannelStruct::SIZE as u32 {
            return Ok(None);
        }
        let chan = W3dTimeCodedBitChannelStruct::from_bytes(&cb);
        let bytesleft = chan.num_time_codes.wrapping_sub(1).wrapping_mul(4);
        check_alloc(chan.num_time_codes, 4)?;
        let mut bits = vec![chan.data[0]];
        if bytesleft > 0 {
            match read_array(cload, bytesleft, bytesleft as usize)? {
                Some(r) => bits.extend(words(&r)),
                None => return Ok(None),
            }
        }
        Ok(Some(TimeCodedBitChannel {
            pivot_idx: u32::from(chan.pivot),
            type_: u32::from(chan.flags),
            default_val: i32::from(chan.default_val),
            num_time_codes: chan.num_time_codes,
            bits,
        }))
    }

    /// `Get_Pivot()`.
    pub fn get_pivot(&self) -> i32 {
        self.pivot_idx as i32
    }
}
