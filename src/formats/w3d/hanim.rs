// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hrawanim.cpp, hrawanim.h
//   (HRawAnimClass::Load_W3D, read_channel, add_channel, read_bit_channel,
//   add_bit_channel)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hcanim.cpp, hcanim.h
//   (HCompressedAnimClass::Load_W3D, read_channel, add_channel,
//   read_bit_channel, add_bit_channel)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, hanim by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Hierarchy animations: `W3D_CHUNK_ANIMATION` (raw, [`HRawAnim`]) and
//! `W3D_CHUNK_COMPRESSED_ANIMATION` ([`HCompressedAnim`]), as their
//! `Load_W3D` reads them. Only the loading; sampling an animation is not
//! translated yet. Morph animations (`W3D_CHUNK_MORPH_ANIMATION`,
//! `hmorphanim.cpp`) are not translated.
//!
//! # As upstream reads them
//!
//! Inside the animation chunk: a first chunk that must be the header
//! ([`W3dAnimHeaderStruct`], or [`W3dCompressedAnimHeaderStruct`] read with
//! the raw header's size, which is the same 44 bytes), then any chunks:
//! channels ([`super::motchan`]) and bit channels; anything else is skipped.
//! The animation's name is `HierarchyName + "." + Name`. The number of nodes
//! comes from the hierarchy the header names, which upstream looks up in the
//! asset manager (loading `<name>.w3d` if needed); here the caller supplies
//! the lookup. Each channel is stored at its pivot and type (a later one
//! replaces an earlier one); a channel whose pivot is not below the node
//! count is discarded, as is one of a type that has no slot. A compressed
//! animation reads its motion channels as time-coded or adaptive-delta
//! according to the header's `Flavor`, and skips them for any other flavor.
//!
//! # Upstream quirks that are kept
//!
//! - On a failure the chunks opened so far stay open (see
//!   [`super::htree`]); an unknown hierarchy fails the load.
//! - Pre-3.0 raw animations add 1 to each channel's pivot (after a
//!   successful channel load); compressed ones never do.
//! - The name is built with `strcpy`/`strcat` from the header on the stack:
//!   a 16-character `HierarchyName` or `Name` runs on into the following
//!   header fields up to their first 0 byte. `HierarchyName` is the header's
//!   16 bytes through `strncpy` (zero-padded after the first 0).
//! - `FrameRate` is the header's integer converted to `float`.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: a name that runs past the end of the header, or that
//! does not fit the 32-byte `Name`; a negative node count, or more nodes
//! than the 32-bit original can allocate; and the channel cases of
//! [`super::motchan`].
//!
//! **Upstream bug** ([`W3dUndefined::DoubleDelete`]): a compressed
//! animation whose load fails after its nodes are allocated (a channel that
//! fails to load) is undefined in the game. The error path's `Free()`
//! deletes `NodeMotion` without clearing the pointer, and the asset manager
//! then releases the animation, whose destructor calls `Free()` again: the
//! array is deleted twice. (`HRawAnimClass::Free` clears it; the raw loader
//! is fine.) Here such inputs are an error, not a load failure.

use std::collections::BTreeMap;

use super::motchan::*;
use super::w3d_file::*;
use super::{c_string, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::chunkio::ChunkLoad;

/// `sizeof(NodeMotionStruct)` on Win32 (8 pointers).
pub const WIN32_SIZEOF_NODE_MOTION: u64 = 32;
/// `sizeof(NodeCompressedMotionStruct)` on Win32 (`Flavor` and 5 pointers).
pub const WIN32_SIZEOF_NODE_COMPRESSED_MOTION: u64 = 24;

/// The lookup that stands in for `WW3DAssetManager::Get_HTree(name)`: the
/// `Num_Pivots()` of the hierarchy named by the 16 bytes (zero-padded), or
/// `None` if there is none.
pub trait HTreeLookup {
    fn num_pivots(&mut self, hierarchy_name: &[u8; W3D_NAME_LEN]) -> Option<i32>;
}

impl<F: FnMut(&[u8; W3D_NAME_LEN]) -> Option<i32>> HTreeLookup for F {
    fn num_pivots(&mut self, hierarchy_name: &[u8; W3D_NAME_LEN]) -> Option<i32> {
        self(hierarchy_name)
    }
}

/// `strcpy(Name, HierarchyName); strcat(Name, "."); strcat(Name, aName)` on
/// a header whose `Name` is at byte 4 and `HierarchyName` at byte 20, into a
/// zeroed 32-byte buffer; and `strncpy(HierarchyName, aheader.HierarchyName,
/// W3D_NAME_LEN)` into a zeroed 16-byte one.
fn anim_names(header: &[u8; 44]) -> W3dResult<([u8; 2 * W3D_NAME_LEN], [u8; W3D_NAME_LEN])> {
    let hier = c_string(header, 20)?;
    let name = c_string(header, 4)?;
    let len = hier.len() + 1 + name.len();
    if len + 1 > 2 * W3D_NAME_LEN {
        return Err(W3dUndefined::StringOverflow.into());
    }
    let mut full = [0u8; 2 * W3D_NAME_LEN];
    full[..hier.len()].copy_from_slice(hier);
    full[hier.len()] = b'.';
    full[hier.len() + 1..len].copy_from_slice(name);
    let mut h = [0u8; W3D_NAME_LEN];
    for (i, &c) in header[20..36].iter().enumerate() {
        if c == 0 {
            break;
        }
        h[i] = c;
    }
    Ok((full, h))
}

fn node_count(lookup: &mut impl HTreeLookup, name: &[u8; W3D_NAME_LEN], elem: u64) -> W3dResult<Option<i32>> {
    let n = match lookup.num_pivots(name) {
        Some(n) => n,
        None => return Ok(None),
    };
    if n < 0 {
        return Err(W3dUndefined::Allocation.into());
    }
    if n as u64 * elem > WIN32_MAX_ALLOCATION {
        return Err(W3dUndefined::Allocation.into());
    }
    Ok(Some(n))
}

/// `struct NodeMotionStruct`: the channels of one node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeMotion {
    pub x: Option<MotionChannel>,
    pub y: Option<MotionChannel>,
    pub z: Option<MotionChannel>,
    pub xr: Option<MotionChannel>,
    pub yr: Option<MotionChannel>,
    pub zr: Option<MotionChannel>,
    pub q: Option<MotionChannel>,
    pub vis: Option<BitChannel>,
}

/// `class HRawAnimClass` as loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct HRawAnim {
    /// `Name`: `"<hierarchy>.<name>"`, zero-padded.
    pub name: [u8; 2 * W3D_NAME_LEN],
    /// `HierarchyName`, zero-padded.
    pub hierarchy_name: [u8; W3D_NAME_LEN],
    pub num_frames: i32,
    pub num_nodes: i32,
    pub frame_rate: f32,
    /// `NodeMotion[i]` for the nodes that have a channel (the others have
    /// none).
    pub node_motion: BTreeMap<u32, NodeMotion>,
}

impl HRawAnim {
    /// `HRawAnimClass::Load_W3D(cload)`, inside the animation chunk.
    /// `Ok(None)` is upstream's `LOAD_ERROR`.
    pub fn load_w3d(cload: &mut ChunkLoad<'_>, htrees: &mut impl HTreeLookup) -> W3dResult<Option<Self>> {
        if !cload.open_chunk()? {
            return Ok(None);
        }
        if cload.cur_chunk_id()? != W3D_CHUNK_ANIMATION_HEADER {
            return Ok(None);
        }
        let mut hb = [0u8; W3dAnimHeaderStruct::SIZE];
        if cload.read_into(&mut hb)? != W3dAnimHeaderStruct::SIZE as u32 {
            return Ok(None);
        }
        cload.close_chunk()?;
        let aheader = W3dAnimHeaderStruct::from_bytes(&hb);
        let pre30 = aheader.version < w3d_make_version(3, 0);
        let (name, hierarchy_name) = anim_names(&hb)?;
        let num_nodes = match node_count(htrees, &hierarchy_name, WIN32_SIZEOF_NODE_MOTION)? {
            Some(n) => n,
            None => return Ok(None),
        };
        let mut anim = HRawAnim {
            name,
            hierarchy_name,
            num_frames: aheader.num_frames as i32,
            num_nodes,
            frame_rate: aheader.frame_rate as f32,
            node_motion: BTreeMap::new(),
        };
        while cload.open_chunk()? {
            match cload.cur_chunk_id()? {
                W3D_CHUNK_ANIMATION_CHANNEL => {
                    let mut chan = match MotionChannel::load_w3d(cload)? {
                        Some(c) => c,
                        None => return Ok(None),
                    };
                    if pre30 {
                        chan.pivot_idx = (chan.get_pivot() + 1) as u32;
                    }
                    if chan.get_pivot() < anim.num_nodes {
                        anim.add_channel(chan);
                    }
                }
                W3D_CHUNK_BIT_CHANNEL => {
                    let mut chan = match BitChannel::load_w3d(cload)? {
                        Some(c) => c,
                        None => return Ok(None),
                    };
                    if pre30 {
                        chan.pivot_idx = chan.pivot_idx.wrapping_add(1);
                    }
                    if chan.get_pivot() < anim.num_nodes {
                        anim.add_bit_channel(chan);
                    }
                }
                _ => {}
            }
            cload.close_chunk()?;
        }
        Ok(Some(anim))
    }

    fn add_channel(&mut self, chan: MotionChannel) {
        let t = chan.type_;
        if t > ANIM_CHANNEL_Q {
            return; // no slot: the channel is dropped (leaked upstream)
        }
        let m = self.node_motion.entry(chan.get_pivot() as u32).or_default();
        let slot = match t {
            ANIM_CHANNEL_X => &mut m.x,
            ANIM_CHANNEL_Y => &mut m.y,
            ANIM_CHANNEL_Z => &mut m.z,
            ANIM_CHANNEL_XR => &mut m.xr,
            ANIM_CHANNEL_YR => &mut m.yr,
            ANIM_CHANNEL_ZR => &mut m.zr,
            _ => &mut m.q,
        };
        *slot = Some(chan);
    }

    fn add_bit_channel(&mut self, chan: BitChannel) {
        if chan.type_ == BIT_CHANNEL_VIS {
            let p = chan.get_pivot() as u32;
            self.node_motion.entry(p).or_default().vis = Some(chan);
        }
    }
}

/// A motion channel of a compressed animation (`NodeCompressedMotionStruct`
/// holds them in a union by the animation's flavor).
#[derive(Clone, Debug, PartialEq)]
pub enum CompressedChannel {
    TimeCoded(TimeCodedMotionChannel),
    AdaptiveDelta(AdaptiveDeltaMotionChannel),
}

/// `struct NodeCompressedMotionStruct`: the channels of one node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeCompressedMotion {
    pub x: Option<CompressedChannel>,
    pub y: Option<CompressedChannel>,
    pub z: Option<CompressedChannel>,
    pub q: Option<CompressedChannel>,
    pub vis: Option<TimeCodedBitChannel>,
}

/// `class HCompressedAnimClass` as loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct HCompressedAnim {
    pub name: [u8; 2 * W3D_NAME_LEN],
    pub hierarchy_name: [u8; W3D_NAME_LEN],
    pub num_frames: i32,
    pub num_nodes: i32,
    pub frame_rate: f32,
    /// `Flavor` (`ANIM_FLAVOR_TIMECODED`, `ANIM_FLAVOR_ADAPTIVE_DELTA`, or
    /// any other value read).
    pub flavor: i32,
    /// `NodeMotion[i]` for the nodes that have a channel.
    pub node_motion: BTreeMap<u32, NodeCompressedMotion>,
}

impl HCompressedAnim {
    /// `HCompressedAnimClass::Load_W3D(cload)`, inside the animation chunk.
    /// `Ok(None)` is upstream's `LOAD_ERROR`.
    pub fn load_w3d(cload: &mut ChunkLoad<'_>, htrees: &mut impl HTreeLookup) -> W3dResult<Option<Self>> {
        if !cload.open_chunk()? {
            return Ok(None);
        }
        if cload.cur_chunk_id()? != W3D_CHUNK_COMPRESSED_ANIMATION_HEADER {
            return Ok(None);
        }
        // read with sizeof(W3dAnimHeaderStruct), which is the same 44 bytes
        let mut hb = [0u8; W3dAnimHeaderStruct::SIZE];
        if cload.read_into(&mut hb)? != W3dAnimHeaderStruct::SIZE as u32 {
            return Ok(None);
        }
        cload.close_chunk()?;
        let aheader = W3dCompressedAnimHeaderStruct::from_bytes(&hb);
        let (name, hierarchy_name) = anim_names(&hb)?;
        let num_nodes = match node_count(htrees, &hierarchy_name, WIN32_SIZEOF_NODE_COMPRESSED_MOTION)? {
            Some(n) => n,
            None => return Ok(None),
        };
        let mut anim = HCompressedAnim {
            name,
            hierarchy_name,
            num_frames: aheader.num_frames as i32,
            num_nodes,
            frame_rate: f32::from(aheader.frame_rate),
            flavor: i32::from(aheader.flavor),
            node_motion: BTreeMap::new(),
        };
        while cload.open_chunk()? {
            match cload.cur_chunk_id()? {
                W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL => match anim.flavor as u32 {
                    ANIM_FLAVOR_TIMECODED => {
                        let chan = match TimeCodedMotionChannel::load_w3d(cload)? {
                            Some(c) => c,
                            None => return Err(W3dUndefined::DoubleDelete.into()),
                        };
                        if chan.get_pivot() < anim.num_nodes {
                            let (p, t) = (chan.get_pivot() as u32, chan.type_);
                            anim.add_channel(p, t, CompressedChannel::TimeCoded(chan));
                        }
                    }
                    ANIM_FLAVOR_ADAPTIVE_DELTA => {
                        let chan = match AdaptiveDeltaMotionChannel::load_w3d(cload)? {
                            Some(c) => c,
                            None => return Err(W3dUndefined::DoubleDelete.into()),
                        };
                        if chan.get_pivot() < anim.num_nodes {
                            let (p, t) = (chan.get_pivot() as u32, chan.type_);
                            anim.add_channel(p, t, CompressedChannel::AdaptiveDelta(chan));
                        }
                    }
                    _ => {}
                },
                W3D_CHUNK_COMPRESSED_BIT_CHANNEL => {
                    let chan = match TimeCodedBitChannel::load_w3d(cload)? {
                        Some(c) => c,
                        None => return Err(W3dUndefined::DoubleDelete.into()),
                    };
                    if chan.get_pivot() < anim.num_nodes && chan.type_ == BIT_CHANNEL_VIS {
                        let p = chan.get_pivot() as u32;
                        anim.node_motion.entry(p).or_default().vis = Some(chan);
                    }
                }
                _ => {}
            }
            cload.close_chunk()?;
        }
        Ok(Some(anim))
    }

    fn add_channel(&mut self, pivot: u32, type_: u32, chan: CompressedChannel) {
        if !matches!(type_, ANIM_CHANNEL_X | ANIM_CHANNEL_Y | ANIM_CHANNEL_Z | ANIM_CHANNEL_Q) {
            return; // no slot: the channel is dropped (leaked upstream)
        }
        let m = self.node_motion.entry(pivot).or_default();
        let slot = match type_ {
            ANIM_CHANNEL_X => &mut m.x,
            ANIM_CHANNEL_Y => &mut m.y,
            ANIM_CHANNEL_Z => &mut m.z,
            _ => &mut m.q,
        };
        *slot = Some(chan);
    }
}
