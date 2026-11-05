// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/motchan.cpp, motchan.h
//   (the Load_W3D of MotionChannelClass, BitChannelClass,
//   TimeCodedMotionChannelClass, TimeCodedBitChannelClass and
//   AdaptiveDeltaMotionChannelClass, and the members they set; and their
//   evaluation: MotionChannelClass::Get_Vector, Get_Vector_As_Quat,
//   set_identity; BitChannelClass::Get_Bit; TimeCodedMotionChannelClass::
//   Get_Vector, Get_QuatVector, get_index, binary_search_index,
//   set_identity; TimeCodedBitChannelClass::Get_Bit;
//   AdaptiveDeltaMotionChannelClass's filter table (constructor), both
//   decompress, getframe, Get_Vector, Get_QuatVector)
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
//! `Load_W3D` of each channel class reads it from its chunk, ~~Only the
//! loading; the evaluation (`Get_Vector`, the time-code search, the
//! adaptive-delta decompression) is not translated yet.~~ **Corrected
//! 2026-11-05 (#10, step 3):** and their evaluation: `Get_Vector` (and the
//! quaternion forms), the time-code search with its cache, the
//! adaptive-delta decompression with its two-frame cache, and `Get_Bit`.
//!
//! # Evaluation (each channel as upstream evaluates it)
//!
//! - [`MotionChannel::get_vector`] (integer frame): the frame's vector, or
//!   the "identity" outside `FirstFrame..=LastFrame` (`0`, or `(0,0,0,1)`
//!   for a quaternion channel). [`BitChannel::get_bit`]: the frame's bit, or
//!   `DefaultVal` outside.
//! - [`TimeCodedMotionChannel`]: packets of a time code and a vector; the
//!   packet at or before the frame is found ([`get_index`]: the cached
//!   packet, the one after it, else a binary search), then linearly
//!   interpolated (slerped for a quaternion) towards the next packet, unless
//!   that packet has `W3D_TIMECODED_BINARY_MOVEMENT_FLAG` (a step) or there
//!   is none (the last value holds).
//! - [`AdaptiveDeltaMotionChannel`]: the first frame's vector, then per
//!   group of 16 frames and per component a 9-byte packet: a filter-table
//!   index and 16 signed 4-bit deltas, each `nybble * filtertable[index] *
//!   Scale`. A frame is decoded by summing deltas; the last two decoded
//!   frames are cached. Frames past `NumFrames - 1` clamp to it.
//! - [`TimeCodedBitChannel::get_bit`]: the bit of the last time code at or
//!   before the frame, searched forward from the cached one.
//!
//! The arithmetic is `f32` in upstream's order (x86-64 SSE, as the C++
//! compiles in the differential tests). The caches are kept (in
//! [`EvalCache`]s), because results can depend on them (below).
//!
//! [`get_index`]: TimeCodedMotionChannel::get_index
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
//!   `DEG_TO_RAD(90.0f * ratio)` expands to `(double)90.0f * ratio * PI /
//!   180.0`: the product is a `double` (the cast binds to `90.0f` only).
//! - **Adaptive delta, upstream bug (found by the differential test,
//!   2026-11-05):** both `decompress` functions declare `bool done` once,
//!   outside the loop over the vector's components, and never reset it.
//!   Component 0 sets it when it reaches the frame; every later component
//!   then leaves its frame loop after its *first* packet (the `if (done)
//!   break;` after the 16 nybbles), so components 1 to 3 of a quaternion (or
//!   of any vector) only get the deltas of the first packet decoded in that
//!   call: frames past the first 16 (from the start, or past the next
//!   16-frame boundary when continuing) come out wrong. Kept.
//! - **Adaptive delta, upstream bug:** `getframe`'s last case (a frame more
//!   than two past the cached one) continues decoding from the *second*
//!   cached frame (`CacheFrame + 1`) as if it were `CacheFrame`, so that
//!   frame's delta is added twice: the result depends on which frames were
//!   asked for before. Kept (the cache is part of the channel).
//! - Adaptive delta `Get_Vector` writes one value, whatever `VectorLen`;
//!   `Get_QuatVector` reads components 0 to 3 of the cache whatever
//!   `VectorLen` (inside the `2 * VectorLen` cache, a smaller `VectorLen`
//!   reads the other cached frame's components).
//! - Time-coded `Get_QuatVector` reads four floats after the time code
//!   whatever `VectorLen`; `MotionChannelClass::Get_Vector_As_Quat` reads
//!   four floats at `frame * VectorLen` likewise.
//! - `Get_Vector` writes `VectorLen` floats from the pointer it is given
//!   (`&trans[0]`, `&mtx[0][3]`, ...): a longer vector runs on into the
//!   following members of the same object (see [`super::hanim`]).
//! - The time-coded search compares time codes without the flag bit,
//!   unsigned; non-increasing time codes are searched as they are.
//! - `TimeCodedBitChannelClass::Get_Bit` starts its forward scan after the
//!   cached index when the frame is at or past the cached time code, else
//!   from 0.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! Evaluation ([`W3dUndefined`]): a read outside a channel's data or cache
//! ([`W3dUndefined::IndexOutOfRange`], e.g. a time-coded channel with fewer
//! words than its time codes need, or an adaptive-delta frame past its
//! packets), a write past the object `Get_Vector`'s pointer is in (the
//! caller's room), a `float` frame converted to `uint32` that cannot hold it
//! ([`W3dUndefined::FloatToInt`]: negative frames of compressed channels,
//! NaN), adaptive-delta `getframe`'s `float temp[4]` given more than four
//! components; and [`W3dError::NonTerminating`] for a time-code binary
//! search that loops forever (it does on time codes that decrease, or when
//! the frame is before the first time code with fewer than three).
//!
//! Loading ([`W3dUndefined`]): a count of 0 elements (`Data[0]` written into a
//! zero-length array); an array the 32-bit original cannot allocate (over
//! 2^31 - 1 bytes, where its `new[]` size computation also overflows); a
//! compressed motion channel whose chunk length is not a multiple of 4
//! (the read of the rest of the chunk overruns `Data` by the odd bytes).

use super::w3d_file::*;
use super::{f32_to_u32, read_array, EvalCache, W3dError, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::chunkio::ChunkLoad;
use crate::ww::quat::{fast_slerp, Quaternion};
use crate::ww::wwmath::{self, WWMATH_PI};
use std::collections::HashSet;
use std::sync::OnceLock;

fn oob() -> W3dError {
    W3dUndefined::IndexOutOfRange.into()
}

/// `setvec[i] = v` into the room the caller's pointer has.
fn put(out: &mut [f32], i: usize, v: f32) -> W3dResult<()> {
    *out.get_mut(i).ok_or_else(oob)? = v;
    Ok(())
}


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

    /// `set_identity(setvec)`: `(0,0,0,1)` for `ANIM_CHANNEL_Q`, else `0`.
    /// Returns the number of floats written.
    fn set_identity(&self, out: &mut [f32]) -> W3dResult<usize> {
        if self.type_ == ANIM_CHANNEL_Q {
            for (i, v) in [0.0, 0.0, 0.0, 1.0].into_iter().enumerate() {
                put(out, i, v)?;
            }
            Ok(4)
        } else {
            put(out, 0, 0.0)?;
            Ok(1)
        }
    }

    /// `Get_Vector(frame, setvec)`: writes the frame's `VectorLen` floats
    /// (or the identity outside the channel's frames) into `out`, the room
    /// from the caller's pointer to the end of its object. Returns the
    /// number of floats written.
    pub fn get_vector(&self, frame: i32, out: &mut [f32]) -> W3dResult<usize> {
        if frame < self.first_frame || frame > self.last_frame {
            return self.set_identity(out);
        }
        let vframe = (frame - self.first_frame) as usize;
        let n = self.vector_len as usize;
        for i in 0..n {
            let v = *self.data.get(vframe * n + i).ok_or_else(oob)?;
            put(out, i, v)?;
        }
        Ok(n)
    }

    /// `Get_Vector_As_Quat(frame, quat)` (motchan.h,
    /// `SPECIAL_GETVEC_AS_QUAT`): four floats at the frame's vector, or the
    /// identity outside the channel's frames.
    pub fn get_vector_as_quat(&self, frame: i32) -> W3dResult<Quaternion> {
        if frame < self.first_frame || frame > self.last_frame {
            return Ok(Quaternion::IDENTITY);
        }
        let at = (frame - self.first_frame) as usize * self.vector_len as usize;
        let d = self.data.get(at..at + 4).ok_or_else(oob)?;
        Ok(Quaternion::new(d[0], d[1], d[2], d[3]))
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

    /// `Get_Bit(frame)`: `DefaultVal` outside the channel's frames, else the
    /// frame's bit (0 or 1).
    pub fn get_bit(&self, frame: i32) -> W3dResult<i32> {
        if frame < self.first_frame || frame > self.last_frame {
            return Ok(self.default_val);
        }
        let bit = (frame - self.first_frame) as usize;
        let mask = 1u8 << (bit % 8);
        let b = *self.bits.get(bit / 8).ok_or_else(oob)?;
        Ok(i32::from(b & mask != 0))
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
    /// `CachedIdx` (0 after loading): the packet index `get_index` found
    /// last.
    pub cached_idx: EvalCache<u32>,
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
            cached_idx: EvalCache::new(0),
        }))
    }

    /// `Get_Pivot()`.
    pub fn get_pivot(&self) -> i32 {
        self.pivot_idx as i32
    }

    /// `Data[i]` (an unsigned index: upstream's `uint32` arithmetic wraps).
    fn word(&self, i: u32) -> W3dResult<u32> {
        self.data.get(i as usize).copied().ok_or_else(oob)
    }

    /// `Data[i]` with an `int` index (negative is before the array).
    fn word_i(&self, i: i32) -> W3dResult<u32> {
        if i < 0 {
            return Err(oob());
        }
        self.word(i as u32)
    }

    /// `binary_search_index(timecode)`: the packet index (in words) of the
    /// last time code at or before `timecode`. Upstream's loop, step for
    /// step, with its `int` indices; a state it has been in before means it
    /// loops forever ([`W3dError::NonTerminating`]).
    pub fn binary_search_index(&self, timecode: u32) -> W3dResult<u32> {
        let mut left_idx: i32 = 0;
        let mut right_idx: i32 = self.num_time_codes.wrapping_sub(2) as i32;
        let mut idx: i32 = self.last_time_code_idx as i32;
        // special case last packet
        let mut time = self.word_i(idx)? & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
        if timecode >= time {
            return Ok(idx as u32);
        }
        let mut seen = HashSet::new();
        loop {
            if !seen.insert((left_idx, right_idx)) {
                return Err(W3dError::NonTerminating);
            }
            let mut dx = right_idx.checked_sub(left_idx).ok_or(W3dUndefined::IndexOutOfRange)?;
            dx >>= 1; // divide by 2
            dx = dx.checked_add(left_idx).ok_or(W3dUndefined::IndexOutOfRange)?;
            idx = (dx as u32).wrapping_mul(self.packet_size) as i32;
            time = self.word_i(idx)? & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
            if timecode < time {
                right_idx = dx;
                continue;
            }
            time = self.word((idx as u32).wrapping_add(self.packet_size))? & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
            if timecode < time {
                return Ok(idx as u32);
            }
            if left_idx ^ dx != 0 {
                left_idx = dx;
                continue;
            }
            // if leftIdx == dx prior to assignment, then leftIdx is stuck.
            left_idx = left_idx.checked_add(1).ok_or(W3dUndefined::IndexOutOfRange)?;
        }
    }

    /// `get_index(timecode)`: the cached packet if `timecode` is in it, else
    /// the next one if it is in that ("one time look-ahead", which moves the
    /// cache even when it then searches), else a binary search. Updates the
    /// cache.
    pub fn get_index(&self, timecode: u32) -> W3dResult<u32> {
        let mut cached = self.cached_idx.lock();
        let mut time = self.word(*cached)? & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
        if timecode >= time {
            // possibly in the current packet; special case for end packets
            if *cached == self.last_time_code_idx {
                return Ok(*cached);
            }
            time = self.word(cached.wrapping_add(self.packet_size))? & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
            if timecode < time {
                return Ok(*cached);
            }
            // Do one time look-ahead before reverting to a search
            *cached = cached.wrapping_add(self.packet_size);
            if *cached == self.last_time_code_idx {
                return Ok(*cached);
            }
            time = self.word(cached.wrapping_add(self.packet_size))? & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG;
            if timecode < time {
                return Ok(*cached);
            }
        }
        *cached = self.binary_search_index(timecode)?;
        Ok(*cached)
    }

    /// The packet at or before `frame` and the one after it: `None` for the
    /// second when the first is the last packet or the next is a binary
    /// movement (both mean "hold the first"); else the ratio between them.
    fn packets(&self, frame: f32) -> W3dResult<(u32, Option<(u32, f32)>)> {
        let tc0 = f32_to_u32(frame)?;
        let pidx = self.get_index(tc0)?;
        if pidx == self.num_time_codes.wrapping_sub(1).wrapping_mul(self.packet_size) {
            return Ok((pidx, None));
        }
        let p2idx = pidx.wrapping_add(self.packet_size);
        let time = self.word(p2idx)?;
        if time & W3D_TIMECODED_BINARY_MOVEMENT_FLAG != 0 {
            return Ok((pidx, None));
        }
        let time1 = (self.word(pidx)? & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG) as f32;
        let time2 = (time & !W3D_TIMECODED_BINARY_MOVEMENT_FLAG) as f32;
        let ratio = (frame - time1) / (time2 - time1);
        Ok((pidx, Some((p2idx, ratio))))
    }

    /// The float `Data[i]`.
    fn float(&self, i: u32) -> W3dResult<f32> {
        Ok(f32::from_bits(self.word(i)?))
    }

    /// `Get_Vector(frame, setvec)`: writes `VectorLen` floats into `out`
    /// (the room from the caller's pointer).
    pub fn get_vector(&self, frame: f32, out: &mut [f32]) -> W3dResult<()> {
        let (pidx, next) = self.packets(frame)?;
        let n = self.vector_len as u32;
        match next {
            None => {
                for i in 0..n {
                    put(out, i as usize, self.float(pidx.wrapping_add(1).wrapping_add(i))?)?;
                }
            }
            Some((p2idx, ratio)) => {
                for i in 0..n {
                    let a = self.float(pidx.wrapping_add(1).wrapping_add(i))?;
                    let b = self.float(p2idx.wrapping_add(1).wrapping_add(i))?;
                    put(out, i as usize, wwmath::lerp(a, b, ratio))?;
                }
            }
        }
        Ok(())
    }

    /// `Get_QuatVector(frame)`: as [`Self::get_vector`] for four floats,
    /// slerped (`Fast_Slerp`) instead of interpolated linearly.
    pub fn get_quat_vector(&self, frame: f32) -> W3dResult<Quaternion> {
        let (pidx, next) = self.packets(frame)?;
        let quat = |at: u32| -> W3dResult<Quaternion> {
            let v = |k: u32| self.float(at.wrapping_add(1).wrapping_add(k));
            Ok(Quaternion::new(v(0)?, v(1)?, v(2)?, v(3)?))
        };
        match next {
            None => quat(pidx),
            Some((p2idx, ratio)) => {
                let q1 = quat(pidx)?;
                let q2 = quat(p2idx)?;
                Ok(fast_slerp(&q1, &q2, ratio))
            }
        }
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
    /// `CacheFrame` (`0x7FFFFFFF` after loading) and `CacheData`
    /// (`2 * VectorLen` floats, zeroed by the game's allocator).
    pub cache: EvalCache<AdaptiveDeltaCache>,
}

/// The decoding cache of an [`AdaptiveDeltaMotionChannel`].
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveDeltaCache {
    /// `CacheFrame`: the frame in the first half of `data`.
    pub frame: u32,
    /// `CacheData`: frame `CacheFrame`'s vector, then frame
    /// `CacheFrame + 1`'s.
    pub data: Vec<f32>,
}

/// `FILTER_TABLE_SIZE`.
pub const FILTER_TABLE_SIZE: usize = 256;
/// `FILTER_TABLE_GEN_START`.
pub const FILTER_TABLE_GEN_START: usize = 16;
/// `PACKET_SIZE`: a filter index and 16 nybbles.
pub const ADAPTIVE_DELTA_PACKET_SIZE: usize = 9;

/// The adaptive-delta `filtertable`: sixteen powers of ten, then what
/// `AdaptiveDeltaMotionChannelClass`'s constructor fills in once,
/// `1 - WWMath::Sin(DEG_TO_RAD(90.0f * ratio))` for `ratio = i / 240`.
pub fn adaptive_delta_filter_table() -> &'static [f32; FILTER_TABLE_SIZE] {
    static TABLE: OnceLock<[f32; FILTER_TABLE_SIZE]> = OnceLock::new();
    TABLE.get_or_init(|| {
        #[allow(clippy::excessive_precision)]
        const POWERS: [f32; FILTER_TABLE_GEN_START] = [
            0.00000001, 0.0000001, 0.000001, 0.00001, 0.0001, 0.001, 0.01, 0.1, 1.0, 10.0, 100.0, 1000.0, 10000.0,
            100000.0, 1000000.0, 10000000.0,
        ];
        let mut t = [0.0f32; FILTER_TABLE_SIZE];
        t[..FILTER_TABLE_GEN_START].copy_from_slice(&POWERS);
        let gen_size = FILTER_TABLE_SIZE - FILTER_TABLE_GEN_START;
        for i in 0..gen_size {
            let mut ratio = i as f32;
            ratio /= gen_size as f32;
            // DEG_TO_RAD(x) is (((double)x)*WWMATH_PI/180.0) with x = 90.0f * ratio
            let rad = (f64::from(90.0f32) * f64::from(ratio)) * f64::from(WWMATH_PI) / 180.0;
            t[i + FILTER_TABLE_GEN_START] = 1.0f32 - wwmath::sin(rad as f32);
        }
        t
    })
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
            cache: EvalCache::new(AdaptiveDeltaCache { frame: 0x7FFF_FFFF, data: vec![0.0; vector_len as usize * 2] }),
        }))
    }

    /// `Get_Pivot()`.
    pub fn get_pivot(&self) -> i32 {
        self.pivot_idx as i32
    }

    /// Byte `off` of `Data`.
    fn byte(&self, off: u64) -> W3dResult<u8> {
        let w = *self.data.get((off / 4) as usize).ok_or_else(oob)?;
        Ok(w.to_le_bytes()[(off % 4) as usize])
    }

    /// The frame loop of both `decompress`es for component `vi`: from
    /// `last_value` (frame `frame - 1`'s), the packet at byte `packet`, the
    /// nybble `fi`, up to `frame_idx`. `done` is upstream's flag, which is
    /// declared once per `decompress`, outside the loop over the components,
    /// and never reset: once component 0 has reached `frame_idx`, every later
    /// component stops after its first packet (see the module docs).
    fn decode(&self, mut packet: u64, mut fi: usize, mut frame: u32, frame_idx: u32, mut last_value: f32, done: &mut bool) -> W3dResult<f32> {
        let vl = self.vector_len as u64;
        let table = adaptive_delta_filter_table();
        while frame <= frame_idx {
            let filter_index = self.byte(packet)?; // packet header
            packet += 1; // skip to nybble compressed data
            let filter = table[filter_index as usize] * self.scale; // decompression filter
            // data is grouped in sets of 16 nybbles
            while fi < 16 {
                let pi = (fi >> 1) as u64;
                let mut factor = i32::from(self.byte(packet + pi)?);
                if fi & 1 != 0 {
                    factor >>= 4;
                }
                // Sign Extend
                factor &= 0xF;
                if factor & 0x8 != 0 {
                    factor |= 0xFFFF_FFF0u32 as i32;
                }
                let ffactor = factor as f32;
                let delta = ffactor * filter;
                last_value += delta;
                if frame == frame_idx {
                    *done = true;
                    break;
                }
                frame += 1;
                fi += 1;
            }
            fi = 0;
            if *done {
                break; // we're at the desired frame (or an earlier component was)
            }
            packet += ADAPTIVE_DELTA_PACKET_SIZE as u64 * vl - 1; // skip to next packet
        }
        Ok(last_value)
    }

    /// `decompress(frame_idx, outdata)`: from the beginning.
    fn decompress(&self, frame_idx: u32, out: &mut [f32]) -> W3dResult<()> {
        let vl = self.vector_len as usize;
        let mut done = false;
        for (vi, o) in out.iter_mut().enumerate().take(vl) {
            let base = f32::from_bits(*self.data.get(vi).ok_or_else(oob)?);
            let packet = (4 * vl + ADAPTIVE_DELTA_PACKET_SIZE * vi) as u64;
            *o = self.decode(packet, 0, 1, frame_idx, base, &mut done)?;
        }
        Ok(())
    }

    /// `decompress(src_idx, srcdata, frame_idx, outdata)`: continuing from
    /// `srcdata`, taken to be frame `src_idx`'s vector.
    fn decompress_from(&self, src_idx: u32, src: &[f32], frame_idx: u32, out: &mut [f32]) -> W3dResult<()> {
        let src_idx = src_idx.wrapping_add(1);
        let vl = self.vector_len as usize;
        let mut done = false;
        for vi in 0..vl {
            let packet = (4 * vl + ADAPTIVE_DELTA_PACKET_SIZE * vi) as u64
                + (ADAPTIVE_DELTA_PACKET_SIZE * vl) as u64 * u64::from(src_idx.wrapping_sub(1) >> 4);
            let fi = (src_idx.wrapping_sub(1) & 0xF) as usize;
            out[vi] = self.decode(packet, fi, src_idx, frame_idx, src[vi], &mut done)?;
        }
        Ok(())
    }

    /// `getframe(frame_idx, vector_idx)`: component `vector_idx` of the
    /// frame's vector, through the two-frame cache (see the module docs for
    /// the upstream bug in its last case).
    pub fn getframe(&self, mut frame_idx: u32, vector_idx: u32) -> W3dResult<f32> {
        let vl = self.vector_len as usize;
        let vi = vector_idx as usize;
        if frame_idx >= self.num_frames {
            frame_idx = self.num_frames.wrapping_sub(1);
        }
        let mut c = self.cache.lock();
        let c = &mut *c;
        let get = |d: &[f32], i: usize| d.get(i).copied().ok_or_else(oob);
        if c.frame == frame_idx {
            return get(&c.data, vi);
        }
        if c.frame.wrapping_add(1) == frame_idx {
            return get(&c.data, vi + vl);
        }
        let last = self.num_frames.wrapping_sub(1);
        if frame_idx < c.frame {
            let (lo, hi) = c.data.split_at_mut(vl);
            self.decompress(frame_idx, lo)?;
            if frame_idx != last {
                self.decompress_from(frame_idx, lo, frame_idx.wrapping_add(1), hi)?;
            }
            c.frame = frame_idx;
            return get(&c.data, vi);
        }
        if frame_idx == c.frame.wrapping_add(2) {
            // Sliding window
            c.data.copy_within(vl.., 0);
            c.frame = c.frame.wrapping_add(1);
            let (lo, hi) = c.data.split_at_mut(vl);
            self.decompress_from(c.frame, lo, frame_idx, hi)?;
            return get(&c.data, vl + vi);
        }
        // Else just use last known frame to decompress forwards
        if vl > 4 {
            return Err(oob()); // float temp[4]
        }
        let temp = c.data[vl..].to_vec();
        self.decompress_from(c.frame, &temp, frame_idx, &mut c.data[..vl])?;
        c.frame = frame_idx;
        if frame_idx != last {
            let (lo, hi) = c.data.split_at_mut(vl);
            self.decompress_from(frame_idx, lo, frame_idx.wrapping_add(1), hi)?;
        }
        get(&c.data, vi)
    }

    /// `Get_Vector(frame, setvec)`: one value (component 0), interpolated
    /// linearly between the frame's integer part and the next frame.
    pub fn get_vector(&self, frame: f32, out: &mut [f32]) -> W3dResult<()> {
        let frame1 = f32_to_u32(frame)?;
        let ratio = frame - frame1 as f32;
        let value1 = self.getframe(frame1, 0)?;
        let value2 = self.getframe(frame1.wrapping_add(1), 0)?;
        put(out, 0, wwmath::lerp(value1, value2, ratio))
    }

    /// `Get_QuatVector(frame)`: components 0 to 3 of the two frames,
    /// slerped (`Fast_Slerp`).
    pub fn get_quat_vector(&self, frame: f32) -> W3dResult<Quaternion> {
        let frame1 = f32_to_u32(frame)?;
        let frame2 = frame1.wrapping_add(1);
        let ratio = frame - frame1 as f32;
        let q1 = Quaternion::new(
            self.getframe(frame1, 0)?,
            self.getframe(frame1, 1)?,
            self.getframe(frame1, 2)?,
            self.getframe(frame1, 3)?,
        );
        let q2 = Quaternion::new(
            self.getframe(frame2, 0)?,
            self.getframe(frame2, 1)?,
            self.getframe(frame2, 2)?,
            self.getframe(frame2, 3)?,
        );
        Ok(fast_slerp(&q1, &q2, ratio))
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
    /// `CachedIdx` (0 after loading).
    pub cached_idx: EvalCache<u32>,
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
            cached_idx: EvalCache::new(0),
        }))
    }

    /// `Get_Pivot()`.
    pub fn get_pivot(&self) -> i32 {
        self.pivot_idx as i32
    }

    /// `Get_Bit(frame)`: the bit of the last time code at or before the
    /// frame (the first one's if none), scanning forward from the cached
    /// index when the frame is at or past its time code. Updates the cache.
    pub fn get_bit(&self, frame: i32) -> W3dResult<i32> {
        let bit = |i: i32| -> W3dResult<u32> {
            if i < 0 {
                return Err(oob());
            }
            self.bits.get(i as usize).copied().ok_or_else(oob)
        };
        let mut cached = self.cached_idx.lock();
        let mut idx: i32 = 0;
        let mut time = (bit(*cached as i32)? & !W3D_TIMECODED_BIT_MASK) as i32;
        if frame >= time {
            // start from here
            idx = (*cached as i32).wrapping_add(1);
        }
        while idx < self.num_time_codes as i32 {
            time = (bit(idx)? & !W3D_TIMECODED_BIT_MASK) as i32;
            if frame < time {
                break;
            }
            idx += 1;
        }
        idx = idx.wrapping_sub(1);
        if idx < 0 {
            idx = 0;
        }
        *cached = idx as u32;
        Ok(i32::from(bit(idx)? & W3D_TIMECODED_BIT_MASK == W3D_TIMECODED_BIT_MASK))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc(vector_len: i32, packets: &[(u32, &[f32])]) -> TimeCodedMotionChannel {
        let packet_size = (vector_len + 1) as u32;
        let mut data = Vec::new();
        for (t, v) in packets {
            data.push(*t);
            data.extend(v.iter().map(|f| f.to_bits()));
        }
        let n = packets.len() as u32;
        TimeCodedMotionChannel {
            pivot_idx: 1,
            type_: ANIM_CHANNEL_X,
            vector_len,
            packet_size,
            num_time_codes: n,
            last_time_code_idx: n.wrapping_sub(1).wrapping_mul(packet_size),
            data,
            cached_idx: EvalCache::new(0),
        }
    }

    fn x(c: &TimeCodedMotionChannel, frame: f32) -> W3dResult<f32> {
        let mut v = [f32::NAN];
        c.get_vector(frame, &mut v)?;
        Ok(v[0])
    }

    /// Predictions: frames 2..=4 hold 10, 20, 30; outside them the identity
    /// is 0 (one float) for a translation channel and (0,0,0,1) (four) for
    /// a quaternion channel.
    #[test]
    fn raw_channel_get_vector() {
        let c = MotionChannel { pivot_idx: 1, type_: ANIM_CHANNEL_X, vector_len: 1, first_frame: 2, last_frame: 4, data: vec![10.0, 20.0, 30.0] };
        let mut v = [9.0f32; 3];
        assert_eq!(c.get_vector(1, &mut v).unwrap(), 1);
        assert_eq!(v[0], 0.0);
        for (f, e) in [(2, 10.0), (3, 20.0), (4, 30.0), (5, 0.0)] {
            c.get_vector(f, &mut v).unwrap();
            assert_eq!(v[0], e, "frame {f}");
        }
        assert!(c.get_vector(3, &mut []).is_err(), "no room");
        let q = MotionChannel { type_: ANIM_CHANNEL_Q, vector_len: 4, first_frame: 0, last_frame: 0, data: vec![1.0, 2.0, 3.0, 4.0], ..c };
        let mut v = [9.0f32; 4];
        assert_eq!(q.get_vector(7, &mut v).unwrap(), 4);
        assert_eq!(v, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(q.get_vector_as_quat(0).unwrap(), Quaternion::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(q.get_vector_as_quat(-1).unwrap(), Quaternion::IDENTITY);
    }

    /// Predictions: frames 1..=10, bits 0b101 then 0b1: frame 1 (bit 0) is
    /// 1, frame 2 is 0, frame 3 is 1, frame 9 (byte 1, bit 0) is 1; frame 0
    /// and 11 are the default (7 here).
    #[test]
    fn raw_bit_channel() {
        let c = BitChannel { pivot_idx: 0, type_: BIT_CHANNEL_VIS, default_val: 7, first_frame: 1, last_frame: 10, bits: vec![0b101, 0b1] };
        let got: Vec<i32> = [0, 1, 2, 3, 9, 11].iter().map(|&f| c.get_bit(f).unwrap()).collect();
        assert_eq!(got, [7, 1, 0, 1, 1, 7]);
    }

    /// Packets (time, value): (0, 0), (10, 5), (20 | BINARY, 7), (30, 1).
    /// Predictions, by hand: frame 0 -> 0; frame 4 -> 0 + 5 * 0.4 = 2.0
    /// (0.4f32 * 5 rounds to 2); frame 10 -> packet 1, whose successor is a
    /// binary movement, so 5 held; 15 -> 5; 25 -> 7 + (1 - 7) * 0.5 = 4;
    /// 30 and 99 -> the last packet, 1. Going back to frame 4 after 25 needs
    /// the binary search (cache at packet 2 = word 4), still 2.0. Frame
    /// -0.5 converts to time code 0 and extrapolates: (-0.5 - 0) / 10 =
    /// -0.05, giving -0.25; frame -1.5 cannot be a `uint32` (undefined).
    #[test]
    fn time_coded_search_and_interpolation() {
        let c = tc(1, &[(0, &[0.0]), (10, &[5.0]), (20 | W3D_TIMECODED_BINARY_MOVEMENT_FLAG, &[7.0]), (30, &[1.0])]);
        for (f, e) in [(0.0, 0.0), (4.0, 2.0), (10.0, 5.0), (15.0, 5.0), (25.0, 4.0), (30.0, 1.0), (99.0, 1.0)] {
            assert_eq!(x(&c, f).unwrap(), e, "frame {f}");
        }
        assert_eq!(*c.cached_idx.lock(), 6, "the last packet");
        assert_eq!(x(&c, 4.0).unwrap(), 2.0);
        assert_eq!(*c.cached_idx.lock(), 0);
        assert_eq!(x(&c, -0.5).unwrap(), -0.25);
        assert_eq!(x(&c, -1.5), Err(W3dUndefined::FloatToInt.into()));
        assert_eq!(c.binary_search_index(25).unwrap(), 4);
        assert_eq!(c.binary_search_index(5).unwrap(), 0);
    }

    /// Prediction: two packets at times 5 and 10 and frame 0 (before the
    /// first): the search's right index is NumTimeCodes - 2 = 0, the middle
    /// is packet 0, whose time 5 is above 0, so `rightIdx = dx = 0` again:
    /// upstream loops forever.
    #[test]
    fn time_coded_search_can_loop_forever() {
        let c = tc(1, &[(5, &[1.0]), (10, &[2.0])]);
        assert_eq!(x(&c, 0.0), Err(W3dError::NonTerminating));
    }

    /// A quaternion channel: from the identity to a 90-degree turn about Z
    /// over 10 frames. Prediction: frame 5 is `Fast_Slerp` at 0.5, close to
    /// a 45-degree turn (z = 0.38268, w = 0.92388).
    #[test]
    fn time_coded_quaternion() {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let c = tc(4, &[(0, &[0.0, 0.0, 0.0, 1.0]), (10, &[0.0, 0.0, s, s])]);
        let q = c.get_quat_vector(5.0).unwrap();
        assert!((q.z - 0.382_683_43).abs() < 2e-4 && (q.w - 0.923_879_5).abs() < 2e-4, "{q:?}");
        assert_eq!(c.get_quat_vector(12.0).unwrap(), Quaternion::new(0.0, 0.0, s, s));
    }

    /// One component, 4 frames, base 1.0, Scale 1, one packet with filter
    /// index 8 (`filtertable[8]` = 1) and deltas +1, -2, +7, then zeros.
    /// Predictions: frames 0..=3 are 1, 2, 0, 7; frames past 3 clamp to 7;
    /// `Get_Vector(1.5)` = lerp(2, 0, 0.5) = 1. On a fresh channel, frame 0
    /// then frame 3 takes `getframe`'s last case, which continues from frame
    /// 1's value as if it were frame 0's: 2 + (1 - 2 + 7) = 8, not 7
    /// (upstream's bug). `Get_QuatVector` reads cache entries 0..3 of a
    /// two-entry cache: undefined.
    #[test]
    fn adaptive_delta_decompression() {
        let packet = [8u8, 0xE1, 0x07, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut data = vec![1.0f32.to_bits()];
        data.extend(packet.chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])));
        let make = || AdaptiveDeltaMotionChannel {
            pivot_idx: 1,
            type_: ANIM_CHANNEL_X,
            vector_len: 1,
            num_frames: 4,
            scale: 1.0,
            data: data.clone(),
            cache: EvalCache::new(AdaptiveDeltaCache { frame: 0x7FFF_FFFF, data: vec![0.0; 2] }),
        };
        let c = make();
        let got: Vec<f32> = (0..6).map(|f| c.getframe(f, 0).unwrap()).collect();
        assert_eq!(got, [1.0, 2.0, 0.0, 7.0, 7.0, 7.0]);
        let mut v = [0.0f32];
        c.get_vector(1.5, &mut v).unwrap();
        assert_eq!(v[0], 1.0);
        let c = make();
        assert_eq!(c.getframe(3, 0).unwrap(), 7.0, "fresh, from the start");
        let c = make();
        assert_eq!(c.getframe(0, 0).unwrap(), 1.0);
        assert_eq!(c.getframe(3, 0).unwrap(), 8.0, "upstream's double-counted delta");
        assert_eq!(make().get_quat_vector(0.0), Err(W3dUndefined::IndexOutOfRange.into()));
    }

    /// Two components, 20 frames, base (0, 0), Scale 1, every delta +1
    /// (filter index 8, nybbles 1) in two 16-frame groups. Prediction (the
    /// shared `done` flag): frame 18 from the start is (18, 16), component 1
    /// stopping after its first packet; correct decoding would give (18, 18).
    /// Frame 5 (inside the first packet) is right: (5, 5).
    #[test]
    fn adaptive_delta_shared_done_flag() {
        let mut bytes = Vec::new();
        for _ in 0..4 {
            bytes.push(8u8);
            bytes.extend([0x11u8; 8]);
        }
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        let mut data = vec![0.0f32.to_bits(), 0.0f32.to_bits()];
        data.extend(bytes.chunks(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])));
        let make = || AdaptiveDeltaMotionChannel {
            pivot_idx: 1,
            type_: ANIM_CHANNEL_X,
            vector_len: 2,
            num_frames: 20,
            scale: 1.0,
            data: data.clone(),
            cache: EvalCache::new(AdaptiveDeltaCache { frame: 0x7FFF_FFFF, data: vec![0.0; 4] }),
        };
        let c = make();
        assert_eq!((c.getframe(18, 0).unwrap(), c.getframe(18, 1).unwrap()), (18.0, 16.0));
        let c = make();
        assert_eq!((c.getframe(5, 0).unwrap(), c.getframe(5, 1).unwrap()), (5.0, 5.0));
    }

    /// Predictions: the sixteen powers of ten as written; entry 16 is
    /// `1 - sin(0)` = 1; the generated part decreases to about
    /// `1 - sin(89.625 degrees)` = 2.1e-5 at entry 255.
    #[test]
    fn adaptive_delta_filter_table_shape() {
        let t = adaptive_delta_filter_table();
        assert_eq!(t[8], 1.0);
        assert_eq!(t[15], 10_000_000.0);
        assert_eq!(t[16], 1.0);
        assert!(t[255] > 1e-5 && t[255] < 3e-5, "{}", t[255]);
        assert!(t[16..].windows(2).all(|w| w[1] <= w[0]));
    }

    /// Time codes (bit on/off): 0 on, 5 off, 10 on. Predictions: frames 0
    /// and 4 on, 5 and 9 off, 10 and 100 on; then frame 3 (before the cached
    /// time code 10) scans from 0 and finds packet 0, on; frame -5 also
    /// gives packet 0 (the index is clamped to 0), on.
    #[test]
    fn time_coded_bits() {
        let c = TimeCodedBitChannel {
            pivot_idx: 1,
            type_: BIT_CHANNEL_VIS,
            default_val: 0,
            num_time_codes: 3,
            bits: vec![W3D_TIMECODED_BIT_MASK, 5, 10 | W3D_TIMECODED_BIT_MASK],
            cached_idx: EvalCache::new(0),
        };
        let got: Vec<i32> = [0, 4, 5, 9, 10, 100, 3, -5].iter().map(|&f| c.get_bit(f).unwrap()).collect();
        assert_eq!(got, [1, 1, 0, 0, 1, 1, 1, 1]);
    }
}
