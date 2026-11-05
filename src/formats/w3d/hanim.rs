// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hrawanim.cpp, hrawanim.h
//   (HRawAnimClass::Load_W3D, read_channel, add_channel, read_bit_channel,
//   add_bit_channel; Get_Translation, Get_Orientation, Get_Transform,
//   Get_Visibility, Is_Node_Motion_Present, Has_*)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hcanim.cpp, hcanim.h
//   (HCompressedAnimClass::Load_W3D, read_channel, add_channel,
//   read_bit_channel, add_bit_channel; Get_Translation, Get_Orientation,
//   Get_Transform, Get_Visibility, Is_Node_Motion_Present, Has_*)
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
//! `Load_W3D` reads them, ~~Only the loading; sampling an animation is not
//! translated yet.~~ **Corrected 2026-11-05 (#10, step 3):** and their
//! sampling: `Get_Translation`, `Get_Orientation`, `Get_Transform` and
//! `Get_Visibility` of a pivot at a (fractional) frame. Morph animations (`W3D_CHUNK_MORPH_ANIMATION`,
//! `hmorphanim.cpp`) ~~are not translated~~ **Corrected 2026-11-05:** are
//! loaded by [`super::hmorphanim`].
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
//! # Sampling, as upstream samples
//!
//! - **Raw** ([`HRawAnim`]): `frame0 = WWMath::Float_To_Long(frame -
//!   0.499999f)`, `frame1 = frame0 + 1` (0 when it reaches `NumFrames`),
//!   `ratio = frame - frame0`; each channel is read at the two integer
//!   frames ([`MotionChannel::get_vector`]) and the translation interpolated
//!   linearly, the orientation with `Fast_Slerp` (not when `ratio` is 0, nor
//!   for the orientation when it is 1). `Get_Orientation` reads the
//!   quaternion with `Get_Vector_As_Quat`, `Get_Transform` with
//!   `Get_Vector`. Visibility is the bit channel at `(int)frame`, else
//!   visible.
//! - **Compressed** ([`HCompressedAnim`]): each channel evaluates the
//!   fractional frame itself ([`TimeCodedMotionChannel`],
//!   [`AdaptiveDeltaMotionChannel`]); a missing channel gives 0 (identity
//!   for the orientation).
//!
//! `Float_To_Long` is the portable branch of `wwmath.h` (`(long)f`, a
//! truncation; see [`crate::ww::wwmath`]), as everywhere in this
//! translation. **The shipped Win32 game differs here, visibly:** its
//! `fistp` rounds to nearest, so it samples frames `round(frame - 0.5)`
//! and `+1` with `ratio` in `[0, 1)`; the portable truncation gives `frame0
//! = floor(frame) - 1` when the fraction is below 0.499999 (for `frame >=
//! 1`), and then `ratio` in `[1, 1.5)`: the translation is *extrapolated*
//! from the previous key pair, the orientation slerped past `q1`. Integer
//! frames `N >= 1` come out as `lerp(v[N-1], v[N], 1)`, which can differ
//! from `v[N]` in the last bit. Recorded, open for review (issue #10).
//!
//! # Upstream quirks that are kept
//!
//! - `Get_Vector` writes `VectorLen` floats from `&trans[i]` or
//!   `&mtx[i][3]`: a channel with a longer vector runs on into the next
//!   components of the same `Vector3` or `Matrix3D` (and a later channel
//!   overwrites them). Only a write past the object is undefined.
//! - Raw `Get_Transform` reads the quaternion with `Get_Vector` into a
//!   `float vals[4]` (fewer than four floats leave the rest uninitialised:
//!   undefined), `Get_Orientation` with `Get_Vector_As_Quat` (four floats
//!   from the data whatever `VectorLen`).
//! - Raw `Get_Translation` returns 0 at once when no translation channel
//!   exists; otherwise it uses the interpolation even for out-of-range
//!   frames (the channels give their identity there).
//! - A compressed animation of an unknown flavor (no channels were loaded)
//!   gives a zero translation and leaves the orientation or transform
//!   passed in unchanged (`WWASSERT(0)` is empty in release); here that is
//!   `None`.
//! - The channels' caches (see [`super::motchan`]) change on every
//!   evaluation, through upstream's `const` functions.
//!
//! # Upstream quirks that are kept (loading)
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
//! Sampling ([`W3dUndefined`]): a pivot index outside `0..NumNodes`
//! ([`W3dUndefined::IndexOutOfRange`]), a frame whose integer conversion
//! does not fit ([`W3dUndefined::FloatToInt`]: `(int)frame`, Win32's 32-bit
//! `long`, or `uint32` for the compressed channels, so negative frames of a
//! compressed animation), the uninitialised quaternion of raw
//! `Get_Transform` ([`W3dUndefined::UninitialisedRead`]), and the channel
//! cases of [`super::motchan`].
//!
//! Loading: [`W3dUndefined`]: a name that runs past the end of the header, or that
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
use super::{c_string, f32_to_i32, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::chunkio::ChunkLoad;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::quat::{build_matrix3d, fast_slerp, Quaternion};
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// A `Matrix3D` as its 12 floats, row-major (`mtx[r][c]` is `[4 * r + c]`),
/// for `Get_Vector`'s pointer into it.
fn mtx_floats(m: &Matrix3D) -> [f32; 12] {
    m.to_array()
}

fn mtx_from_floats(f: &[f32; 12]) -> Matrix3D {
    Matrix3D::new(f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[11])
}

/// `Vector3::Lerp(a, b, alpha, &result)`.
fn lerp3(a: &Vector3, b: &Vector3, alpha: f32) -> Vector3 {
    Vector3::new(a.x + (b.x - a.x) * alpha, a.y + (b.y - a.y) * alpha, a.z + (b.z - a.z) * alpha)
}

/// The pivot index as an index into `NodeMotion[NumNodes]`.
fn node_index(pividx: i32, num_nodes: i32) -> W3dResult<u32> {
    if pividx < 0 || pividx >= num_nodes {
        return Err(W3dUndefined::IndexOutOfRange.into());
    }
    Ok(pividx as u32)
}

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

    /// `NodeMotion[pividx]` (a node without channels is the default).
    fn node(&self, pividx: i32) -> W3dResult<std::borrow::Cow<'_, NodeMotion>> {
        let i = node_index(pividx, self.num_nodes)?;
        Ok(match self.node_motion.get(&i) {
            Some(m) => std::borrow::Cow::Borrowed(m),
            None => std::borrow::Cow::Owned(NodeMotion::default()),
        })
    }

    /// `Get_Num_Pivots()`.
    pub fn get_num_pivots(&self) -> i32 {
        self.num_nodes
    }

    /// `Get_Num_Frames()`.
    pub fn get_num_frames(&self) -> i32 {
        self.num_frames
    }

    /// `Get_Node_Motion_Array()[pividx]`, for `HTreeClass::Anim_Update`'s
    /// raw fast path.
    pub fn node_motion_at(&self, pividx: i32) -> W3dResult<std::borrow::Cow<'_, NodeMotion>> {
        self.node(pividx)
    }

    /// The integer frames and ratio of the raw sampling (see the module
    /// docs): `frame0`, `frame1`, `ratio`.
    fn frames(&self, frame: f32) -> W3dResult<(i32, i32, f32)> {
        // WWMath::Float_To_Long (portable branch, `(long) f`): undefined
        // outside a 32-bit long, else `wwmath::float_to_long`
        let f = frame - 0.499_999f32;
        f32_to_i32(f)?;
        let frame0 = wwmath::float_to_long(f);
        let mut frame1 = frame0 + 1;
        let ratio = frame - frame0 as f32;
        if frame1 >= self.num_frames {
            frame1 = 0;
        }
        Ok((frame0, frame1, ratio))
    }

    /// The translation channels at one integer frame, into a zero vector.
    fn translation_at(m: &NodeMotion, frame: i32) -> W3dResult<Vector3> {
        let mut t = [0.0f32; 3];
        if let Some(x) = &m.x {
            x.get_vector(frame, &mut t[0..])?;
        }
        if let Some(y) = &m.y {
            y.get_vector(frame, &mut t[1..])?;
        }
        if let Some(z) = &m.z {
            z.get_vector(frame, &mut t[2..])?;
        }
        Ok(Vector3::new(t[0], t[1], t[2]))
    }

    /// `Get_Translation(trans, pividx, frame)`.
    pub fn get_translation(&self, pividx: i32, frame: f32) -> W3dResult<Vector3> {
        let m = self.node(pividx)?;
        if m.x.is_none() && m.y.is_none() && m.z.is_none() {
            return Ok(Vector3::new(0.0, 0.0, 0.0));
        }
        let (frame0, frame1, ratio) = self.frames(frame)?;
        let trans0 = Self::translation_at(&m, frame0)?;
        if ratio == 0.0f32 {
            return Ok(trans0);
        }
        let trans1 = Self::translation_at(&m, frame1)?;
        Ok(lerp3(&trans0, &trans1, ratio))
    }

    /// `Get_Orientation(q, pividx, frame)` (the `SPECIAL_GETVEC_AS_QUAT`
    /// branch).
    pub fn get_orientation(&self, pividx: i32, frame: f32) -> W3dResult<Quaternion> {
        let (frame0, frame1, ratio) = self.frames(frame)?;
        let m = self.node(pividx)?;
        let (q0, q1) = match &m.q {
            Some(mc) => (mc.get_vector_as_quat(frame0)?, mc.get_vector_as_quat(frame1)?),
            None => (Quaternion::IDENTITY, Quaternion::IDENTITY),
        };
        if ratio == 0.0f32 {
            Ok(q0)
        } else if ratio == 1.0f32 {
            Ok(q1)
        } else {
            Ok(fast_slerp(&q0, &q1, ratio))
        }
    }

    /// `Q->Get_Vector(frame, vals); q.Set(vals[0..4])` of `Get_Transform`.
    fn quat_vals(m: &NodeMotion, frame: i32) -> W3dResult<Quaternion> {
        match &m.q {
            None => Ok(Quaternion::IDENTITY),
            Some(q) => {
                let mut vals = [0.0f32; 4];
                if q.get_vector(frame, &mut vals)? < 4 {
                    return Err(W3dUndefined::UninitialisedRead.into());
                }
                Ok(Quaternion::new(vals[0], vals[1], vals[2], vals[3]))
            }
        }
    }

    /// `Get_Transform(mtx, pividx, frame)`.
    pub fn get_transform(&self, pividx: i32, frame: f32) -> W3dResult<Matrix3D> {
        let m = self.node(pividx)?;
        let (frame0, frame1, ratio) = self.frames(frame)?;
        let q0 = Self::quat_vals(&m, frame0)?;
        if ratio == 0.0f32 {
            let mut f = mtx_floats(&build_matrix3d(&q0));
            if let Some(x) = &m.x {
                x.get_vector(frame0, &mut f[3..])?;
            }
            if let Some(y) = &m.y {
                y.get_vector(frame0, &mut f[7..])?;
            }
            if let Some(z) = &m.z {
                z.get_vector(frame0, &mut f[11..])?;
            }
            return Ok(mtx_from_floats(&f));
        }
        let q1 = Self::quat_vals(&m, frame1)?;
        let q = fast_slerp(&q0, &q1, ratio);
        let mut mtx = build_matrix3d(&q);
        let trans0 = Self::translation_at(&m, frame0)?;
        let trans1 = Self::translation_at(&m, frame1)?;
        let trans = lerp3(&trans0, &trans1, ratio);
        mtx.set_translation(&trans);
        Ok(mtx)
    }

    /// `Get_Visibility(pividx, frame)`: the bit channel at `(int)frame`, else
    /// visible.
    pub fn get_visibility(&self, pividx: i32, frame: f32) -> W3dResult<bool> {
        let m = self.node(pividx)?;
        match &m.vis {
            Some(v) => Ok(v.get_bit(f32_to_i32(frame)?)? == 1),
            None => Ok(true),
        }
    }

    /// `Is_Node_Motion_Present(pividx)`.
    pub fn is_node_motion_present(&self, pividx: i32) -> W3dResult<bool> {
        let m = self.node(pividx)?;
        Ok(m.x.is_some()
            || m.y.is_some()
            || m.z.is_some()
            || m.xr.is_some()
            || m.yr.is_some()
            || m.zr.is_some()
            || m.q.is_some()
            || m.vis.is_some())
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

    /// `NodeMotion[pividx]`.
    fn node(&self, pividx: i32) -> W3dResult<std::borrow::Cow<'_, NodeCompressedMotion>> {
        let i = node_index(pividx, self.num_nodes)?;
        Ok(match self.node_motion.get(&i) {
            Some(m) => std::borrow::Cow::Borrowed(m),
            None => std::borrow::Cow::Owned(NodeCompressedMotion::default()),
        })
    }

    /// `Get_Num_Pivots()`.
    pub fn get_num_pivots(&self) -> i32 {
        self.num_nodes
    }

    /// `Get_Num_Frames()`.
    pub fn get_num_frames(&self) -> i32 {
        self.num_frames
    }

    fn known_flavor(&self) -> bool {
        matches!(self.flavor as u32, ANIM_FLAVOR_TIMECODED | ANIM_FLAVOR_ADAPTIVE_DELTA)
    }

    /// `chan->Get_Vector(frame, out)` of either kind.
    fn vector(c: &Option<CompressedChannel>, frame: f32, out: &mut [f32]) -> W3dResult<()> {
        match c {
            None => Ok(()),
            Some(CompressedChannel::TimeCoded(c)) => c.get_vector(frame, out),
            Some(CompressedChannel::AdaptiveDelta(c)) => c.get_vector(frame, out),
        }
    }

    /// `Q->Get_QuatVector(frame)`, or `None` without a channel.
    fn quat(c: &Option<CompressedChannel>, frame: f32) -> W3dResult<Option<Quaternion>> {
        match c {
            None => Ok(None),
            Some(CompressedChannel::TimeCoded(c)) => c.get_quat_vector(frame).map(Some),
            Some(CompressedChannel::AdaptiveDelta(c)) => c.get_quat_vector(frame).map(Some),
        }
    }

    /// `Get_Translation(trans, pividx, frame)`.
    pub fn get_translation(&self, pividx: i32, frame: f32) -> W3dResult<Vector3> {
        let m = self.node(pividx)?;
        let mut t = [0.0f32; 3];
        if self.known_flavor() {
            Self::vector(&m.x, frame, &mut t[0..])?;
            Self::vector(&m.y, frame, &mut t[1..])?;
            Self::vector(&m.z, frame, &mut t[2..])?;
        }
        Ok(Vector3::new(t[0], t[1], t[2]))
    }

    /// `Get_Orientation(q, pividx, frame)`: `None` for an unknown flavor (the
    /// caller's quaternion is left as it was).
    pub fn get_orientation(&self, pividx: i32, frame: f32) -> W3dResult<Option<Quaternion>> {
        let m = self.node(pividx)?;
        if !self.known_flavor() {
            return Ok(None);
        }
        Ok(Some(Self::quat(&m.q, frame)?.unwrap_or(Quaternion::IDENTITY)))
    }

    /// `Get_Transform(mtx, pividx, frame)`: `None` for an unknown flavor (the
    /// caller's matrix is left as it was).
    pub fn get_transform(&self, pividx: i32, frame: f32) -> W3dResult<Option<Matrix3D>> {
        let m = self.node(pividx)?;
        if !self.known_flavor() {
            return Ok(None);
        }
        let mtx = match Self::quat(&m.q, frame)? {
            Some(q) => build_matrix3d(&q),
            None => {
                let mut i = Matrix3D::default();
                i.make_identity();
                i
            }
        };
        let mut f = mtx_floats(&mtx);
        Self::vector(&m.x, frame, &mut f[3..])?;
        Self::vector(&m.y, frame, &mut f[7..])?;
        Self::vector(&m.z, frame, &mut f[11..])?;
        Ok(Some(mtx_from_floats(&f)))
    }

    /// `Get_Visibility(pividx, frame)`: the bit channel at `(int)frame`, else
    /// visible.
    pub fn get_visibility(&self, pividx: i32, frame: f32) -> W3dResult<bool> {
        let m = self.node(pividx)?;
        match &m.vis {
            Some(v) => Ok(v.get_bit(f32_to_i32(frame)?)? == 1),
            None => Ok(true),
        }
    }

    /// `Is_Node_Motion_Present(pividx)`.
    pub fn is_node_motion_present(&self, pividx: i32) -> W3dResult<bool> {
        let m = self.node(pividx)?;
        Ok(m.x.is_some() || m.y.is_some() || m.z.is_some() || m.q.is_some() || m.vis.is_some())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::formats::w3d::EvalCache;

    pub(crate) const S: f32 = std::f32::consts::FRAC_1_SQRT_2;

    fn name32(s: &[u8]) -> [u8; 2 * W3D_NAME_LEN] {
        let mut n = [0u8; 2 * W3D_NAME_LEN];
        n[..s.len()].copy_from_slice(s);
        n
    }

    /// A raw animation "SKL.WALK" of 3 frames over 2 nodes: node 1 has X =
    /// 0, 10, 40 and Q = identity, identity, 90 degrees about Z.
    pub(crate) fn raw_anim() -> HRawAnim {
        let x = MotionChannel { pivot_idx: 1, type_: ANIM_CHANNEL_X, vector_len: 1, first_frame: 0, last_frame: 2, data: vec![0.0, 10.0, 40.0] };
        let q = MotionChannel {
            pivot_idx: 1,
            type_: ANIM_CHANNEL_Q,
            vector_len: 4,
            first_frame: 0,
            last_frame: 2,
            data: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, S, S],
        };
        let mut node_motion = BTreeMap::new();
        node_motion.insert(1, NodeMotion { x: Some(x), q: Some(q), ..Default::default() });
        let mut h = [0u8; W3D_NAME_LEN];
        h[..3].copy_from_slice(b"SKL");
        HRawAnim { name: name32(b"SKL.WALK"), hierarchy_name: h, num_frames: 3, num_nodes: 2, frame_rate: 30.0, node_motion }
    }

    /// Predictions (portable `Float_To_Long`, a truncation, of `frame -
    /// 0.499999`): frame 0 -> frames 0/1, ratio 0: 0. Frame 0.25 -> 0.25 of
    /// the way: 2.5. Frame 1.0 -> 0.500001 truncates to 0, ratio 1: 10.
    /// Frame 1.25 -> still frame 0, ratio 1.25: extrapolated, 12.5 (the
    /// shipped game's rounding would give frames 1/2 at 0.25: 17.5). Frame
    /// 1.75 -> frame 1, ratio 0.75: 32.5. Frame 2.5 -> frame 2 and frame 3,
    /// which wraps to 0: lerp(40, 0, 0.5) = 20. Node 0 has no channels: 0.
    #[test]
    fn raw_translation() {
        let a = raw_anim();
        for (f, e) in [(0.0, 0.0), (0.25, 2.5), (1.0, 10.0), (1.25, 12.5), (1.75, 32.5), (2.5, 20.0)] {
            assert_eq!(a.get_translation(1, f).unwrap(), Vector3::new(e, 0.0, 0.0), "frame {f}");
        }
        assert_eq!(a.get_translation(0, 1.5).unwrap(), Vector3::new(0.0, 0.0, 0.0));
        assert_eq!(a.get_translation(2, 0.0), Err(W3dUndefined::IndexOutOfRange.into()));
        assert!(a.get_visibility(1, 1.0).unwrap());
    }

    /// Predictions: at frame 2.0 (frame 1, ratio 1.0) `Get_Orientation`
    /// returns q1 exactly, the 90-degree turn; frame 1.75 slerps 0.75 of the
    /// way from the identity (z close to sin(33.75 deg) = 0.55557).
    /// `Get_Transform` at frame 2.0 slerps at ratio 1 (not special-cased)
    /// and puts the translation 10 + 30 * 1 = 40 in `[0][3]`.
    #[test]
    fn raw_orientation_and_transform() {
        let a = raw_anim();
        assert_eq!(a.get_orientation(1, 2.0).unwrap(), Quaternion::new(0.0, 0.0, S, S));
        let q = a.get_orientation(1, 1.75).unwrap();
        assert!((q.z - 0.555_570_2).abs() < 2e-4, "{q:?}");
        let m = a.get_transform(1, 2.0).unwrap();
        assert_eq!(m[0][3], 40.0);
        assert!((m[1][0] - 1.0).abs() < 1e-3 && m[0][0].abs() < 1e-3, "{m:?}");
    }

    /// A time-coded compressed animation, X: (0, 0), (10, 5). Predictions:
    /// frame 4 gives 2.0, frame 20 gives 5; no Q channel: the identity; a
    /// negative frame cannot be a `uint32`. An unknown flavor: no
    /// translation, orientation left alone (`None`).
    #[test]
    fn compressed_sampling() {
        let x = TimeCodedMotionChannel {
            pivot_idx: 1,
            type_: ANIM_CHANNEL_X,
            vector_len: 1,
            packet_size: 2,
            num_time_codes: 2,
            last_time_code_idx: 2,
            data: vec![0, 0.0f32.to_bits(), 10, 5.0f32.to_bits()],
            cached_idx: EvalCache::new(0),
        };
        let mut node_motion = BTreeMap::new();
        node_motion.insert(1, NodeCompressedMotion { x: Some(CompressedChannel::TimeCoded(x)), ..Default::default() });
        let mut a = HCompressedAnim {
            name: name32(b"SKL.C"),
            hierarchy_name: [0; W3D_NAME_LEN],
            num_frames: 11,
            num_nodes: 2,
            frame_rate: 30.0,
            flavor: ANIM_FLAVOR_TIMECODED as i32,
            node_motion,
        };
        assert_eq!(a.get_translation(1, 4.0).unwrap().x, 2.0);
        assert_eq!(a.get_translation(1, 20.0).unwrap().x, 5.0);
        assert_eq!(a.get_orientation(1, 3.0).unwrap(), Some(Quaternion::IDENTITY));
        assert_eq!(a.get_transform(1, 4.0).unwrap().unwrap()[0][3], 2.0);
        assert!(a.get_translation(1, -2.0).is_err());
        a.flavor = 7;
        assert_eq!(a.get_translation(1, 4.0).unwrap().x, 0.0);
        assert_eq!(a.get_orientation(1, 4.0).unwrap(), None);
    }
}
