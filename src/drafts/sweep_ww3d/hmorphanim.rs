//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hmorphanim.cpp
//!
//! The authoring-side members of `HMorphAnimClass`: `Resolve_Pivot_Channels`, `Free_Morph`, `Create_New_Morph`,
//! `Insert_Morph_Key`, `Release_Keys`. Nothing in the game calls them (grep of the whole upstream tree: only
//! hmorphanim.cpp/.h; they were W3dView's, which is not in the release), so they are drafted for completeness over the
//! verified [`HMorphAnim`] (formats::w3d::hmorphanim); the pose animations are the [`MorphPose`] trait.
//!
//! Excluded: `HMorphAnimClass::write_channel` (hmorphanim.cpp:659): reached only through `Save_W3D`, which only
//! W3dView / the exporters call: editor-only.

use crate::formats::w3d::hmorphanim::{HMorphAnim, MorphChannel, MorphKey};

/// `HMorphAnimClass::OK` / `LOAD_ERROR`.
pub const OK: i32 = 0;
pub const LOAD_ERROR: i32 = 1;

/// What the morph reads of a pose `HAnimClass` (`PoseData[channel]`).
pub trait MorphPose {
    /// `Get_Frame_Rate()`.
    fn get_frame_rate(&self) -> f32;
    /// `Get_Num_Pivots()`.
    fn get_num_pivots(&self) -> i32;
    /// `Is_Node_Motion_Present(pivot)`.
    fn is_node_motion_present(&self, pivot: i32) -> bool;
    /// `Get_Name()`.
    fn get_name(&self) -> Vec<u8>;
}

/// `HMorphAnimClass::Resolve_Pivot_Channels()` (hmorphanim.cpp:458): for each pivot, the last channel whose pose has node
/// motion for it (0 if none): "Find out which animation channel affects this pivot".
pub fn resolve_pivot_channels<P: MorphPose>(morph: &mut HMorphAnim, pose_data: &[&P]) {
    for pivot in 0..morph.num_nodes {
        morph.pivot_channel[pivot as usize] = 0;
        for channel in 0..morph.channel_count {
            if pose_data[channel as usize].is_node_motion_present(pivot) {
                morph.pivot_channel[pivot as usize] = channel as u32;
            }
        }
    }
}

/// `HMorphAnimClass::Free_Morph()` (hmorphanim.cpp:507): `Free()`, which releases `PoseData`, `MorphKeyData` and
/// `PivotChannel` (the counts stay).
pub fn free_morph(morph: &mut HMorphAnim) {
    morph.channels.clear();
    morph.pivot_channel.clear();
}

/// `HMorphAnimClass::Create_New_Morph(channels, anim[])` (hmorphanim.cpp:512): frees the previous contents, sets the
/// channel count, then (with `anim == NULL` -> `LOAD_ERROR`) takes the frame rate and node count of `anim[0]`, keeps
/// every pose, creates an empty key list per channel and resolves the pivot channels. `FrameCount` is 0.
pub fn create_new_morph<P: MorphPose>(morph: &mut HMorphAnim, channels: i32, anim: Option<&[&P]>) -> i32 {
    // clean out the previous instance of this class
    free_morph(morph);

    // set the number of channels
    morph.channel_count = channels;

    // read in the animation header
    let anim = match anim {
        Some(a) => a,
        None => return LOAD_ERROR,
    };

    morph.frame_count = 0;
    morph.frame_rate = anim[0].get_frame_rate();
    morph.num_nodes = anim[0].get_num_pivots();

    // Set up the anim data for all the channels (the pose is held by name)
    morph.channels = (0..channels as usize)
        .map(|i| MorphChannel { pose_name: Some(anim[i].get_name()), keys: Default::default() })
        .collect();

    // Create a timecodekey array for each channel and initialize the pivot channels
    morph.pivot_channel = vec![0; morph.num_nodes as usize];

    // Resolve the pivots so that they correspond to the proper morphing channels
    resolve_pivot_channels(morph, anim);

    OK
}

/// `TimeCodedMorphKeysClass::Add_Key(morph_frame, pose_frame)` (hmorphanim.cpp:91): `Keys.Add(MorphKeyStruct(...))`. The
/// loaded form stores `repeats` copies of the last key implicitly; they are made explicit first so the new key lands
/// after them, as `Keys.Add` appends to the full list.
fn add_key(channel: &mut MorphChannel, morph_frame: u32, pose_frame: u32) {
    if channel.keys.repeats > 0 {
        if let Some(last) = channel.keys.keys.last().copied() {
            for _ in 0..channel.keys.repeats {
                channel.keys.keys.push(last);
            }
        }
        channel.keys.repeats = 0;
    }
    channel.keys.keys.push(MorphKey { morph_frame, pose_frame });
}

/// `HMorphAnimClass::Insert_Morph_Key(channel, morph_frame, pose_frame)` (hmorphanim.cpp:726): adds the key and raises
/// `FrameCount` to `Max(morph_frame, FrameCount)`. (`assert(channel<ChannelCount)` is a debug assertion.)
pub fn insert_morph_key(morph: &mut HMorphAnim, channel: i32, morph_frame: u32, pose_frame: u32) {
    add_key(&mut morph.channels[channel as usize], morph_frame, pose_frame);

    // update the framecount to reflect the newly added key
    morph.frame_count = (morph_frame as i32).max(morph.frame_count);
}

/// `HMorphAnimClass::Release_Keys()` (hmorphanim.cpp:735): `MorphKeyData[i].Free()` for every channel (`Keys.Delete_All();
/// CachedIdx = 0;`), and the frame count back to 0.
pub fn release_keys(morph: &mut HMorphAnim) {
    for i in 0..morph.channel_count as usize {
        if let Some(c) = morph.channels.get_mut(i) {
            c.keys.keys.clear();
            c.keys.repeats = 0;
            c.keys.cached_idx = Default::default();
        }
    }

    // update the framecount as 0
    morph.frame_count = 0;
}
