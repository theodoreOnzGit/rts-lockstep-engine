// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/part_ldr.cpp, part_ldr.h
//   (ParticleEmitterDefClass: constructor, Free_Props, Load_W3D,
//   Initialize_To_Ver2, Convert_To_Ver2, Read_Header, Read_User_Data,
//   Read_Info, Create_Randomizer, Read_InfoV2, Read_Props,
//   Read_Color_Keyframe, Read_Opacity_Keyframe, Read_Size_Keyframe,
//   Read_Line_Properties, Read_Rotation_Keyframes, Read_Frame_Keyframes,
//   Read_Blur_Time_Keyframes, Read_Extra_Info, the Get_* accessors;
//   ParticleEmitterLoaderClass::Load_W3D; ParticleEmitterPrototypeClass's
//   Get_Name and Get_Class_ID; the RGBA_TO_VECTOR3 macro)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/part_emt.h
//   (ParticlePropertyStruct)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/v3_rnd.cpp, v3_rnd.h
//   (the four Vector3Randomizer constructors, their Class_IDs)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, part_ldr by Patrick Smith, v3_rnd by Naty Hoffman)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D particle emitters (`W3D_CHUNK_EMITTER`):
//! `ParticleEmitterDefClass::Load_W3D`, which reads the definition the
//! game's `ParticleEmitterLoaderClass` keeps in a
//! `ParticleEmitterPrototypeClass` (upstream's `Create` builds a
//! `ParticleEmitterClass` from it; that render object is not translated
//! here, see [`ParticleEmitterDef`]). GitHub issue #17.
//!
//! # As upstream reads it
//!
//! `Initialize_To_Ver2` first: the info structures zeroed, then the
//! version-2 defaults (burst size 1, the additive sprite shader, solid-box
//! randomizers of size 0) and no keyframes. Then, each in its own chunk
//! that must come in this order and be of the right type:
//! `W3D_CHUNK_EMITTER_HEADER` ([`W3dEmitterHeaderStruct`]: the version and
//! the name, `_strdup`'d), `W3D_CHUNK_EMITTER_USER_DATA`
//! ([`W3dEmitterUserInfoStruct`], 12 bytes, then `SizeofStringParam` bytes
//! of user string), `W3D_CHUNK_EMITTER_INFO` ([`W3dEmitterInfoStruct`]).
//! For a version above 1.0, `W3D_CHUNK_EMITTER_INFOV2`
//! ([`W3dEmitterInfoStructV2`], whose two volume structs make the
//! randomizers) and `W3D_CHUNK_EMITTER_PROPS` ([`W3dEmitterPropertyStruct`]:
//! the keyframe counts, each one more than the keyframes, and the random
//! ranges; then the colour start and keyframes, the opacity start and
//! keyframes, the size start and keyframes, each an 8-byte struct). For
//! version 1.0 and older, `Convert_To_Ver2` instead: the shader from the
//! texture (alpha-blended if the texture's format has alpha), solid-box
//! randomizers from `PositionRandom / 1000` and `VelocityRandom`, and one
//! colour and one opacity keyframe at `FadeTime` from `StartColor` to
//! `EndColor`, the size fixed at `StartSize`. Then any chunks ("future
//! additions"): line properties, rotation, frame and blur-time keyframes
//! (each a header with the count and random range, the start key, then
//! the keys), extra info; others (the obsolete keyframe chunks included)
//! are skipped. The loop stops at the first of these that fails.
//!
//! # Upstream quirks that are kept
//!
//! - **The loop opens one chunk more after a failure.** `while
//!   (Open_Chunk() && ret_val == OK)` opens the next chunk before it tests
//!   the result, and does not close it. So does a failure before the loop:
//!   a load error always leaves that chunk open too, if there is one.
//! - A header, user-data, info, info-v2 or props chunk of the wrong type
//!   is left open (its `Close_Chunk` is inside the `if`); one too short is
//!   closed.
//! - Colours are bytes over 255 (`RGBA_TO_VECTOR3`); opacity `A / 255`.
//! - The props' keyframe reads are not checked: keys the chunk does not
//!   hold keep the allocator's zeros (as does a start value: 0), and the
//!   load succeeds. Their counts are the file's minus one, unsigned.
//! - If the last colour keyframe is black and the colour has a random
//!   range, it is set to minus the range ("so any randomized color will
//!   end up as black") -- but only from the second keyframe on: with one
//!   keyframe, index 0, the test `last_keyframe > 0` fails.
//! - A rotation, frame or blur-time chunk whose start key is short keeps
//!   the previous start (0, or the previous chunk's); a second such chunk
//!   replaces the keys (the old arrays leak upstream) unless its count is
//!   0, when the old arrays stay with the new count of 0.
//! - A rotation key that cannot be read is set to 0 and ends the load.
//! - `Read_Props` empties every keyframe set first (`Free_Props`),
//!   rotation, frame and blur time included.
//! - The emitter of a version-1 file takes its shader from the asset
//!   manager's texture (`Get_Texture(TextureFilename)`, `Has_Alpha` of its
//!   format): upstream that depends on the renderer's state (a texture not
//!   yet initialised has `WW3D_FORMAT_UNKNOWN`, no alpha). Here the caller
//!   answers the question ([`load_w3d`]'s `texture_has_alpha`). An empty
//!   name asks nothing (`Get_Texture` returns null). The texture name runs
//!   on into the rest of `m_Info` when its 260 bytes have no 0.
//! - `Read_Info` and `Read_InfoV2` zero their struct before reading it;
//!   `m_LineProperties` is zeroed only by the constructor.
//! - The user string's buffer has room for a terminator that is never
//!   written: a string with no 0 in its `SizeofStringParam` bytes is not
//!   terminated ([`ParticleEmitterDef::user_string_c_str`]).
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: a header name with no 0 in the struct (`_strdup` reads
//! past it), a version-1 texture name with no 0 in `m_Info`
//! ([`W3dUndefined::StringOverrun`]); a rotation, frame or blur-time chunk
//! too short for its header (the uninitialised header's count is used), a
//! frame or blur-time key that cannot be read (the uninitialised key is
//! copied into the array) ([`W3dUndefined::UninitialisedRead`]); a
//! `SizeofStringParam` of `0xFFFFFFFF` (`malloc(0)`, then the terminator
//! written into it: [`W3dUndefined::WriteToEmptyArray`]); allocations the
//! 32-bit original cannot make ([`W3dUndefined::Allocation`]): a user
//! string of 2^31 - 1 bytes or more, keyframe arrays over 2^31 - 1 bytes
//! (a count of 0 in the props wraps to `0xFFFFFFFF`).
//!
//! # Representation
//!
//! [`ParticleEmitterDef`] holds what a successful `Load_W3D` leaves (on a
//! load error upstream deletes it). The info structures are kept as the
//! file's structs; the randomizers as [`Vector3Randomizer`] (`None` for a
//! class id `Create_Randomizer` does not know). Keyframe arrays are
//! [`Arr`]s, so memory follows what the file holds, not its counts. The
//! `ShaderClass m_Shader` member, never set, is not kept.

use super::mesh::Arr;
use super::shader::{convert_shader_to_w3d, PRESET_ADDITIVE_SPRITE_SHADER, PRESET_ALPHA_SPRITE_SHADER};
use super::w3d_file::*;
use super::{c_string, read_partial, W3dResult, W3dUndefined, WIN32_MAX_ALLOCATION};
use crate::ww::chunkio::ChunkLoad;

/// `RenderObjClass::CLASSID_PARTICLEEMITTER`.
pub const CLASSID_PARTICLEEMITTER: u32 = 14;
/// `EMITTER_TYPEID_DEFAULT` (w3d_file.h).
pub const EMITTER_TYPEID_DEFAULT: i32 = 0;
/// `Vector3Randomizer::CLASSID_*`.
pub const CLASSID_SOLIDBOX: u32 = 0;
pub const CLASSID_SOLIDSPHERE: u32 = 1;
pub const CLASSID_HOLLOWSPHERE: u32 = 2;
pub const CLASSID_SOLIDCYLINDER: u32 = 3;

/// `MAX(x, 0.0f)` (always.h: `((a) > (b)) ? (a) : (b)`): NaN and -0 give 0.
fn max0(x: f32) -> f32 {
    if x > 0.0 {
        x
    } else {
        0.0
    }
}

/// A `Vector3Randomizer` as `Create_Randomizer` makes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Vector3Randomizer {
    /// `Vector3SolidBoxRandomizer(extents)`: `Extents`.
    SolidBox([f32; 3]),
    /// `Vector3SolidSphereRandomizer(radius)`: `Radius`.
    SolidSphere(f32),
    /// `Vector3HollowSphereRandomizer(radius)`: `Radius`.
    HollowSphere(f32),
    /// `Vector3SolidCylinderRandomizer(extent, radius)`.
    SolidCylinder { extent: f32, radius: f32 },
}

impl Vector3Randomizer {
    /// `ParticleEmitterDefClass::Create_Randomizer(info)`, with the
    /// constructors' clamping to 0.
    pub fn create(info: &W3dVolumeRandomizerStruct) -> Option<Self> {
        Some(match info.class_id {
            CLASSID_SOLIDBOX => Vector3Randomizer::SolidBox([max0(info.value1), max0(info.value2), max0(info.value3)]),
            CLASSID_SOLIDSPHERE => Vector3Randomizer::SolidSphere(max0(info.value1)),
            CLASSID_HOLLOWSPHERE => Vector3Randomizer::HollowSphere(max0(info.value1)),
            CLASSID_SOLIDCYLINDER => Vector3Randomizer::SolidCylinder { extent: max0(info.value1), radius: max0(info.value2) },
            _ => return None,
        })
    }

    /// `Class_ID()`.
    pub fn class_id(&self) -> u32 {
        match self {
            Vector3Randomizer::SolidBox(_) => CLASSID_SOLIDBOX,
            Vector3Randomizer::SolidSphere(_) => CLASSID_SOLIDSPHERE,
            Vector3Randomizer::HollowSphere(_) => CLASSID_HOLLOWSPHERE,
            Vector3Randomizer::SolidCylinder { .. } => CLASSID_SOLIDCYLINDER,
        }
    }
}

/// `template<class T> struct ParticlePropertyStruct`.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleProperty<T: Copy + PartialEq> {
    /// `Start`.
    pub start: T,
    /// `Rand`.
    pub rand: T,
    /// `NumKeyFrames`.
    pub num_key_frames: u32,
    /// `KeyTimes` (`None` for a null pointer).
    pub key_times: Option<Arr<f32>>,
    /// `Values`.
    pub values: Option<Arr<T>>,
}

impl<T: Copy + PartialEq + Default> ParticleProperty<T> {
    fn zeroed() -> Self {
        ParticleProperty { start: T::default(), rand: T::default(), num_key_frames: 0, key_times: None, values: None }
    }

    /// `Free_Props` on this set: count 0, arrays deleted (start and random
    /// range kept).
    fn free(&mut self) {
        self.num_key_frames = 0;
        self.key_times = None;
        self.values = None;
    }

    /// Keyframe `i` (time, value), `i < num_key_frames`.
    pub fn key(&self, i: u32) -> Option<(f32, T)> {
        if i >= self.num_key_frames {
            return None;
        }
        let t = self.key_times.as_ref()?.get(i as usize);
        let v = self.values.as_ref()?.get(i as usize);
        Some((t, v))
    }

    /// `KeyTimes = new float[n]; Values = new T[n]` (zero-filled), with
    /// `elem` = `sizeof(T)`.
    fn allocate(&mut self, n: u32, elem: u64) -> W3dResult<()> {
        if u64::from(n) * 4 > WIN32_MAX_ALLOCATION || u64::from(n) * elem > WIN32_MAX_ALLOCATION {
            return Err(W3dUndefined::Allocation.into());
        }
        self.key_times = Some(Arr::new(n as usize, 0.0));
        self.values = Some(Arr::new(n as usize, T::default()));
        Ok(())
    }

    fn set_key(&mut self, i: u32, time: f32, value: T) {
        if let (Some(t), Some(v)) = (self.key_times.as_mut(), self.values.as_mut()) {
            t.set(i as usize, time);
            v.set(i as usize, value);
        }
    }
}

/// `class ParticleEmitterDefClass`: what `Load_W3D` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleEmitterDef {
    /// `m_pName` (`Get_Name()`).
    pub name: Option<Vec<u8>>,
    /// `m_pUserString`: its `SizeofStringParam` bytes as read (`None` for a
    /// null pointer).
    pub user_string: Option<Vec<u8>>,
    /// `m_iUserType`.
    pub user_type: i32,
    /// `m_Version`.
    pub version: u32,
    /// `m_Info`.
    pub info: W3dEmitterInfoStruct,
    /// `m_InfoV2`.
    pub info_v2: W3dEmitterInfoStructV2,
    /// `m_ExtraInfo`.
    pub extra_info: W3dEmitterExtraInfoStruct,
    /// `m_LineProperties`.
    pub line_properties: W3dEmitterLinePropertiesStruct,
    /// `m_ColorKeyframes`.
    pub color_keyframes: ParticleProperty<[f32; 3]>,
    /// `m_OpacityKeyframes`.
    pub opacity_keyframes: ParticleProperty<f32>,
    /// `m_SizeKeyframes`.
    pub size_keyframes: ParticleProperty<f32>,
    /// `m_RotationKeyframes`.
    pub rotation_keyframes: ParticleProperty<f32>,
    /// `m_InitialOrientationRandom`.
    pub initial_orientation_random: f32,
    /// `m_FrameKeyframes`.
    pub frame_keyframes: ParticleProperty<f32>,
    /// `m_BlurTimeKeyframes`.
    pub blur_time_keyframes: ParticleProperty<f32>,
    /// `m_pCreationVolume`.
    pub creation_volume: Option<Vector3Randomizer>,
    /// `m_pVelocityRandomizer`.
    pub velocity_randomizer: Option<Vector3Randomizer>,
}

/// `RGBA_TO_VECTOR3(rgba)`: each byte as `(float)byte / 255.0F`.
fn rgba_to_vector3(c: &W3dRGBAStruct) -> [f32; 3] {
    [f32::from(c.r) / 255.0, f32::from(c.g) / 255.0, f32::from(c.b) / 255.0]
}

/// One 8-byte keyframe struct read with `chunk_load.Read`: `None` when the
/// read fails. A failed read changes nothing that a later read of the same
/// chunk could see (it either fails the chunk bound, or leaves the file at
/// its end), so every later 8-byte read of the chunk fails too.
fn read_key(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<[u8; 8]>> {
    let mut b = [0u8; 8];
    Ok((cload.read_into(&mut b)? == 8).then_some(b))
}

fn f32_at(b: &[u8; 8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl ParticleEmitterDef {
    /// `ParticleEmitterDefClass()`.
    pub fn new() -> Self {
        ParticleEmitterDef {
            name: None,
            user_string: None,
            user_type: EMITTER_TYPEID_DEFAULT,
            version: 0,
            info: W3dEmitterInfoStruct::from_bytes(&[0; W3dEmitterInfoStruct::SIZE]),
            info_v2: W3dEmitterInfoStructV2::from_bytes(&[0; W3dEmitterInfoStructV2::SIZE]),
            extra_info: W3dEmitterExtraInfoStruct::from_bytes(&[0; W3dEmitterExtraInfoStruct::SIZE]),
            line_properties: W3dEmitterLinePropertiesStruct::from_bytes(&[0; W3dEmitterLinePropertiesStruct::SIZE]),
            color_keyframes: ParticleProperty::zeroed(),
            opacity_keyframes: ParticleProperty::zeroed(),
            size_keyframes: ParticleProperty::zeroed(),
            rotation_keyframes: ParticleProperty::zeroed(),
            initial_orientation_random: 0.0,
            frame_keyframes: ParticleProperty::zeroed(),
            blur_time_keyframes: ParticleProperty::zeroed(),
            creation_volume: None,
            velocity_randomizer: None,
        }
    }

    /// `ParticleEmitterPrototypeClass::Get_Class_ID()`.
    pub fn class_id(&self) -> u32 {
        CLASSID_PARTICLEEMITTER
    }

    /// `Get_User_String()` as a C string: `None` for a null pointer, or for
    /// a string with no 0 in its bytes (upstream's terminator byte is
    /// uninitialised).
    pub fn user_string_c_str(&self) -> Option<&[u8]> {
        let s = self.user_string.as_deref()?;
        s.iter().position(|&c| c == 0).map(|n| &s[..n])
    }

    /// `Get_Texture_Filename()` (the C string in `m_Info`; `None` if it runs
    /// past it).
    pub fn texture_filename(&self) -> Option<Vec<u8>> {
        c_string(&self.info.to_bytes(), 0).ok().map(|s| s.to_vec())
    }

    /// `Get_Render_Mode()` (`W3D_EMITTER_RENDER_MODE_*`).
    pub fn render_mode(&self) -> i32 {
        self.info_v2.render_mode as i32
    }

    /// `Get_Frame_Mode()` (`W3D_EMITTER_FRAME_MODE_*`).
    pub fn frame_mode(&self) -> i32 {
        self.info_v2.frame_mode as i32
    }

    /// `Free_Props()`.
    fn free_props(&mut self) {
        self.color_keyframes.free();
        self.opacity_keyframes.free();
        self.size_keyframes.free();
        self.rotation_keyframes.free();
        self.frame_keyframes.free();
        self.blur_time_keyframes.free();
    }

    /// `Initialize_To_Ver2()`.
    fn initialize_to_ver2(&mut self) {
        self.info = W3dEmitterInfoStruct::from_bytes(&[0; W3dEmitterInfoStruct::SIZE]);
        self.info_v2 = W3dEmitterInfoStructV2::from_bytes(&[0; W3dEmitterInfoStructV2::SIZE]);
        self.extra_info = W3dEmitterExtraInfoStruct::from_bytes(&[0; W3dEmitterExtraInfoStruct::SIZE]);
        // Set the version 2 values using defaults from version 1
        self.info_v2.burst_size = 1;
        self.info_v2.outward_vel = 0.0;
        self.info_v2.vel_inherit = 0.0;
        self.info_v2.shader = convert_shader_to_w3d(PRESET_ADDITIVE_SPRITE_SHADER);
        for v in [&mut self.info_v2.creation_volume, &mut self.info_v2.vel_random] {
            v.class_id = CLASSID_SOLIDBOX;
            v.value1 = 0.0;
            v.value2 = 0.0;
            v.value3 = 0.0;
        }
        self.free_props();
    }

    /// `Convert_To_Ver2()` (called for version 1.0 and older).
    fn convert_to_ver2<F: FnMut(&[u8]) -> bool>(&mut self, texture_has_alpha: &mut F) -> W3dResult<()> {
        if self.version >= 0x0002_0000 {
            return Ok(());
        }
        self.info_v2.burst_size = 1;
        self.info_v2.outward_vel = 0.0;
        self.info_v2.vel_inherit = 0.0;
        // Determine which shader to use...
        let mut shader = PRESET_ADDITIVE_SPRITE_SHADER;
        // Get_Texture(m_Info.TextureFilename): null for an empty name
        let info_bytes = self.info.to_bytes();
        let filename = c_string(&info_bytes, 0)?;
        if !filename.is_empty() && texture_has_alpha(filename) {
            shader = PRESET_ALPHA_SPRITE_SHADER;
        }
        self.info_v2.shader = convert_shader_to_w3d(shader);
        // Convert the randomziers
        let p = self.info.position_random / 1000.0;
        let v = self.info.velocity_random;
        let cv = &mut self.info_v2.creation_volume;
        (cv.class_id, cv.value1, cv.value2, cv.value3) = (CLASSID_SOLIDBOX, p, p, p);
        let vr = &mut self.info_v2.vel_random;
        (vr.class_id, vr.value1, vr.value2, vr.value3) = (CLASSID_SOLIDBOX, v, v, v);
        // Recreate the randomizers
        self.creation_volume = Vector3Randomizer::create(&self.info_v2.creation_volume);
        self.velocity_randomizer = Vector3Randomizer::create(&self.info_v2.vel_random);
        // Convert the colors, opacities, and sizes
        self.free_props();
        let fade = self.info.fade_time;
        let c = &mut self.color_keyframes;
        c.start = rgba_to_vector3(&self.info.start_color);
        c.rand = [0.0; 3];
        c.num_key_frames = 1;
        c.key_times = Some(Arr::new(1, fade));
        c.values = Some(Arr::new(1, rgba_to_vector3(&self.info.end_color)));
        let o = &mut self.opacity_keyframes;
        o.start = f32::from(self.info.start_color.a) / 255.0;
        o.rand = 0.0;
        o.num_key_frames = 1;
        o.key_times = Some(Arr::new(1, fade));
        o.values = Some(Arr::new(1, f32::from(self.info.end_color.a) / 255.0));
        let s = &mut self.size_keyframes;
        s.start = self.info.start_size;
        s.rand = 0.0;
        s.num_key_frames = 0;
        Ok(())
    }

    /// `Open_Chunk() && Cur_Chunk_ID() == id`.
    fn open_expected(cload: &mut ChunkLoad<'_>, id: u32) -> W3dResult<bool> {
        Ok(cload.open_chunk()? && cload.cur_chunk_id()? == id)
    }

    /// `Read_Header()`.
    fn read_header(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        if !Self::open_expected(cload, W3D_CHUNK_EMITTER_HEADER)? {
            return Ok(false);
        }
        let mut hb = [0u8; W3dEmitterHeaderStruct::SIZE];
        let ok = cload.read_into(&mut hb)? == W3dEmitterHeaderStruct::SIZE as u32;
        if ok {
            // Copy the name from the header structure
            self.name = Some(c_string(&hb, 4)?.to_vec());
            self.version = W3dEmitterHeaderStruct::from_bytes(&hb).version;
        }
        cload.close_chunk()?;
        Ok(ok)
    }

    /// `Read_User_Data()`.
    fn read_user_data(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        if !Self::open_expected(cload, W3D_CHUNK_EMITTER_USER_DATA)? {
            return Ok(false);
        }
        let mut ub = [0u8; W3dEmitterUserInfoStruct::SIZE];
        let mut ok = false;
        if cload.read_into(&mut ub)? == W3dEmitterUserInfoStruct::SIZE as u32 {
            ok = true;
            let user_info = W3dEmitterUserInfoStruct::from_bytes(&ub);
            self.user_type = user_info.type_ as i32;
            let n = user_info.sizeof_string_param;
            if n > 0 {
                // m_pUserString = malloc(n + 1); m_pUserString[0] = 0
                if n == u32::MAX {
                    return Err(W3dUndefined::WriteToEmptyArray.into());
                }
                if u64::from(n) + 1 > WIN32_MAX_ALLOCATION {
                    return Err(W3dUndefined::Allocation.into());
                }
                let (read_ok, bytes) = read_partial(cload, n)?;
                self.user_string = Some(bytes);
                if !read_ok {
                    ok = false;
                }
            }
        }
        cload.close_chunk()?;
        Ok(ok)
    }

    /// `Read_Info()`.
    fn read_info(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        if !Self::open_expected(cload, W3D_CHUNK_EMITTER_INFO)? {
            return Ok(false);
        }
        let mut ib = [0u8; W3dEmitterInfoStruct::SIZE];
        let ok = cload.read_into(&mut ib)? == W3dEmitterInfoStruct::SIZE as u32;
        self.info = W3dEmitterInfoStruct::from_bytes(&ib);
        cload.close_chunk()?;
        Ok(ok)
    }

    /// `Read_InfoV2()`.
    fn read_info_v2(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        if !Self::open_expected(cload, W3D_CHUNK_EMITTER_INFOV2)? {
            return Ok(false);
        }
        let mut ib = [0u8; W3dEmitterInfoStructV2::SIZE];
        let ok = cload.read_into(&mut ib)? == W3dEmitterInfoStructV2::SIZE as u32;
        self.info_v2 = W3dEmitterInfoStructV2::from_bytes(&ib);
        if ok {
            // Recreate the randomizers
            self.creation_volume = Vector3Randomizer::create(&self.info_v2.creation_volume);
            self.velocity_randomizer = Vector3Randomizer::create(&self.info_v2.vel_random);
        }
        cload.close_chunk()?;
        Ok(ok)
    }

    /// `Read_Props()`, with `Read_Color_Keyframe`, `Read_Opacity_Keyframe`
    /// and `Read_Size_Keyframe`.
    fn read_props(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        self.free_props();
        if !Self::open_expected(cload, W3D_CHUNK_EMITTER_PROPS)? {
            return Ok(false);
        }
        let mut pb = [0u8; W3dEmitterPropertyStruct::SIZE];
        if cload.read_into(&mut pb)? != W3dEmitterPropertyStruct::SIZE as u32 {
            cload.close_chunk()?;
            return Ok(false);
        }
        let info = W3dEmitterPropertyStruct::from_bytes(&pb);
        self.color_keyframes.num_key_frames = info.color_keyframes.wrapping_sub(1);
        self.opacity_keyframes.num_key_frames = info.opacity_keyframes.wrapping_sub(1);
        self.size_keyframes.num_key_frames = info.size_keyframes.wrapping_sub(1);
        self.color_keyframes.rand = rgba_to_vector3(&info.color_random);
        self.opacity_keyframes.rand = info.opacity_random;
        self.size_keyframes.rand = info.size_random;
        // Allocate the arrays of keyframes
        let (nc, no, ns) =
            (self.color_keyframes.num_key_frames, self.opacity_keyframes.num_key_frames, self.size_keyframes.num_key_frames);
        if nc > 0 {
            self.color_keyframes.allocate(nc, 12)?;
        }
        if no > 0 {
            self.opacity_keyframes.allocate(no, 4)?;
        }
        if ns > 0 {
            self.size_keyframes.allocate(ns, 4)?;
        }
        // Read the color keyframes from the chunk
        let color = |b: &[u8; 8]| rgba_to_vector3(&W3dRGBAStruct { r: b[4], g: b[5], b: b[6], a: b[7] });
        if let Some(b) = read_key(cload)? {
            self.color_keyframes.start = color(&b);
        }
        for i in 0..nc {
            let Some(b) = read_key(cload)? else { break };
            self.color_keyframes.set_key(i, f32_at(&b, 0), color(&b));
        }
        // a black last keyframe with a color randomizer ends below 0
        let last_keyframe = nc.wrapping_sub(1) as i32;
        if last_keyframe > 0 {
            let c = &mut self.color_keyframes;
            let rand = c.rand;
            if let Some(values) = c.values.as_mut() {
                let v = values.get(last_keyframe as usize);
                if v[0] == 0.0 && v[1] == 0.0 && v[2] == 0.0 && (rand[0] > 0.0 || rand[1] > 0.0 || rand[2] > 0.0) {
                    values.set(last_keyframe as usize, [-rand[0], -rand[1], -rand[2]]);
                }
            }
        }
        // Read the opacity keyframes from the chunk
        if let Some(b) = read_key(cload)? {
            self.opacity_keyframes.start = f32_at(&b, 4);
        }
        for i in 0..no {
            let Some(b) = read_key(cload)? else { break };
            self.opacity_keyframes.set_key(i, f32_at(&b, 0), f32_at(&b, 4));
        }
        // Read the size keyframes from the chunk
        if let Some(b) = read_key(cload)? {
            self.size_keyframes.start = f32_at(&b, 4);
        }
        for i in 0..ns {
            let Some(b) = read_key(cload)? else { break };
            self.size_keyframes.set_key(i, f32_at(&b, 0), f32_at(&b, 4));
        }
        cload.close_chunk()?;
        Ok(true)
    }

    /// `Read_Line_Properties()` (inside its chunk).
    fn read_line_properties(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        // read straight into the member (a short file keeps what it had)
        let mut lb = [0u8; W3dEmitterLinePropertiesStruct::SIZE];
        lb.copy_from_slice(&self.line_properties.to_bytes());
        let ok = cload.read_into(&mut lb)? == W3dEmitterLinePropertiesStruct::SIZE as u32;
        self.line_properties = W3dEmitterLinePropertiesStruct::from_bytes(&lb);
        Ok(ok)
    }

    /// `Read_Rotation_Keyframes()` (inside its chunk).
    fn read_rotation_keyframes(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        let mut hb = [0u8; W3dEmitterRotationHeaderStruct::SIZE];
        if cload.read_into(&mut hb)? != W3dEmitterRotationHeaderStruct::SIZE as u32 {
            // the uninitialised header is used below
            return Err(W3dUndefined::UninitialisedRead.into());
        }
        let header = W3dEmitterRotationHeaderStruct::from_bytes(&hb);
        let r = &mut self.rotation_keyframes;
        r.num_key_frames = header.keyframe_count;
        r.rand = header.random;
        self.initial_orientation_random = header.orientation_random;
        // Read in the first key
        if let Some(b) = read_key(cload)? {
            r.start = f32_at(&b, 4);
        }
        // Allocate the rotation keys
        if r.num_key_frames > 0 {
            r.allocate(r.num_key_frames, 4)?;
        }
        // Read in the keys
        for i in 0..header.keyframe_count {
            match read_key(cload)? {
                Some(b) => r.set_key(i, f32_at(&b, 0), f32_at(&b, 4)),
                None => {
                    r.set_key(i, 0.0, 0.0);
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// `Read_Frame_Keyframes()` and `Read_Blur_Time_Keyframes()` (the same
    /// text with a header of `header_size` bytes), inside their chunk.
    fn read_frame_like(cload: &mut ChunkLoad<'_>, header_size: usize, p: &mut ParticleProperty<f32>) -> W3dResult<bool> {
        let mut hb = [0u8; W3dEmitterFrameHeaderStruct::SIZE];
        if cload.read(&mut hb[..header_size], header_size as u32)? != header_size as u32 {
            return Err(W3dUndefined::UninitialisedRead.into());
        }
        // KeyframeCount and Random are the first two members of both headers
        let count = u32::from_le_bytes([hb[0], hb[1], hb[2], hb[3]]);
        let random = f32::from_le_bytes([hb[4], hb[5], hb[6], hb[7]]);
        // Read in the first key
        if let Some(b) = read_key(cload)? {
            p.start = f32_at(&b, 4);
        }
        // Allocate the keys
        p.num_key_frames = count;
        p.rand = random;
        if count > 0 {
            p.allocate(count, 4)?;
        }
        // Read in the keys
        for i in 0..count {
            match read_key(cload)? {
                Some(b) => p.set_key(i, f32_at(&b, 0), f32_at(&b, 4)),
                // the uninitialised key is copied into the arrays
                None => return Err(W3dUndefined::UninitialisedRead.into()),
            }
        }
        Ok(true)
    }

    /// `Read_Extra_Info()` (inside its chunk).
    fn read_extra_info(&mut self, cload: &mut ChunkLoad<'_>) -> W3dResult<bool> {
        let mut eb = [0u8; W3dEmitterExtraInfoStruct::SIZE];
        let ok = cload.read_into(&mut eb)? == W3dEmitterExtraInfoStruct::SIZE as u32;
        self.extra_info = W3dEmitterExtraInfoStruct::from_bytes(&eb);
        Ok(ok)
    }
}

impl Default for ParticleEmitterDef {
    fn default() -> Self {
        Self::new()
    }
}

/// `ParticleEmitterLoaderClass::Load_W3D(cload)`: a new definition's
/// `Load_W3D`, with `cload` inside the emitter chunk. `Ok(None)` is a load
/// error (upstream deletes the definition and returns no prototype).
///
/// `texture_has_alpha(name)` answers `Convert_To_Ver2`'s question for a
/// version-1 emitter: whether `WW3DAssetManager::Get_Texture(name)`'s
/// format has alpha (it is asked only for a non-empty name).
pub fn load_w3d<F: FnMut(&[u8]) -> bool>(cload: &mut ChunkLoad<'_>, mut texture_has_alpha: F) -> W3dResult<Option<ParticleEmitterDef>> {
    let mut d = ParticleEmitterDef::new();
    d.initialize_to_ver2();
    // Attempt to read the different sections of the emitter definition
    let mut ok = false;
    if d.read_header(cload)? && d.read_user_data(cload)? && d.read_info(cload)? {
        if d.version > 0x0001_0000 {
            // Read the version 2.0 structures from the chunk
            if d.read_info_v2(cload)? && d.read_props(cload)? {
                ok = true;
            }
        } else {
            // Make sure the data fits version 2
            d.convert_to_ver2(&mut texture_has_alpha)?;
            ok = true;
        }
    }
    // Handle all future additions in the typical chunk fashion
    while cload.open_chunk()? && ok {
        match cload.cur_chunk_id()? {
            W3D_CHUNK_EMITTER_LINE_PROPERTIES => ok = d.read_line_properties(cload)?,
            W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES => ok = d.read_rotation_keyframes(cload)?,
            W3D_CHUNK_EMITTER_FRAME_KEYFRAMES => {
                ok = ParticleEmitterDef::read_frame_like(cload, W3dEmitterFrameHeaderStruct::SIZE, &mut d.frame_keyframes)?
            }
            W3D_CHUNK_EMITTER_BLUR_TIME_KEYFRAMES => {
                ok = ParticleEmitterDef::read_frame_like(cload, W3dEmitterBlurTimeHeaderStruct::SIZE, &mut d.blur_time_keyframes)?
            }
            W3D_CHUNK_EMITTER_EXTRA_INFO => ok = d.read_extra_info(cload)?,
            _ => {} // "Unhandled Chunk!"
        }
        cload.close_chunk()?;
    }
    Ok(ok.then_some(d))
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

    pub(crate) fn header(version: u32, name: &[u8]) -> Vec<u8> {
        let mut h = W3dEmitterHeaderStruct::from_bytes(&[0; 20]);
        h.version = version;
        h.name[..name.len()].copy_from_slice(name);
        chunk(W3D_CHUNK_EMITTER_HEADER, &h.to_bytes())
    }

    pub(crate) fn user(type_: u32, n: u32, s: &[u8]) -> Vec<u8> {
        let mut b = type_.to_le_bytes().to_vec();
        b.extend(n.to_le_bytes());
        b.extend([0u8; 4]);
        b.extend_from_slice(s);
        chunk(W3D_CHUNK_EMITTER_USER_DATA, &b)
    }

    pub(crate) fn info(texture: &[u8], f: impl FnOnce(&mut W3dEmitterInfoStruct)) -> Vec<u8> {
        let mut i = W3dEmitterInfoStruct::from_bytes(&[0; 332]);
        i.texture_filename[..texture.len()].copy_from_slice(texture);
        f(&mut i);
        chunk(W3D_CHUNK_EMITTER_INFO, &i.to_bytes())
    }

    fn key(t: f32, v: [u8; 4]) -> Vec<u8> {
        let mut b = t.to_le_bytes().to_vec();
        b.extend(v);
        b
    }

    fn fkey(t: f32, v: f32) -> Vec<u8> {
        key(t, v.to_le_bytes())
    }

    /// A version-1 emitter `name` with the texture `texture`.
    pub(crate) fn emitter_v1(name: &[u8], texture: &[u8]) -> Vec<u8> {
        let mut body = header(0x0001_0000, name);
        body.extend(user(0, 0, b""));
        body.extend(info(texture, |i| {
            i.position_random = 500.0;
            i.velocity_random = 2.0;
            i.fade_time = 0.75;
            i.start_color = W3dRGBAStruct { r: 255, g: 128, b: 0, a: 255 };
            i.end_color = W3dRGBAStruct { r: 0, g: 0, b: 0, a: 51 };
            i.start_size = 1.5;
        }));
        chunk(W3D_CHUNK_EMITTER, &body)
    }

    fn props(counts: [u32; 3], rand: [u8; 4], keys: &[Vec<u8>]) -> Vec<u8> {
        let mut p = W3dEmitterPropertyStruct::from_bytes(&[0; 40]);
        (p.color_keyframes, p.opacity_keyframes, p.size_keyframes) = (counts[0], counts[1], counts[2]);
        p.color_random = W3dRGBAStruct { r: rand[0], g: rand[1], b: rand[2], a: rand[3] };
        p.opacity_random = 0.5;
        p.size_random = 0.25;
        let mut b = p.to_bytes();
        for k in keys {
            b.extend(k);
        }
        chunk(W3D_CHUNK_EMITTER_PROPS, &b)
    }

    fn info_v2(f: impl FnOnce(&mut W3dEmitterInfoStructV2)) -> Vec<u8> {
        let mut i = W3dEmitterInfoStructV2::from_bytes(&[0; 124]);
        f(&mut i);
        chunk(W3D_CHUNK_EMITTER_INFOV2, &i.to_bytes())
    }

    fn rot(count: u32, keys: &[Vec<u8>]) -> Vec<u8> {
        let mut b = count.to_le_bytes().to_vec();
        b.extend(0.1f32.to_le_bytes());
        b.extend(0.3f32.to_le_bytes());
        b.extend([0u8; 4]);
        for k in keys {
            b.extend(k);
        }
        chunk(W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES, &b)
    }

    /// The start of a version-2 emitter that loads: header, user data,
    /// info, info v2, props with one keyframe each.
    fn v2_base(name: &[u8]) -> Vec<u8> {
        let mut body = header(0x0002_0000, name);
        body.extend(user(0, 0, b""));
        body.extend(info(b"a.tga", |_| {}));
        body.extend(info_v2(|_| {}));
        let k = [key(0.0, [0; 4]), key(1.0, [0; 4])];
        body.extend(props([2, 2, 2], [0; 4], &[k[0].clone(), k[1].clone(), k[0].clone(), k[1].clone(), k[0].clone(), k[1].clone()]));
        body
    }

    fn load_with(body: &[u8], alpha: &mut dyn FnMut(&[u8]) -> bool) -> (W3dResult<Option<ParticleEmitterDef>>, i32) {
        let data = chunk(W3D_CHUNK_EMITTER, body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let r = load_w3d(&mut c, |n: &[u8]| alpha(n));
        (r, c.cur_chunk_depth())
    }

    fn load(body: &[u8]) -> (W3dResult<Option<ParticleEmitterDef>>, i32) {
        load_with(body, &mut |_| panic!("texture asked"))
    }

    fn keys<T: Copy + PartialEq + Default>(p: &ParticleProperty<T>) -> Vec<(f32, T)> {
        (0..p.num_key_frames).map(|i| p.key(i).unwrap()).collect()
    }

    /// A version-2 emitter with every chunk kind.
    ///
    /// Predicted before the run: name `SMOKE`, user type 7 and string
    /// `hello`; burst 3; a solid sphere of radius 2.5 and a cylinder of
    /// extent 1 whose radius -2 is clamped to 0; colour start (1, 0, 0),
    /// random (0.2, 0, 0), keys (0.5, green) and (1.0, black made
    /// (-0.2, -0, -0)); opacity start 1, one key (1, 0); size start 4, no
    /// keys (count 1); line flags 5; rotation random 0.1, orientation 0.3,
    /// start 0.5, keys (0.2, 1) (0.8, 2); frame start 3, key (0.5, 7); blur
    /// random 0 start 0.1 no keys; future start time 1.5; the obsolete
    /// colour chunk skipped; depth 1.
    #[test]
    fn loads_version_2() {
        let mut body = header(0x0002_0000, b"SMOKE");
        body.extend(user(7, 6, b"hello\0"));
        body.extend(info(b"smoke.tga", |i| i.lifetime = 2.0));
        body.extend(info_v2(|i| {
            i.burst_size = 3;
            i.creation_volume.class_id = CLASSID_SOLIDSPHERE;
            i.creation_volume.value1 = 2.5;
            i.vel_random.class_id = CLASSID_SOLIDCYLINDER;
            i.vel_random.value1 = 1.0;
            i.vel_random.value2 = -2.0;
        }));
        body.extend(props(
            [3, 2, 1],
            [51, 0, 0, 0],
            &[
                key(0.0, [255, 0, 0, 0]),
                key(0.5, [0, 255, 0, 0]),
                key(1.0, [0, 0, 0, 0]),
                fkey(0.0, 1.0),
                fkey(1.0, 0.0),
                fkey(0.0, 4.0),
                fkey(9.0, 9.0),
            ],
        ));
        let mut lp = W3dEmitterLinePropertiesStruct::from_bytes(&[0; 64]);
        lp.flags = 5;
        body.extend(chunk(W3D_CHUNK_EMITTER_LINE_PROPERTIES, &lp.to_bytes()));
        body.extend(rot(2, &[fkey(0.0, 0.5), fkey(0.2, 1.0), fkey(0.8, 2.0)]));
        let mut fb = 1u32.to_le_bytes().to_vec();
        fb.extend([0u8; 12]);
        fb.extend(fkey(0.0, 3.0));
        fb.extend(fkey(0.5, 7.0));
        body.extend(chunk(W3D_CHUNK_EMITTER_FRAME_KEYFRAMES, &fb));
        let mut bb = 0u32.to_le_bytes().to_vec();
        bb.extend(0.25f32.to_le_bytes());
        bb.extend([0u8; 4]);
        bb.extend(fkey(0.0, 0.1));
        body.extend(chunk(W3D_CHUNK_EMITTER_BLUR_TIME_KEYFRAMES, &bb));
        body.extend(chunk(OBSOLETE_W3D_CHUNK_EMITTER_COLOR_KEYFRAME, &[1, 2, 3]));
        let mut eb = 1.5f32.to_le_bytes().to_vec();
        eb.extend([0u8; 36]);
        body.extend(chunk(W3D_CHUNK_EMITTER_EXTRA_INFO, &eb));
        let (r, depth) = load(&body);
        let d = r.unwrap().unwrap();
        assert_eq!(depth, 1);
        assert_eq!(d.name.as_deref(), Some(&b"SMOKE"[..]));
        assert_eq!((d.user_type, d.user_string_c_str()), (7, Some(&b"hello"[..])));
        assert_eq!(d.texture_filename().as_deref(), Some(&b"smoke.tga"[..]));
        assert_eq!((d.info.lifetime, d.info_v2.burst_size), (2.0, 3));
        assert_eq!(d.creation_volume, Some(Vector3Randomizer::SolidSphere(2.5)));
        assert_eq!(d.velocity_randomizer, Some(Vector3Randomizer::SolidCylinder { extent: 1.0, radius: 0.0 }));
        let c = &d.color_keyframes;
        assert_eq!((c.start, c.rand), ([1.0, 0.0, 0.0], [0.2, 0.0, 0.0]));
        let ck = keys(c);
        assert_eq!(ck[0], (0.5, [0.0, 1.0, 0.0]));
        assert_eq!((ck[1].0, ck[1].1.map(f32::to_bits)), (1.0, [(-0.2f32).to_bits(), 0x8000_0000, 0x8000_0000]));
        assert_eq!((d.opacity_keyframes.start, keys(&d.opacity_keyframes)), (1.0, vec![(1.0, 0.0)]));
        assert_eq!((d.size_keyframes.start, d.size_keyframes.num_key_frames, d.size_keyframes.rand), (4.0, 0, 0.25));
        assert_eq!(d.line_properties.flags, 5);
        let r = &d.rotation_keyframes;
        assert_eq!((r.rand, d.initial_orientation_random, r.start), (0.1, 0.3, 0.5));
        assert_eq!(keys(r), vec![(0.2, 1.0), (0.8, 2.0)]);
        assert_eq!((d.frame_keyframes.start, keys(&d.frame_keyframes)), (3.0, vec![(0.5, 7.0)]));
        let b = &d.blur_time_keyframes;
        assert_eq!((b.rand, b.start, b.num_key_frames), (0.25, 0.1, 0));
        assert_eq!(d.extra_info.future_start_time, 1.5);
        assert_eq!(d.class_id(), CLASSID_PARTICLEEMITTER);
    }

    /// `Convert_To_Ver2` for a version-1 emitter.
    ///
    /// Predicted before the run: the texture is asked about once, by its
    /// name as written (`fx\spark.tga`, not cut at the backslash); with an
    /// alpha answer the shader is the alpha sprite (source blend 2,
    /// destination 5, depth mask 0, texturing 1, depth compare 3), without
    /// it the additive sprite (1, 1); solid boxes of 0.5 (500 / 1000) and
    /// 2; colour start (1, 128/255, 0) with one key (0.75, black); opacity
    /// start 1, one key (0.75, 0.2); size start 1.5 with no keys; burst 1.
    /// An info-v2 chunk after the info is skipped. An empty texture name
    /// asks nothing (additive).
    #[test]
    fn converts_version_1() {
        let v1 = emitter_v1(b"SPARK", b"fx\\spark.tga");
        let mut body = v1[8..].to_vec();
        body.extend(info_v2(|i| i.burst_size = 9));
        let mut asked = Vec::new();
        let d = load_with(&body, &mut |n| {
            asked.push(n.to_vec());
            true
        })
        .0
        .unwrap()
        .unwrap();
        assert_eq!(asked, vec![b"fx\\spark.tga".to_vec()]);
        let sh = d.info_v2.shader;
        assert_eq!((sh.src_blend, sh.dest_blend, sh.depth_mask, sh.texturing, sh.depth_compare), (2, 5, 0, 1, 3));
        assert_eq!(d.creation_volume, Some(Vector3Randomizer::SolidBox([0.5; 3])));
        assert_eq!(d.velocity_randomizer, Some(Vector3Randomizer::SolidBox([2.0; 3])));
        assert_eq!(d.info_v2.burst_size, 1);
        assert_eq!(d.color_keyframes.start, [1.0, 128.0 / 255.0, 0.0]);
        assert_eq!(keys(&d.color_keyframes), vec![(0.75, [0.0; 3])]);
        assert_eq!((d.opacity_keyframes.start, keys(&d.opacity_keyframes)), (1.0, vec![(0.75, 51.0 / 255.0)]));
        assert_eq!((d.size_keyframes.start, d.size_keyframes.num_key_frames), (1.5, 0));
        let d = load_with(&body, &mut |_| false).0.unwrap().unwrap();
        assert_eq!((d.info_v2.shader.src_blend, d.info_v2.shader.dest_blend), (1, 1));
        let d = load(&emitter_v1(b"E", b"")[8..]).0.unwrap().unwrap();
        assert_eq!(d.info_v2.shader, convert_shader_to_w3d(PRESET_ADDITIVE_SPRITE_SHADER));
    }

    /// Quirks of the props and the later chunks.
    ///
    /// Predicted before the run: (1) one black colour keyframe with a random
    /// range stays black (index 0 is not tweaked); (2) a props chunk
    /// holding no keys loads with zero starts and zero keys (counts 3, 1,
    /// 1); (3) a user string `abc` without 0 has no C string; (4) a second
    /// rotation chunk with count 0 and no start key keeps the first chunk's
    /// start (0.5) and gets count 0; (5) a rotation chunk with count 3 and
    /// one key ends the load with an error, its failed key zeroed, and the
    /// loop opens the next chunk (depth 2).
    #[test]
    fn props_and_keyframe_quirks() {
        let mut body = header(0x0002_0000, b"Q");
        body.extend(user(0, 0, b""));
        body.extend(info(b"", |_| {}));
        body.extend(info_v2(|_| {}));
        body.extend(props([2, 1, 1], [9, 0, 0, 0], &[key(0.0, [1, 1, 1, 0]), key(1.0, [0; 4])]));
        let d = load(&body).0.unwrap().unwrap();
        assert_eq!(keys(&d.color_keyframes), vec![(1.0, [0.0; 3])]);
        let mut body = header(0x0002_0000, b"Q");
        body.extend(user(0, 3, b"abc"));
        body.extend(info(b"", |_| {}));
        body.extend(info_v2(|_| {}));
        body.extend(props([4, 2, 2], [0; 4], &[]));
        body.extend(rot(1, &[fkey(0.0, 0.5), fkey(0.25, 1.0)]));
        body.extend(rot(0, &[]));
        let d = load(&body).0.unwrap().unwrap();
        assert_eq!(d.user_string_c_str(), None);
        assert_eq!(d.user_string.as_deref(), Some(&b"abc"[..]));
        assert_eq!((d.color_keyframes.start, keys(&d.color_keyframes)), ([0.0; 3], vec![(0.0, [0.0; 3]); 3]));
        assert_eq!(keys(&d.opacity_keyframes), vec![(0.0, 0.0)]);
        assert_eq!((d.rotation_keyframes.start, d.rotation_keyframes.num_key_frames), (0.5, 0));
        let mut body = v2_base(b"Q");
        body.extend(rot(3, &[fkey(0.0, 0.5), fkey(0.25, 1.0)]));
        body.extend(chunk(W3D_CHUNK_EMITTER_EXTRA_INFO, &[0; 40]));
        assert_eq!(load(&body), (Ok(None), 2));
    }

    /// Load errors before the loop, and undefined inputs.
    ///
    /// Predicted before the run: a user-data chunk where the header should
    /// be is left open, and the loop opens a chunk inside it (its 12 zero
    /// bytes read as a chunk header): depth 3; an empty emitter chunk is a
    /// load error at depth 1; a short header is closed, then the loop opens
    /// the next chunk: depth 2. Undefined: a 16-character name
    /// (`StringOverrun`), `SizeofStringParam` 0xFFFFFFFF
    /// (`WriteToEmptyArray`) and 0x7FFFFFFF (`Allocation`), a colour count
    /// of 0 (`Allocation`), a short rotation header and a missing frame key
    /// (`UninitialisedRead`), a version-1 texture name filling `m_Info`
    /// without a 0 (`StringOverrun`).
    #[test]
    fn load_errors_and_undefined() {
        assert_eq!(load(&user(0, 0, b"")), (Ok(None), 3));
        assert_eq!(load(&[]), (Ok(None), 1));
        let mut body = chunk(W3D_CHUNK_EMITTER_HEADER, &[0; 19]);
        body.extend(user(0, 0, b""));
        assert_eq!(load(&body), (Ok(None), 2));
        let undefined = |body: &[u8]| match load(body).0 {
            Err(W3dError::Undefined(u)) => u,
            r => panic!("{r:?}"),
        };
        assert_eq!(undefined(&header(0x20000, b"ABCDEFGHIJKLMNOP")), W3dUndefined::StringOverrun);
        let mut body = header(0x20000, b"N");
        body.extend(user(0, u32::MAX, b""));
        assert_eq!(undefined(&body), W3dUndefined::WriteToEmptyArray);
        let mut body = header(0x20000, b"N");
        body.extend(user(0, 0x7FFF_FFFF, b""));
        assert_eq!(undefined(&body), W3dUndefined::Allocation);
        let mut body = header(0x20000, b"N");
        body.extend(user(0, 0, b""));
        body.extend(info(b"", |_| {}));
        body.extend(info_v2(|_| {}));
        body.extend(props([0, 1, 1], [0; 4], &[]));
        assert_eq!(undefined(&body), W3dUndefined::Allocation);
        let mut body = v2_base(b"N");
        body.extend(chunk(W3D_CHUNK_EMITTER_ROTATION_KEYFRAMES, &[0; 15]));
        assert_eq!(undefined(&body), W3dUndefined::UninitialisedRead);
        let mut body = v2_base(b"N");
        let mut fb = 2u32.to_le_bytes().to_vec();
        fb.extend([0u8; 12]);
        fb.extend(fkey(0.0, 3.0));
        fb.extend(fkey(0.5, 7.0));
        body.extend(chunk(W3D_CHUNK_EMITTER_FRAME_KEYFRAMES, &fb));
        assert_eq!(undefined(&body), W3dUndefined::UninitialisedRead);
        let mut body = header(0x10000, b"N");
        body.extend(user(0, 0, b""));
        body.extend(info(&[b'x'; 260], |i| {
            i.start_size = f32::from_bits(0x4141_4141);
            i.end_size = f32::from_bits(0x4141_4141);
            for f in [&mut i.lifetime, &mut i.emission_rate, &mut i.max_emissions, &mut i.velocity_random] {
                *f = f32::from_bits(0x4141_4141);
            }
            for f in [&mut i.position_random, &mut i.fade_time, &mut i.gravity, &mut i.elasticity] {
                *f = f32::from_bits(0x4141_4141);
            }
            for v in [&mut i.velocity, &mut i.acceleration] {
                (v.x, v.y, v.z) = (f32::from_bits(0x4141_4141), f32::from_bits(0x4141_4141), f32::from_bits(0x4141_4141));
            }
            i.start_color = W3dRGBAStruct { r: 1, g: 1, b: 1, a: 1 };
            i.end_color = W3dRGBAStruct { r: 1, g: 1, b: 1, a: 1 };
        }));
        assert_eq!(undefined(&body), W3dUndefined::StringOverrun);
    }
}
