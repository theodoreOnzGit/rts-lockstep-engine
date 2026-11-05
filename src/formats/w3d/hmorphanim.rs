// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hmorphanim.cpp
//   (HMorphAnimClass: constructor, Load_W3D, read_channel;
//   TimeCodedMorphKeysClass: constructor, Free, Load_W3D) and hmorphanim.h
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/simplevec.h
//   (SimpleDynVecClass::Add and Grow: the allocation sizes)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/wwstring.h
//   (StringClass(), Get_Buffer: the pose name's buffer)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, hmorphanim by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D morph animations (`W3D_CHUNK_MORPH_ANIMATION`):
//! `HMorphAnimClass::Load_W3D`, which reads an animation that morphs a
//! hierarchy between poses taken from other animations: a header, then per
//! morph channel the name of its pose animation and its keys (which pose
//! frame to use from which frame on), and which channel drives each pivot.
//! Only the loading; evaluating the animation (`Get_Transform`, ...), the
//! text import (`Import`) and saving are not translated.
//!
//! # As upstream reads it
//!
//! Inside the morph chunk: a first chunk read as the header
//! ([`W3dMorphAnimHeaderStruct`]; its id is only asserted, so any id is
//! read). The animation's name is `HierarchyName + "." + Name`. The number
//! of pivots comes from the hierarchy the header names, which upstream
//! looks up in the asset manager; here the caller supplies the lookup
//! ([`HTreeLookup`], as for [`super::hanim`]); an unknown hierarchy fails the
//! load. Then any chunks: the n-th `W3D_CHUNK_MORPHANIM_CHANNEL` is morph
//! channel n (a first sub-chunk read as the pose name, a second as the keys:
//! `length / 8` [`W3dMorphAnimKeyStruct`]s); each
//! `W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA` is read over the pivot-channel
//! array (`uint32` per pivot, zeroed first). Others are skipped.
//!
//! # Upstream quirks that are kept
//!
//! Almost nothing is checked; only the hierarchy lookup can fail the load.
//! - The results of `Open_Chunk` are ignored: when the header's, a pose
//!   name's or a key chunk's open fails, the read goes to the chunk around
//!   it and the following `Close_Chunk` closes that one, so the reader goes
//!   on one level out (as [`super::hlod`]'s failed LOD header, but deeper).
//!   No chunk id inside the animation is checked except the two the loop
//!   switches on.
//! - The names are copied with `strncpy` (16 bytes, unterminated at 16
//!   characters) and joined with `strcpy`/`strcat`: a 16-character
//!   `HierarchyName` runs on into `FrameCount`, still 0 at that point; a
//!   16-character `Name` runs on into `HierarchyName`.
//! - The pose name is read into a `StringClass` buffer of the chunk's length
//!   (zeroed), so a short read keeps zeros after what the file had. Upstream
//!   then asks the asset manager for that animation (`Get_HAnim`) and keeps
//!   the pointer; here the name is kept ([`MorphChannel::pose_name`]). An
//!   unknown name is not an error.
//! - Key reads go into one struct on the stack that is not reset: a read
//!   that fails after a successful one leaves the previous key's bytes
//!   (overwritten by whatever the file still had), and every remaining key
//!   repeats it ([`MorphKeys::repeats`]).
//! - The pivot-channel read copies what the file has of the chunk over the
//!   array (zeros elsewhere); several such chunks each overwrite it.
//! - Channels never read keep a null pose and no keys.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: a header read that fails (its uninitialised fields are
//! used: [`W3dUndefined::UninitialisedRead`]), as does a first key read that
//! fails; names joined into more than the 32-byte `Name`
//! ([`W3dUndefined::StringOverflow`]); a pose name that fills its buffer
//! without a 0 ([`W3dUndefined::StringOverrun`]); more channel chunks than
//! `ChannelCount` (`PoseData[channel]` written past the array:
//! [`W3dUndefined::IndexOutOfRange`]); a pivot-channel chunk (or a pose name
//! chunk of 2^31 bytes or more, which gets no buffer) that copies more bytes
//! than the array holds (the chunk reader's `BufferOverrun`); arrays the
//! 32-bit original cannot allocate, or negative counts
//! ([`W3dUndefined::Allocation`]: `ChannelCount * 20 + 4` bytes of
//! `TimeCodedMorphKeysClass`, `NumNodes * 4`, a pose name's `length + 8`, the
//! key array's growth past 2^31 - 1 bytes); and the chunk reader's own cases
//! (a read or close with no chunk open, reached when the misdirected closes
//! leave the animation).
//!
//! # Representation
//!
//! [`HMorphAnim::channels`] holds the channels read, in order (channel `n`
//! is the `n`-th channel chunk); the others, up to `channel_count`, are
//! empty ([`HMorphAnim::channel`]). Keys are stored as read plus a count of
//! repeats of the last one, so memory grows with the file, not with the
//! chunk lengths.

pub use super::hanim::HTreeLookup;
use super::w3d_file::*;
use super::{W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::chunkio::ChunkLoad;

/// `sizeof(TimeCodedMorphKeysClass)` on Win32 (a `SimpleDynVecClass`:
/// vtable pointer, `Vector`, `VectorMax`, `ActiveCount`; then `CachedIdx`).
pub const WIN32_SIZEOF_MORPH_KEYS: u64 = 20;
/// The count `new[]` stores before an array of a class with a destructor.
pub const WIN32_ARRAY_COOKIE: u64 = 4;
/// `sizeof(StringClass::_HEADER)`.
pub const STRING_HEADER_SIZE: u64 = 8;

/// `TimeCodedMorphKeysClass::MorphKeyStruct`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MorphKey {
    /// `MorphFrame`.
    pub morph_frame: u32,
    /// `PoseFrame`.
    pub pose_frame: u32,
}

/// `TimeCodedMorphKeysClass::Keys`: the keys read, then `repeats` copies of
/// the last one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MorphKeys {
    pub keys: Vec<MorphKey>,
    pub repeats: u32,
}

impl MorphKeys {
    /// `Keys.Count()`.
    pub fn len(&self) -> usize {
        self.keys.len() + self.repeats as usize
    }

    /// Whether there are no keys.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `Keys[i]`.
    pub fn get(&self, i: usize) -> Option<MorphKey> {
        if i < self.keys.len() {
            Some(self.keys[i])
        } else if i < self.len() {
            self.keys.last().copied()
        } else {
            None
        }
    }
}

/// One morph channel: `PoseData[n]` (by name) and `MorphKeyData[n]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MorphChannel {
    /// The name passed to `Get_HAnim` (`None`: the channel was not read, its
    /// pose is null).
    pub pose_name: Option<Vec<u8>>,
    /// The keys.
    pub keys: MorphKeys,
}

/// `class HMorphAnimClass`: what `Load_W3D` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct HMorphAnim {
    /// `Name` (32 bytes, zero-padded).
    pub name: [u8; 2 * W3D_NAME_LEN],
    /// `AnimName` (16 bytes, as `strncpy` leaves them).
    pub anim_name: [u8; W3D_NAME_LEN],
    /// `HierarchyName` (16 bytes, as `strncpy` leaves them).
    pub hierarchy_name: [u8; W3D_NAME_LEN],
    /// `FrameCount`.
    pub frame_count: i32,
    /// `FrameRate`.
    pub frame_rate: f32,
    /// `ChannelCount`.
    pub channel_count: i32,
    /// `NumNodes`.
    pub num_nodes: i32,
    /// The channels read, in order (see the module docs).
    pub channels: Vec<MorphChannel>,
    /// `PivotChannel` (`NumNodes` entries).
    pub pivot_channel: Vec<u32>,
}

impl HMorphAnim {
    /// Channel `n` (`None` at or past `channel_count`).
    pub fn channel(&self, n: usize) -> Option<MorphChannel> {
        if n >= self.channel_count.max(0) as usize {
            return None;
        }
        Some(self.channels.get(n).cloned().unwrap_or_default())
    }
}

/// `strncpy(dst, src, 16)` into a zeroed array.
fn strncpy16(src: &[u8; W3D_NAME_LEN]) -> [u8; W3D_NAME_LEN] {
    let mut d = [0u8; W3D_NAME_LEN];
    for (o, &c) in d.iter_mut().zip(src.iter()) {
        if c == 0 {
            break;
        }
        *o = c;
    }
    d
}

fn c_str16(s: &[u8; W3D_NAME_LEN]) -> &[u8] {
    &s[..s.iter().position(|&c| c == 0).unwrap_or(W3D_NAME_LEN)]
}

/// The bytes left in the file.
fn file_left(cload: &ChunkLoad<'_>) -> usize {
    let f = cload.file();
    f.file_size().wrapping_sub(f.position()).max(0) as usize
}

/// `HMorphAnimClass::Load_W3D(cload)` on a new animation, with `cload`
/// inside the morph chunk. `Ok(None)` is upstream's `LOAD_ERROR`.
pub fn load_w3d(cload: &mut ChunkLoad<'_>, htrees: &mut impl HTreeLookup) -> W3dResult<Option<HMorphAnim>> {
    // read in the animation header (the open and the id are not checked)
    cload.open_chunk()?;
    let mut hb = [0u8; W3dMorphAnimHeaderStruct::SIZE];
    if cload.read_into(&mut hb)? != W3dMorphAnimHeaderStruct::SIZE as u32 {
        return Err(W3dUndefined::UninitialisedRead.into());
    }
    cload.close_chunk()?;
    let header = W3dMorphAnimHeaderStruct::from_bytes(&hb);

    let anim_name = strncpy16(&header.name);
    let hierarchy_name = strncpy16(&header.hierarchy_name);
    // strcpy(Name,HierarchyName); strcat(Name,"."); strcat(Name,AnimName):
    // HierarchyName runs on into FrameCount (0), AnimName into HierarchyName
    let h = c_str16(&hierarchy_name);
    let mut a = c_str16(&anim_name).to_vec();
    if a.len() == W3D_NAME_LEN {
        a.extend_from_slice(h);
    }
    let len = h.len() + 1 + a.len();
    if len + 1 > 2 * W3D_NAME_LEN {
        return Err(W3dUndefined::StringOverflow.into());
    }
    let mut name = [0u8; 2 * W3D_NAME_LEN];
    name[..h.len()].copy_from_slice(h);
    name[h.len()] = b'.';
    name[h.len() + 1..len].copy_from_slice(&a);

    let Some(num_nodes) = htrees.num_pivots(&hierarchy_name) else {
        return Ok(None);
    };
    let channel_count = header.channel_count as i32;
    // PoseData = new HAnimClass*[n]; MorphKeyData = new TimeCodedMorphKeysClass[n];
    // PivotChannel = new uint32[NumNodes]
    if channel_count < 0 || channel_count as u64 * WIN32_SIZEOF_MORPH_KEYS + WIN32_ARRAY_COOKIE > WIN32_MAX_ALLOCATION {
        return Err(W3dUndefined::Allocation.into());
    }
    if num_nodes < 0 || num_nodes as u64 * 4 > WIN32_MAX_ALLOCATION {
        return Err(W3dUndefined::Allocation.into());
    }
    let mut anim = HMorphAnim {
        name,
        anim_name,
        hierarchy_name,
        frame_count: header.frame_count as i32,
        frame_rate: header.frame_rate,
        channel_count,
        num_nodes,
        channels: Vec::new(),
        pivot_channel: vec![0; num_nodes as usize],
    };

    // read in the rest of the chunks
    while cload.open_chunk()? {
        match cload.cur_chunk_id()? {
            W3D_CHUNK_MORPHANIM_CHANNEL => {
                let channel = anim.channels.len();
                read_channel(cload, &mut anim, channel)?;
            }
            W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA => {
                // cload.Read(PivotChannel, Cur_Chunk_Length()), unchecked
                let len = cload.cur_chunk_length()?;
                let dst = anim.pivot_channel.len() * 4;
                let mut tmp = vec![0u8; dst.min(file_left(cload))];
                let before = cload.file().position();
                cload.read(&mut tmp, len)?;
                let copied = cload.file().position().wrapping_sub(before).max(0) as usize;
                for (i, &b) in tmp[..copied].iter().enumerate() {
                    let w = &mut anim.pivot_channel[i / 4];
                    let mut bytes = w.to_le_bytes();
                    bytes[i % 4] = b;
                    *w = u32::from_le_bytes(bytes);
                }
            }
            _ => {}
        }
        cload.close_chunk()?;
    }
    Ok(Some(anim))
}

/// `HMorphAnimClass::read_channel(cload, channel)`.
fn read_channel(cload: &mut ChunkLoad<'_>, anim: &mut HMorphAnim, channel: usize) -> W3dResult<()> {
    // StringClass anim_name; (empty: no buffer)
    cload.open_chunk()?;
    // cload.Read(anim_name.Get_Buffer(Cur_Chunk_Length()), Cur_Chunk_Length())
    let len = cload.cur_chunk_length()?;
    let cap = if len as i32 > 0 {
        if u64::from(len) + STRING_HEADER_SIZE > WIN32_MAX_ALLOCATION {
            return Err(W3dUndefined::Allocation.into());
        }
        len as usize
    } else {
        0 // Uninitialised_Grow does nothing: the buffer is the empty string
    };
    let mut buf = vec![0u8; cap.min(file_left(cload))];
    cload.read(&mut buf, len)?;
    let pose_name = match buf.iter().position(|&c| c == 0) {
        Some(n) => buf[..n].to_vec(),
        None if buf.len() < cap || cap == 0 => buf,
        None => return Err(W3dUndefined::StringOverrun.into()),
    };
    cload.close_chunk()?;

    // PoseData[channel] = Get_HAnim(anim_name)
    if channel >= anim.channel_count.max(0) as usize {
        return Err(W3dUndefined::IndexOutOfRange.into());
    }

    cload.open_chunk()?;
    let keys = load_keys(cload)?;
    cload.close_chunk()?;
    anim.channels.push(MorphChannel { pose_name: Some(pose_name), keys });
    Ok(())
}

/// `TimeCodedMorphKeysClass::Load_W3D(cload)` on an empty key set.
fn load_keys(cload: &mut ChunkLoad<'_>) -> W3dResult<MorphKeys> {
    let key_count = cload.cur_chunk_length()? / W3dMorphAnimKeyStruct::SIZE as u32;
    // Keys.Add grows the array (Grow: by a quarter, at least 4) from empty
    let mut cap: u64 = 0;
    while cap < u64::from(key_count) {
        cap = (cap + cap / 4).max(cap + 4);
        if cap * W3dMorphAnimKeyStruct::SIZE as u64 > WIN32_MAX_ALLOCATION {
            return Err(W3dUndefined::Allocation.into());
        }
    }
    let mut out = MorphKeys::default();
    // W3dMorphAnimKeyStruct w3dkey; (one struct for every read)
    let mut w3dkey: Option<[u8; 8]> = None;
    let mut i = 0u32;
    while i < key_count {
        let mut b = w3dkey.unwrap_or([0; 8]);
        if cload.read_into(&mut b)? != 8 {
            if w3dkey.is_none() {
                return Err(W3dUndefined::UninitialisedRead.into());
            }
            // the file is used up (or the chunk bound passed): every later
            // read copies nothing
            let k = W3dMorphAnimKeyStruct::from_bytes(&b);
            out.keys.push(MorphKey { morph_frame: k.morph_frame, pose_frame: k.pose_frame });
            out.repeats = key_count - i - 1;
            break;
        }
        w3dkey = Some(b);
        let k = W3dMorphAnimKeyStruct::from_bytes(&b);
        out.keys.push(MorphKey { morph_frame: k.morph_frame, pose_frame: k.pose_frame });
        i += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::w3d::prim_anim::tests::chunk;
    use crate::formats::w3d::W3dError;
    use crate::ww::chunkio::{ChunkIoUndefined, GameFile};

    fn header(name: &[u8], hier: &[u8], frames: u32, channels: u32) -> Vec<u8> {
        let mut h = W3dMorphAnimHeaderStruct::from_bytes(&[0; 48]);
        h.version = W3D_CURRENT_MORPH_HANIM_VERSION;
        h.name[..name.len()].copy_from_slice(name);
        h.hierarchy_name[..hier.len()].copy_from_slice(hier);
        h.frame_count = frames;
        h.frame_rate = 30.0;
        h.channel_count = channels;
        chunk(W3D_CHUNK_MORPHANIM_HEADER, &h.to_bytes())
    }

    fn keys(k: &[(u32, u32)]) -> Vec<u8> {
        let b: Vec<u8> = k.iter().flat_map(|(m, p)| m.to_le_bytes().into_iter().chain(p.to_le_bytes())).collect();
        chunk(W3D_CHUNK_MORPHANIM_KEYDATA, &b)
    }

    fn channel(pose: &[u8], k: &[(u32, u32)]) -> Vec<u8> {
        let mut b = chunk(W3D_CHUNK_MORPHANIM_POSENAME, pose);
        b.extend(keys(k));
        chunk(W3D_CHUNK_MORPHANIM_CHANNEL, &b)
    }

    fn trees(n: &[u8; W3D_NAME_LEN]) -> Option<i32> {
        if c_str16(n).starts_with(b"SKL") {
            Some(3)
        } else {
            None
        }
    }

    fn load_data(data: &[u8]) -> (W3dResult<Option<HMorphAnim>>, i32) {
        let mut c = ChunkLoad::new(GameFile::new(data).unwrap());
        assert!(c.open_chunk().unwrap());
        let r = load_w3d(&mut c, &mut trees);
        (r, c.cur_chunk_depth())
    }

    fn load(body: &[u8]) -> (W3dResult<Option<HMorphAnim>>, i32) {
        load_data(&chunk(W3D_CHUNK_MORPH_ANIMATION, body))
    }

    /// A morph animation with two channels and pivot-channel data.
    ///
    /// Predicted before the run: name `SKL.FACE`, 60 frames at 30, 2
    /// channels, 3 nodes; channel 0 pose `SKL.PHONEMES` with keys (0,0) and
    /// (10,4); channel 1 pose `SKL.EXPR` with key (0,1); pivot channels 0, 1,
    /// 1; depth 1.
    #[test]
    fn loads_header_channels_and_pivots() {
        let mut body = header(b"FACE", b"SKL", 60, 2);
        body.extend(channel(b"SKL.PHONEMES\0", &[(0, 0), (10, 4)]));
        body.extend(channel(b"SKL.EXPR\0", &[(0, 1)]));
        let piv: Vec<u8> = [0u32, 1, 1].iter().flat_map(|v| v.to_le_bytes()).collect();
        body.extend(chunk(W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA, &piv));
        let (r, depth) = load(&body);
        let a = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(&a.name[..9], b"SKL.FACE\0");
        assert_eq!((a.frame_count, a.frame_rate, a.channel_count, a.num_nodes), (60, 30.0, 2, 3));
        assert_eq!(a.channels[0].pose_name.as_deref(), Some(&b"SKL.PHONEMES"[..]));
        assert_eq!(
            a.channels[0].keys.keys,
            vec![MorphKey { morph_frame: 0, pose_frame: 0 }, MorphKey { morph_frame: 10, pose_frame: 4 }]
        );
        assert_eq!(a.channel(1).unwrap().keys.get(0), Some(MorphKey { morph_frame: 0, pose_frame: 1 }));
        assert_eq!(a.pivot_channel, vec![0, 1, 1]);
        assert_eq!(a.channel(2), None);
    }

    /// Quirks: an unknown hierarchy; 16-character names; an unread channel;
    /// a short pivot-channel chunk; a pose name without a terminator in a
    /// file that is cut short.
    ///
    /// Predicted before the run: the unknown hierarchy is a load error
    /// (`Ok(None)`); `ABCDEFGHIJKLMNOP` with hierarchy `SKL` gives
    /// `SKL.ABCDEFGHIJKLMNOPSKL`; with 3 channels and one channel chunk,
    /// channels 1 and 2 are empty (no pose, no keys); a 4-byte pivot chunk
    /// sets pivot 0 only (7, then 0, 0).
    #[test]
    fn names_unread_channels_and_short_pivots() {
        assert_eq!(load(&header(b"F", b"NOPE", 1, 0)).0, Ok(None));
        let mut body = header(b"ABCDEFGHIJKLMNOP", b"SKL", 1, 3);
        body.extend(channel(b"P\0", &[(0, 0)]));
        body.extend(chunk(W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA, &7u32.to_le_bytes()));
        let a = load(&body).0.unwrap().unwrap();
        assert_eq!(&a.name[..24], b"SKL.ABCDEFGHIJKLMNOPSKL\0");
        assert_eq!(&a.anim_name, b"ABCDEFGHIJKLMNOP");
        assert_eq!(a.channel(1), Some(MorphChannel::default()));
        assert_eq!(a.channel(2).unwrap().pose_name, None);
        assert_eq!(a.pivot_channel, vec![7, 0, 0]);
    }

    /// Quirk: the stale key struct. A key chunk claiming 5 keys in a file
    /// that ends 2 bytes into the third.
    ///
    /// Predicted before the run: keys (1,2), (3,4), then (5 with its low two
    /// bytes from the file: 0x0000_0005 overwrites only bytes 0..2 of the
    /// stale (3,4), giving morph frame 5, pose frame 4), then two repeats of
    /// it: 5 keys in all.
    #[test]
    fn failed_key_reads_repeat_the_stale_key() {
        let mut kb = keys(&[(1, 2), (3, 4), (5, 6)]);
        kb[4] = 40;
        kb.truncate(8 + 16 + 2);
        let mut body = header(b"F", b"SKL", 1, 1);
        let mut ch = chunk(W3D_CHUNK_MORPHANIM_POSENAME, b"P\0");
        ch.extend(kb);
        let mut chc = W3D_CHUNK_MORPHANIM_CHANNEL.to_le_bytes().to_vec();
        chc.extend_from_slice(&(ch.len() as u32 + 22).to_le_bytes());
        chc.extend(ch);
        body.extend(chc);
        let mut data = W3D_CHUNK_MORPH_ANIMATION.to_le_bytes().to_vec();
        data.extend_from_slice(&(body.len() as u32 + 22).to_le_bytes());
        data.extend(body);
        let a = load_data(&data).0.unwrap().unwrap();
        let k = &a.channels[0].keys;
        assert_eq!(k.len(), 5);
        assert_eq!(k.repeats, 2);
        assert_eq!(k.get(2), Some(MorphKey { morph_frame: 5, pose_frame: 4 }));
        assert_eq!(k.get(4), Some(MorphKey { morph_frame: 5, pose_frame: 4 }));
    }

    /// Quirk: a channel chunk with no sub-chunks. Its pose-name open fails,
    /// so the name is read from the (empty) channel chunk, the close closes
    /// the channel chunk, and the key open takes the next chunk of the
    /// animation (here the pivot data) as the keys; the loop's close then
    /// closes the animation chunk itself, and the loop goes on at file level.
    ///
    /// Predicted before the run: the pose name is empty; the pivot chunk's 12
    /// bytes are read as one key (0, 1) (and 1 byte short of a second: 12/8
    /// = 1 key); the pivot channels stay 0; the next top-level chunk (an
    /// unknown one) is opened and closed by the loop; depth 0 at the end.
    #[test]
    fn empty_channel_chunk_misdirects_the_closes() {
        let mut body = header(b"F", b"SKL", 1, 1);
        body.extend(chunk(W3D_CHUNK_MORPHANIM_CHANNEL, &[]));
        let piv: Vec<u8> = [0u32, 1, 1].iter().flat_map(|v| v.to_le_bytes()).collect();
        body.extend(chunk(W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA, &piv));
        let mut data = chunk(W3D_CHUNK_MORPH_ANIMATION, &body);
        data.extend(chunk(0x1234, &[9; 4]));
        let (r, depth) = load_data(&data);
        let a = r.unwrap().unwrap();
        assert_eq!(depth, 0);
        assert_eq!(a.channels[0].pose_name.as_deref(), Some(&b""[..]));
        assert_eq!(a.channels[0].keys.keys, vec![MorphKey { morph_frame: 0, pose_frame: 1 }]);
        assert_eq!(a.pivot_channel, vec![0, 0, 0]);
    }

    /// Undefined inputs.
    ///
    /// Predicted before the run: a short header is `UninitialisedRead`;
    /// names joined into 32 bytes or more (`ABCDEFGHIJKLMNOP` with a
    /// 15-character hierarchy: 15 + 1 + 16 + 15) `StringOverflow`; a second
    /// channel chunk with `ChannelCount` 1 `IndexOutOfRange`; a pivot chunk
    /// of 16 bytes for 3 nodes the chunk reader's `BufferOverrun`; a pose
    /// name filling its chunk without a 0 `StringOverrun`; a first key read
    /// cut by the end of the file `UninitialisedRead`; `ChannelCount`
    /// 0x80000000 `Allocation`.
    #[test]
    fn undefined_inputs() {
        let undefined = |u: W3dUndefined| Err(W3dError::Undefined(u));
        assert_eq!(load(&chunk(W3D_CHUNK_MORPHANIM_HEADER, &[0; 47])).0, undefined(W3dUndefined::UninitialisedRead));
        assert_eq!(load(&header(b"ABCDEFGHIJKLMNOP", b"SKLSKLSKLSKLSKL", 1, 0)).0, undefined(W3dUndefined::StringOverflow));
        let mut body = header(b"F", b"SKL", 1, 1);
        body.extend(channel(b"P\0", &[]));
        body.extend(channel(b"Q\0", &[]));
        assert_eq!(load(&body).0, undefined(W3dUndefined::IndexOutOfRange));
        let mut body = header(b"F", b"SKL", 1, 0);
        body.extend(chunk(W3D_CHUNK_MORPHANIM_PIVOTCHANNELDATA, &[0; 16]));
        assert_eq!(load(&body).0, undefined(W3dUndefined::ChunkIo(ChunkIoUndefined::BufferOverrun)));
        let mut body = header(b"F", b"SKL", 1, 1);
        body.extend(channel(b"POSE", &[]));
        assert_eq!(load(&body).0, undefined(W3dUndefined::StringOverrun));
        let mut body = header(b"F", b"SKL", 1, 1);
        let mut ch = chunk(W3D_CHUNK_MORPHANIM_POSENAME, b"P\0");
        let mut kb = keys(&[(1, 2)]);
        kb.truncate(8 + 5);
        ch.extend(kb);
        body.extend(chunk(W3D_CHUNK_MORPHANIM_CHANNEL, &ch));
        // the key chunk claims 8 bytes, the file ends 5 bytes into them
        assert_eq!(load(&body).0, undefined(W3dUndefined::UninitialisedRead));
        assert_eq!(load(&header(b"F", b"SKL", 1, 0x8000_0000)).0, undefined(W3dUndefined::Allocation));
    }
}
