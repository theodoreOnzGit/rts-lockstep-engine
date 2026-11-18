//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/part_buf.cpp
//!
//! `ParticleBufferClass`' key-frame getters and line-renderer queries that no earlier pass translated. The six
//! `Get_*_Key_Frames` functions have one shape; [`key_frames`] is it, and each getter below passes its own arrays and
//! scale. They hand back the key times (seconds) and values after the first, plus a synthesised LAST key at `MaxAge`
//! when the final delta is nonzero (the particle keeps changing until it dies).

/// `ParticlePropertyStruct<T>`: `Start`, `Rand`, and the keys after the first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParticleProperty<T> {
    pub start: T,
    pub rand: T,
    pub num_key_frames: i32,
    pub key_times: Vec<f32>,
    pub values: Vec<T>,
}

/// The shared body of the `Get_*_Key_Frames` functions.
///
/// - `times[i]` is `XKeyFrameTimes[i]` (milliseconds), `values[i]` is `XKeyFrameValues[i]`.
/// - `last_delta_is_nonzero` is the test on `XKeyFrameDeltas[N - 1]` (false when the delta array is NULL).
/// - `last_value` is what the synthesised key at `MaxAge` takes: `start + delta * (MaxAge - times[N - 1])` for the type.
/// - `n` is `NumXKeyFrames`.
fn key_frames<T: Clone>(
    n: usize,
    times: &[u32],
    values: &[T],
    max_age: f32,
    last_delta_is_nonzero: bool,
    last_value: impl FnOnce(f32) -> T,
    start: T,
    rand: T,
    map_value: impl Fn(&T) -> T,
) -> ParticleProperty<T> {
    let mut real_keyframe_count: i32 = if n > 0 { n as i32 - 1 } else { 0 };
    let mut create_last_keyframe = false;
    if last_delta_is_nonzero {
        real_keyframe_count += 1;
        create_last_keyframe = true;
    }
    let mut out = ParticleProperty { start, rand, num_key_frames: real_keyframe_count, key_times: Vec::new(), values: Vec::new() };
    if real_keyframe_count > 0 {
        let mut index = 1usize;
        while index < n {
            out.key_times.push(times[index] as f32 / 1000.0);
            out.values.push(map_value(&values[index]));
            index += 1;
        }
        if create_last_keyframe {
            out.key_times.push(max_age / 1000.0);
            let time_delta = max_age - times[index - 1] as f32;
            out.values.push(last_value(time_delta));
        }
    }
    out
}

/// `ParticleBufferClass::Get_Color_Key_Frames` (part_buf.cpp:3035). `deltas` is `ColorKeyFrameDeltas` (`None` is NULL).
pub fn get_color_key_frames(times: &[u32], values: &[[f32; 3]], deltas: Option<&[[f32; 3]]>, color_random: [f32; 3], max_age: f32) -> ParticleProperty<[f32; 3]> {
    let n = times.len();
    let nonzero = deltas.map_or(false, |d| n > 0 && (d[n - 1][0] != 0.0 || d[n - 1][1] != 0.0 || d[n - 1][2] != 0.0));
    let start = values[0];
    key_frames(
        n,
        times,
        values,
        max_age,
        nonzero,
        |td| {
            let s = values[n - 1];
            let d = deltas.unwrap()[n - 1];
            [s[0] + d[0] * td, s[1] + d[1] * td, s[2] + d[2] * td]
        },
        start,
        color_random,
        |v| *v,
    )
}

fn float_key_frames(times: &[u32], values: &[f32], deltas: Option<&[f32]>, random: f32, max_age: f32) -> ParticleProperty<f32> {
    let n = times.len();
    let nonzero = deltas.map_or(false, |d| n > 0 && d[n - 1] != 0.0);
    key_frames(n, times, values, max_age, nonzero, |td| values[n - 1] + deltas.unwrap()[n - 1] * td, values[0], random, |v| *v)
}

/// `ParticleBufferClass::Get_Opacity_Key_Frames` (part_buf.cpp:3093).
pub fn get_opacity_key_frames(times: &[u32], values: &[f32], deltas: Option<&[f32]>, opacity_random: f32, max_age: f32) -> ParticleProperty<f32> {
    float_key_frames(times, values, deltas, opacity_random, max_age)
}

/// `ParticleBufferClass::Get_Size_Key_Frames` (part_buf.cpp:3150).
pub fn get_size_key_frames(times: &[u32], values: &[f32], deltas: Option<&[f32]>, size_random: f32, max_age: f32) -> ParticleProperty<f32> {
    float_key_frames(times, values, deltas, size_random, max_age)
}

/// `ParticleBufferClass::Get_Rotation_Key_Frames` (part_buf.cpp:3207): rotations are stored per millisecond and given per
/// second (`* 1000`); the stored delta is HALF the rotation delta (`HalfRotationKeyFrameDeltas`), so the last key uses
/// `2 * delta`. `start` is `RotationKeyFrameValues[0] * 1000`, or 0 without values.
pub fn get_rotation_key_frames(times: &[u32], values: &[f32], half_deltas: Option<&[f32]>, rotation_random: f32, max_age: f32) -> ParticleProperty<f32> {
    let n = times.len();
    let nonzero = half_deltas.map_or(false, |d| n > 0 && d[n - 1] != 0.0);
    let start = if values.is_empty() { 0.0 } else { values[0] * 1000.0 };
    key_frames(
        n,
        times,
        values,
        max_age,
        nonzero,
        |td| {
            let start_rotation = values[n - 1];
            let delta = 2.0 * half_deltas.unwrap()[n - 1];
            (start_rotation + delta * td) * 1000.0
        },
        start,
        rotation_random * 1000.0,
        |v| *v * 1000.0,
    )
}

/// `ParticleBufferClass::Get_Frame_Key_Frames` (part_buf.cpp:3269).
pub fn get_frame_key_frames(times: &[u32], values: &[f32], deltas: Option<&[f32]>, frame_random: f32, max_age: f32) -> ParticleProperty<f32> {
    float_key_frames(times, values, deltas, frame_random, max_age)
}

/// `ParticleBufferClass::Get_Blur_Time_Key_Frames` (part_buf.cpp:3325).
pub fn get_blur_time_key_frames(times: &[u32], values: &[f32], deltas: Option<&[f32]>, blur_time_random: f32, max_age: f32) -> ParticleProperty<f32> {
    float_key_frames(times, values, deltas, blur_time_random, max_age)
}

/// `SegLineRendererClass::UNIFORM_WIDTH_TEXTURE_MAP` (segline.h).
pub const UNIFORM_WIDTH_TEXTURE_MAP: i32 = 0;

/// `ParticleBufferClass::Get_Line_Texture_Mapping_Mode` (part_buf.cpp:3399): the line renderer's mode, or the uniform-width
/// default without one.
pub fn get_line_texture_mapping_mode(line_renderer_mode: Option<i32>) -> i32 {
    line_renderer_mode.unwrap_or(UNIFORM_WIDTH_TEXTURE_MAP)
}

/// `ParticleBufferClass::Get_Subdivision_Level` (part_buf.cpp:3439): the line renderer's current level, else 0.
pub fn get_subdivision_level(line_renderer_level: Option<i32>) -> i32 {
    line_renderer_level.unwrap_or(0)
}
