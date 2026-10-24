// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/ddsfile.cpp, ddsfile.h
//   (DDSFileClass: the constructor, Load, the level accessors,
//   Calculate_DXTC_Surface_Size, Get_Pixel, Get_4x4_Block, the memory
//   Copy_Level_To_Surface, Get_CubeMap_Memory_Pointer,
//   Copy_CubeMap_Level_To_Surface, Copy_Volume_Level_To_Surface,
//   RGB565_To_ARGB8888, ARGB8888_To_RGB565, Combine_Colors)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D2; 08/06/02 KM added cube map and volume
//   texture support)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! DDS textures as WW3D2 reads them (`DDSFileClass`), from an in-memory
//! byte buffer: the header, the mip-level table, the compressed payload,
//! and upstream's DXT1/DXT5 decompression to raw pixel buffers.
//!
//! # The format, as upstream reads it
//!
//! ```text
//! 4 bytes (not checked: any magic is accepted)
//! LegacyDDSURFACEDESC2 (124 bytes on Win32; its Size field must equal
//!   the number of bytes read for it)
//! the levels, largest first
//! ```
//!
//! The format is `PixelFormat.FourCC` through `D3DFormat_To_WW3DFormat`
//! (meant to be DXT1-DXT5; anything else is accepted, see below). The level
//! sizes are computed, never read: `(Width/4)*(Height/4)` blocks of 8 bytes
//! (DXT1) or 16 (DXT2-5), times `Depth` for a volume texture, divided by 4
//! (8 for a volume) per level while larger than 16. `MipMapCount` (0 means
//! 1) loses the reduction factor and then the two smallest levels.
//!
//! # Upstream quirks that are kept
//!
//! - **A reduction factor at least the mip count** sets the count to 1 and
//!   then subtracts that 1 from the factor (`MipLevels=1;
//!   ReductionFactor=ReductionFactor-MipLevels;`), so one fewer level is
//!   skipped than asked.
//! - The constructor divides volume levels by 8, but `Load` skips reduced
//!   levels dividing by 4 and without the depth.
//! - The payload size is `file size - Size - 4` minus the skipped levels, in
//!   `unsigned` arithmetic: it can wrap, and a size of 2^31 or more silently
//!   loads nothing (`Load` still returns true). A file shorter than that
//!   leaves the rest of the buffer as the allocator's zeros.
//! - A FourCC that is not DXT gives whatever `WW3DFormat` the table has for
//!   it (`21` is `A8R8G8B8`); the sizes then count 1 byte per 4x4 block.
//!   `Get_Pixel` returns `0xFFFFFFFF` for every format but DXT1 and DXT5,
//!   and `Get_4x4_Block` writes nothing for them.
//! - Copying a level to a surface of the same format with a hue shift only
//!   handles DXT1 and DXT5 (shifting the two block colours, without
//!   decoding); other formats copy nothing. The DXT1 to DXT2 copy adds an
//!   opaque alpha block. Any other conversion decodes 4x4 blocks with
//!   [`write_b8g8r8a8`] (so `R3G3B2` writes a byte past each pixel). A
//!   destination whose size is not the level's gets nothing.
//! - **Cube maps decode from face 0**: `Copy_CubeMap_Level_To_Surface`
//!   reads the requested face only when it copies blocks as they are; when
//!   it decodes, `Get_4x4_Block` reads `Get_Memory_Pointer(level)`, which is
//!   the first face.
//! - `Get_Volume_Memory_Pointer` returns NULL, so
//!   `Copy_Volume_Level_To_Surface` copies nothing.
//! - DXT1 alpha (the 3-colour mode's index 3) is decoded to transparent
//!   black.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`DdsUndefined`]: a header read too short to give the fields used (its
//! struct is uninitialised); a FourCC with the top bit set; a reduction
//! factor or level of 32 or more in a shift; a level table larger than the
//! 32-bit original can allocate; a level index past the table; reading past
//! the payload or through a missing one; writing past the destination; a
//! hue shift that makes the hue sector NaN or not an `int`. A copy whose
//! block loop never ends upstream (its counter wraps) is reported as
//! [`DdsUndefined::NonTerminating`].
//!
//! # What is left out
//!
//! - The file name handling (the constructor turns `.tga` into `.dds`) and
//!   the file factory: the bytes are given. The date-time is always 0
//!   (`GameFileClass` does not override `Get_Date_Time`).
//! - The `IDirect3DSurface8` overload of `Copy_Level_To_Surface` (it locks
//!   the surface and calls the memory one, which is here).
//! - DXT2, DXT3 and DXT4 decoding: upstream has none (`Get_Pixel` returns
//!   white, `Get_4x4_Block` nothing).

use super::colorspace::{recolor, HueOutOfRange};
use super::format::{bytes_per_pixel, d3d_format_to_ww3d, write_b8g8r8a8, Ww3dFormat};
use crate::ww::chunkio::{GameFile, SeekFrom};

/// `sizeof(LegacyDDSURFACEDESC2)` on Win32 (its `void* Surface` is 4 bytes).
pub const LEGACY_DDSURFACEDESC2_SIZE: usize = 124;
/// `DDSCAPS2_CUBEMAP` (`ddraw.h`).
pub const DDSCAPS2_CUBEMAP: u32 = 0x0000_0200;
/// `DDSCAPS2_VOLUME` (`ddraw.h`).
pub const DDSCAPS2_VOLUME: u32 = 0x0020_0000;
/// The bytes of the surface description the reader uses must have been
/// read: up to `Caps.Caps2` (offset 108).
const SURFACE_DESC_USED: usize = 112;

/// Where upstream is undefined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DdsUndefined {
    /// A surface-description field used without having been read.
    UninitialisedRead,
    /// A FourCC with the top bit set (a negative `D3DFORMAT` indexes before
    /// the conversion table).
    FormatIndex,
    /// A shift by 32 or more (a reduction factor or a level).
    ShiftOutOfRange,
    /// A level table the 32-bit original cannot allocate.
    Allocation,
    /// A level index past the level table.
    IndexOutOfRange,
    /// A level table or payload used that was never allocated.
    NullPointer,
    /// A read past the payload.
    ReadOutOfRange,
    /// A write past the destination surface.
    WriteOutOfRange,
    /// A hue shift whose sector is not an `int` ([`HueOutOfRange`]).
    HueOutOfRange,
    /// Not undefined, but no result either: upstream never returns (a block
    /// loop over a surface 2^32 - 3 or more pixels wide or high, whose
    /// counter wraps).
    NonTerminating,
}

impl From<HueOutOfRange> for DdsUndefined {
    fn from(_: HueOutOfRange) -> Self {
        DdsUndefined::HueOutOfRange
    }
}

/// Result of the DDS functions.
pub type DdsResult<T> = Result<T, DdsUndefined>;

/// `LegacyDDPIXELFORMAT` (each union by its first member).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LegacyDdPixelFormat {
    pub size: u32,
    pub flags: u32,
    pub four_cc: u32,
    pub rgb_bit_count: u32,
    pub r_bit_mask: u32,
    pub g_bit_mask: u32,
    pub b_bit_mask: u32,
    pub rgb_alpha_bit_mask: u32,
}

/// `LegacyDDSURFACEDESC2` (each union by its first member, except `Depth`,
/// which the reader uses; the colour keys as pairs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LegacyDdSurfaceDesc2 {
    pub size: u32,
    pub flags: u32,
    pub height: u32,
    pub width: u32,
    pub pitch: u32,
    pub depth: u32,
    pub mip_map_count: u32,
    pub alpha_bit_depth: u32,
    pub reserved: u32,
    /// `void* Surface` (4 bytes on Win32).
    pub surface: u32,
    pub ck_dest_overlay: [u32; 2],
    pub ck_dest_blt: [u32; 2],
    pub ck_src_overlay: [u32; 2],
    pub ck_src_blt: [u32; 2],
    pub pixel_format: LegacyDdPixelFormat,
    /// `Caps`, `Caps2`, `Caps3`, `Caps4`.
    pub caps: [u32; 4],
    pub texture_stage: u32,
}

impl LegacyDdSurfaceDesc2 {
    /// From the 124 bytes as Win32 lays them out.
    pub fn from_bytes(b: &[u8; LEGACY_DDSURFACEDESC2_SIZE]) -> Self {
        let u = |i: usize| u32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
        LegacyDdSurfaceDesc2 {
            size: u(0),
            flags: u(1),
            height: u(2),
            width: u(3),
            pitch: u(4),
            depth: u(5),
            mip_map_count: u(6),
            alpha_bit_depth: u(7),
            reserved: u(8),
            surface: u(9),
            ck_dest_overlay: [u(10), u(11)],
            ck_dest_blt: [u(12), u(13)],
            ck_src_overlay: [u(14), u(15)],
            ck_src_blt: [u(16), u(17)],
            pixel_format: LegacyDdPixelFormat {
                size: u(18),
                flags: u(19),
                four_cc: u(20),
                rgb_bit_count: u(21),
                r_bit_mask: u(22),
                g_bit_mask: u(23),
                b_bit_mask: u(24),
                rgb_alpha_bit_mask: u(25),
            },
            caps: [u(26), u(27), u(28), u(29)],
            texture_stage: u(30),
        }
    }

    /// The 124 bytes as Win32 lays them out.
    pub fn to_bytes(&self) -> [u8; LEGACY_DDSURFACEDESC2_SIZE] {
        let p = &self.pixel_format;
        let words = [
            self.size,
            self.flags,
            self.height,
            self.width,
            self.pitch,
            self.depth,
            self.mip_map_count,
            self.alpha_bit_depth,
            self.reserved,
            self.surface,
            self.ck_dest_overlay[0],
            self.ck_dest_overlay[1],
            self.ck_dest_blt[0],
            self.ck_dest_blt[1],
            self.ck_src_overlay[0],
            self.ck_src_overlay[1],
            self.ck_src_blt[0],
            self.ck_src_blt[1],
            p.size,
            p.flags,
            p.four_cc,
            p.rgb_bit_count,
            p.r_bit_mask,
            p.g_bit_mask,
            p.b_bit_mask,
            p.rgb_alpha_bit_mask,
            self.caps[0],
            self.caps[1],
            self.caps[2],
            self.caps[3],
            self.texture_stage,
        ];
        let mut b = [0u8; LEGACY_DDSURFACEDESC2_SIZE];
        for (i, w) in words.iter().enumerate() {
            b[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
        }
        b
    }
}

/// `enum DDSType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DdsType {
    Texture,
    CubeMap,
    Volume,
}

/// `LevelSizes` and `LevelOffsets`, held as the rule that fills them: the
/// first size, divided by `dec` per level while larger than 16 (so at most
/// a few dozen sizes differ; the rest are the same), offsets the running
/// `unsigned` sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LevelTable {
    count: u32,
    first: u32,
    dec: u32,
}

impl LevelTable {
    fn check(&self, level: u32) -> DdsResult<()> {
        if level >= self.count {
            return Err(DdsUndefined::IndexOutOfRange);
        }
        Ok(())
    }

    fn size(&self, level: u32) -> u32 {
        let mut s = self.first;
        for _ in 0..level {
            if s > 16 {
                s /= self.dec;
            } else {
                break;
            }
        }
        s
    }

    fn offset(&self, level: u32) -> u32 {
        let mut s = self.first;
        let mut off = 0u32;
        let mut i = 0u32;
        while i < level {
            if s <= 16 {
                return off.wrapping_add(s.wrapping_mul(level - i));
            }
            off = off.wrapping_add(s);
            s /= self.dec;
            i += 1;
        }
        off
    }
}

/// `DDSMemory`: `size` bytes from the game's zero-filling allocator, the
/// first `bytes.len()` of them read from the file (the rest are 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DdsMemory {
    pub size: u32,
    pub bytes: Vec<u8>,
}

/// `DDSFileClass`.
#[derive(Clone, Debug)]
pub struct DdsFile<'a> {
    data: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub full_width: u32,
    pub full_height: u32,
    pub full_depth: u32,
    pub mip_levels: u32,
    /// `Get_Date_Time()`: always 0 here.
    pub date_time: u32,
    pub reduction_factor: u32,
    pub format: Ww3dFormat,
    pub dds_type: DdsType,
    levels: Option<LevelTable>,
    pub cube_face_size: u32,
    /// The surface description and how many of its bytes were read (the
    /// rest are uninitialised upstream; 0 here).
    pub surface_desc: LegacyDdSurfaceDesc2,
    pub surface_desc_read: usize,
    memory: Option<DdsMemory>,
}

/// `RGB565_To_ARGB8888` (alpha 0).
pub fn rgb565_to_argb8888(rgb: u16) -> u32 {
    let rgb = u32::from(rgb);
    (rgb & 0x001f) << 3 | (rgb & 0x07e0) << 5 | (rgb & 0xf800) << 8
}

/// `ARGB8888_To_RGB565`.
pub fn argb8888_to_rgb565(argb: u32) -> u16 {
    let b = argb.to_le_bytes();
    ((u16::from(b[2]) & 0xf8) << 8)
        | ((u16::from(b[1]) & 0xfc) << 3)
        | ((u16::from(b[0]) & 0xf8) >> 3)
}

/// `Combine_Colors(col1, col2, rel)`: `(col1*rel + col2*(255-rel)) >> 8` per
/// channel, red and blue together and green alone, in 32-bit arithmetic.
pub fn combine_colors(col1: u32, col2: u32, rel: u32) -> u32 {
    const R_B_MASK: u32 = 0x00ff_00ff;
    const G_MASK: u32 = 0x0000_ff00;
    let rel2 = 255u32.wrapping_sub(rel);
    let rb = ((col1 & R_B_MASK)
        .wrapping_mul(rel)
        .wrapping_add((col2 & R_B_MASK).wrapping_mul(rel2))
        >> 8)
        & R_B_MASK;
    let g = ((col1 & G_MASK)
        .wrapping_mul(rel)
        .wrapping_add((col2 & G_MASK).wrapping_mul(rel2))
        >> 8)
        & G_MASK;
    rb | g
}

/// `Calculate_DXTC_Surface_Size(width, height, format)`.
pub fn calculate_dxtc_surface_size(width: u32, height: u32, format: Ww3dFormat) -> u32 {
    let n = (width / 4).wrapping_mul(height / 4);
    match format {
        Ww3dFormat::Dxt1 => n.wrapping_mul(8),
        Ww3dFormat::Dxt2 | Ww3dFormat::Dxt3 | Ww3dFormat::Dxt4 | Ww3dFormat::Dxt5 => {
            n.wrapping_mul(16)
        }
        _ => n,
    }
}

fn shr(v: u32, by: u32) -> DdsResult<u32> {
    v.checked_shr(by).ok_or(DdsUndefined::ShiftOutOfRange)
}

fn has_hsv_shift(h: [f32; 3]) -> bool {
    h[0] != 0.0 || h[1] != 0.0 || h[2] != 0.0
}

/// A destination surface being written: bounds-checked stores.
struct Dest<'d> {
    buf: &'d mut [u8],
}

impl Dest<'_> {
    fn put(&mut self, at: u64, bytes: &[u8]) -> DdsResult<()> {
        let at = usize::try_from(at).map_err(|_| DdsUndefined::WriteOutOfRange)?;
        let end = at
            .checked_add(bytes.len())
            .ok_or(DdsUndefined::WriteOutOfRange)?;
        self.buf
            .get_mut(at..end)
            .ok_or(DdsUndefined::WriteOutOfRange)?
            .copy_from_slice(bytes);
        Ok(())
    }

    fn pixel(&mut self, at: u64, format: Ww3dFormat, argb: u32) -> DdsResult<()> {
        let at = usize::try_from(at).map_err(|_| DdsUndefined::WriteOutOfRange)?;
        write_b8g8r8a8(self.buf, at, format, argb).ok_or(DdsUndefined::WriteOutOfRange)
    }
}

impl<'a> DdsFile<'a> {
    fn empty(data: &'a [u8], reduction_factor: u32) -> Self {
        DdsFile {
            data,
            width: 0,
            height: 0,
            depth: 0,
            full_width: 0,
            full_height: 0,
            full_depth: 0,
            mip_levels: 0,
            date_time: 0,
            reduction_factor,
            format: Ww3dFormat::Unknown,
            dds_type: DdsType::Texture,
            levels: None,
            cube_face_size: 0,
            surface_desc: LegacyDdSurfaceDesc2::default(),
            surface_desc_read: 0,
            memory: None,
        }
    }

    /// `DDSFileClass(name, reduction_factor)` on the file `data`: reads the
    /// header and builds the level table. A file it cannot read gives an
    /// object that is not [`available`](Self::is_available).
    pub fn new(data: &'a [u8], reduction_factor: u32) -> DdsResult<Self> {
        let mut d = DdsFile::empty(data, reduction_factor);
        // a file that does not fit an `int` cannot be a RAMFile: treat it
        // as unreadable (not reachable from the game, whose archives are
        // 32-bit)
        let Some(mut file) = GameFile::new(data) else {
            return Ok(d);
        };
        let mut header = [0u8; 4];
        let n = file
            .read(&mut header, 4)
            .map_err(|_| DdsUndefined::ReadOutOfRange)?;
        if n == 0 {
            return Ok(d);
        }
        let mut sd = [0u8; LEGACY_DDSURFACEDESC2_SIZE];
        let read_bytes = file
            .read(&mut sd, LEGACY_DDSURFACEDESC2_SIZE as i32)
            .map_err(|_| DdsUndefined::ReadOutOfRange)? as usize;
        d.surface_desc_read = read_bytes;
        d.surface_desc = LegacyDdSurfaceDesc2::from_bytes(&sd);
        if read_bytes == 0 {
            return Ok(d);
        }
        if read_bytes < 4 {
            return Err(DdsUndefined::UninitialisedRead);
        }
        if read_bytes as u32 != d.surface_desc.size {
            return Ok(d);
        }
        if read_bytes < SURFACE_DESC_USED {
            return Err(DdsUndefined::UninitialisedRead);
        }
        let sd = d.surface_desc;
        d.format = d3d_format_to_ww3d(sd.pixel_format.four_cc).ok_or(DdsUndefined::FormatIndex)?;

        d.mip_levels = sd.mip_map_count;
        if d.mip_levels == 0 {
            d.mip_levels = 1;
        }
        if d.mip_levels > d.reduction_factor {
            d.mip_levels -= d.reduction_factor;
        } else {
            d.mip_levels = 1;
            d.reduction_factor -= d.mip_levels;
        }
        if d.mip_levels > 2 {
            d.mip_levels -= 2;
        } else {
            d.mip_levels = 1;
        }

        if sd.caps[1] & DDSCAPS2_CUBEMAP != 0 {
            d.dds_type = DdsType::CubeMap;
        } else if sd.caps[1] & DDSCAPS2_VOLUME != 0 {
            d.dds_type = DdsType::Volume;
        }

        d.full_width = sd.width;
        d.full_height = sd.height;
        d.full_depth = sd.depth;
        d.width = shr(sd.width, d.reduction_factor)?;
        d.height = shr(sd.height, d.reduction_factor)?;
        d.depth = sd.depth;

        let mut level_size = calculate_dxtc_surface_size(sd.width, sd.height, d.format);
        let mut level_mip_dec = 4;
        if d.dds_type == DdsType::Volume {
            level_size = level_size.wrapping_mul(sd.depth);
            level_mip_dec = 8;
        }

        // `W3DNEWARRAY unsigned[MipLevels]`, twice
        if u64::from(d.mip_levels) * 4 > super::super::w3d::WIN32_MAX_ALLOCATION {
            return Err(DdsUndefined::Allocation);
        }
        for _ in 0..d.reduction_factor {
            if level_size > 16 {
                level_size /= level_mip_dec;
            }
        }
        let table = LevelTable {
            count: d.mip_levels,
            first: level_size,
            dec: level_mip_dec,
        };
        d.levels = Some(table);

        if d.dds_type == DdsType::CubeMap {
            // the sum of the sizes is the offset one past the last level
            d.cube_face_size = table.offset(d.mip_levels);
            if d.mip_levels > 2 {
                d.cube_face_size = d.cube_face_size.wrapping_add(16);
            }
        }
        Ok(d)
    }

    /// `Is_Available()`: the level table exists (the header was read).
    pub fn is_available(&self) -> bool {
        self.levels.is_some()
    }

    fn table(&self) -> DdsResult<&LevelTable> {
        self.levels.as_ref().ok_or(DdsUndefined::NullPointer)
    }

    /// `Get_Width(level)`: `Width >> level`, at least 4.
    pub fn get_width(&self, level: u32) -> DdsResult<u32> {
        Ok(shr(self.width, level)?.max(4))
    }

    /// `Get_Height(level)`.
    pub fn get_height(&self, level: u32) -> DdsResult<u32> {
        Ok(shr(self.height, level)?.max(4))
    }

    /// `Get_Depth(level)`.
    pub fn get_depth(&self, level: u32) -> DdsResult<u32> {
        Ok(shr(self.depth, level)?.max(4))
    }

    /// `Get_Level_Size(level)`: `LevelSizes[level]`.
    pub fn get_level_size(&self, level: u32) -> DdsResult<u32> {
        let t = self.table()?;
        t.check(level)?;
        Ok(t.size(level))
    }

    /// `LevelOffsets[level]`: where the level starts in the payload.
    pub fn get_level_offset(&self, level: u32) -> DdsResult<u32> {
        let t = self.table()?;
        t.check(level)?;
        Ok(t.offset(level))
    }

    /// The payload `Load` read (`DDSMemory`), if it allocated one.
    pub fn memory(&self) -> Option<&DdsMemory> {
        self.memory.as_ref()
    }

    /// `Load()`: reads the payload, skipping the levels the reduction
    /// factor drops. False if it was loaded already, if the header was not
    /// read, or if there is no payload.
    pub fn load(&mut self) -> DdsResult<bool> {
        if self.memory.is_some() {
            return Ok(false);
        }
        if self.levels.is_none() {
            return Ok(false);
        }
        let Some(mut file) = GameFile::new(self.data) else {
            return Ok(false);
        };
        let sd = self.surface_desc;
        let mut size = (file.file_size() as u32)
            .wrapping_sub(sd.size)
            .wrapping_sub(4);
        if size == 0 {
            return Ok(false);
        }
        let mut level_size = calculate_dxtc_surface_size(sd.width, sd.height, self.format);
        let mut skipped_offset = 0u32;
        for _ in 0..self.reduction_factor {
            skipped_offset = skipped_offset.wrapping_add(level_size);
            size = size.wrapping_sub(level_size);
            if level_size > 16 {
                level_size /= 4;
            }
        }
        // Seek(pos) is SEEK_CUR, from 0 on the freshly opened file
        file.seek(
            sd.size.wrapping_add(4).wrapping_add(skipped_offset) as i32,
            SeekFrom::Cur,
        );
        if size != 0 && size < 0x8000_0000 {
            let mut buf = vec![0u8; (size as usize).min(self.data.len())];
            let n = file
                .read(&mut buf, size as i32)
                .map_err(|_| DdsUndefined::ReadOutOfRange)?;
            buf.truncate(n.max(0) as usize);
            self.memory = Some(DdsMemory { size, bytes: buf });
        }
        Ok(true)
    }

    fn mem_byte(&self, off: u64) -> DdsResult<u8> {
        let m = self.memory.as_ref().ok_or(DdsUndefined::NullPointer)?;
        if off >= u64::from(m.size) {
            return Err(DdsUndefined::ReadOutOfRange);
        }
        Ok(m.bytes.get(off as usize).copied().unwrap_or(0))
    }

    fn mem_u16(&self, off: u64) -> DdsResult<u16> {
        Ok(u16::from_le_bytes([
            self.mem_byte(off)?,
            self.mem_byte(off + 1)?,
        ]))
    }

    fn mem_u32(&self, off: u64) -> DdsResult<u32> {
        Ok(u32::from_le_bytes([
            self.mem_byte(off)?,
            self.mem_byte(off + 1)?,
            self.mem_byte(off + 2)?,
            self.mem_byte(off + 3)?,
        ]))
    }

    /// `Get_Memory_Pointer(level)`, as an offset into the payload.
    fn level_ptr(&self, level: u32) -> DdsResult<u64> {
        Ok(u64::from(self.get_level_offset(level)?))
    }

    /// The 8- or 16-byte block holding pixel `(x, y)` of `level`.
    fn block_ptr(&self, level: u32, x: u32, y: u32, block_bytes: u32) -> DdsResult<u64> {
        let base = self.level_ptr(level)?;
        let blocks_per_row = self.get_width(level)? / 4;
        let a = (x / 4).wrapping_mul(block_bytes);
        let b = (y / 4)
            .wrapping_mul(blocks_per_row)
            .wrapping_mul(block_bytes);
        Ok(base + u64::from(a) + u64::from(b))
    }

    /// `Get_Pixel(level, x, y)`: one pixel as A8R8G8B8. DXT1 and DXT5 are
    /// decoded; every other format gives `0xFFFFFFFF`.
    pub fn get_pixel(&self, level: u32, x: u32, y: u32) -> DdsResult<u32> {
        match self.format {
            Ww3dFormat::Dxt1 => {
                let block = self.block_ptr(level, x, y, 8)?;
                let col0 = rgb565_to_argb8888(self.mem_u16(block)?);
                let col1 = rgb565_to_argb8888(self.mem_u16(block + 2)?);
                let line = (self.mem_byte(block + 4 + u64::from(y % 4))? >> ((x % 4) * 2)) & 3;
                Ok(if col0 > col1 {
                    match line {
                        0 => col0 | 0xff00_0000,
                        1 => col1 | 0xff00_0000,
                        2 => combine_colors(col1, col0, 85) | 0xff00_0000,
                        _ => combine_colors(col0, col1, 85) | 0xff00_0000,
                    }
                } else {
                    match line {
                        0 => col0 | 0xff00_0000,
                        1 => col1 | 0xff00_0000,
                        2 => combine_colors(col1, col0, 128) | 0xff00_0000,
                        _ => 0,
                    }
                })
            }
            Ww3dFormat::Dxt5 => {
                let alpha_block = self.block_ptr(level, x, y, 16)?;
                let alpha0 = u32::from(self.mem_byte(alpha_block)?);
                let alpha1 = u32::from(self.mem_byte(alpha_block + 1)?);
                let mut bit_idx = ((x % 4) + 4 * (y % 4)) * 3;
                let mut byte_idx = bit_idx / 8;
                bit_idx %= 8;
                let mut alpha_index = 0u32;
                for i in 0..3 {
                    let bit = (u32::from(self.mem_byte(alpha_block + 2 + u64::from(byte_idx))?)
                        >> bit_idx)
                        & 1;
                    alpha_index |= bit << i;
                    bit_idx += 1;
                    if bit_idx >= 8 {
                        bit_idx = 0;
                        byte_idx += 1;
                    }
                }
                let alpha_value = dxt5_alphas(alpha0, alpha1)[alpha_index as usize] << 24;
                let color_block = alpha_block + 8;
                let col0 = rgb565_to_argb8888(self.mem_u16(color_block)?);
                let col1 = rgb565_to_argb8888(self.mem_u16(color_block + 2)?);
                let line =
                    (self.mem_byte(color_block + 4 + u64::from(y % 4))? >> ((x % 4) * 2)) & 3;
                Ok(match line {
                    0 => col0 | alpha_value,
                    1 => col1 | alpha_value,
                    2 => combine_colors(col1, col0, 85) | alpha_value,
                    _ => combine_colors(col0, col1, 85) | alpha_value,
                })
            }
            _ => Ok(0xffff_ffff),
        }
    }

    /// `Get_4x4_Block(dest_ptr, dest_pitch, dest_format, level, source_x,
    /// source_y, hsv_shift)`: decodes the block at `(source_x, source_y)`
    /// into `dest` starting at byte `at`, rows `dest_pitch` apart. True if
    /// the block had alpha. Writes nothing (false) for formats other than
    /// DXT1 and DXT5.
    #[allow(clippy::too_many_arguments)]
    pub fn get_4x4_block(
        &self,
        dest: &mut [u8],
        at: u64,
        dest_pitch: u32,
        dest_format: Ww3dFormat,
        level: u32,
        source_x: u32,
        source_y: u32,
        hsv_shift: [f32; 3],
    ) -> DdsResult<bool> {
        self.block_4x4(
            &mut Dest { buf: dest },
            at,
            dest_pitch,
            dest_format,
            level,
            source_x,
            source_y,
            hsv_shift,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn block_4x4(
        &self,
        dest: &mut Dest<'_>,
        mut at: u64,
        dest_pitch: u32,
        dest_format: Ww3dFormat,
        level: u32,
        source_x: u32,
        source_y: u32,
        hsv_shift: [f32; 3],
    ) -> DdsResult<bool> {
        let dest_bpp = u64::from(bytes_per_pixel(dest_format));
        let has_hsv = has_hsv_shift(hsv_shift);
        match self.format {
            Ww3dFormat::Dxt1 => {
                let block = self.block_ptr(level, source_x, source_y, 8)?;
                let mut col0 = rgb565_to_argb8888(self.mem_u16(block)?);
                let mut col1 = rgb565_to_argb8888(self.mem_u16(block + 2)?);
                let four_colour = col0 > col1;
                if has_hsv {
                    col0 = recolor(col0, hsv_shift)?;
                    col1 = recolor(col1, hsv_shift)?;
                }
                let mut contains_alpha = false;
                for y in 0..4u64 {
                    let mut tmp = at;
                    at += u64::from(dest_pitch);
                    let mut line = self.mem_byte(block + 4 + y)?;
                    for _ in 0..4 {
                        let pixel = match (four_colour, line & 3) {
                            (_, 0) => col0 | 0xff00_0000,
                            (_, 1) => col1 | 0xff00_0000,
                            (true, 2) => combine_colors(col1, col0, 85) | 0xff00_0000,
                            (true, _) => combine_colors(col0, col1, 85) | 0xff00_0000,
                            (false, 2) => combine_colors(col1, col0, 128) | 0xff00_0000,
                            (false, _) => {
                                contains_alpha = true;
                                0
                            }
                        };
                        line >>= 2;
                        dest.pixel(tmp, dest_format, pixel)?;
                        tmp += dest_bpp;
                    }
                }
                Ok(contains_alpha)
            }
            Ww3dFormat::Dxt5 => {
                let alpha_block = self.block_ptr(level, source_x, source_y, 16)?;
                let alphas = dxt5_alphas(
                    u32::from(self.mem_byte(alpha_block)?),
                    u32::from(self.mem_byte(alpha_block + 1)?),
                );
                let color_block = alpha_block + 8;
                let mut col0 = rgb565_to_argb8888(self.mem_u16(color_block)?);
                let mut col1 = rgb565_to_argb8888(self.mem_u16(color_block + 2)?);
                if has_hsv {
                    col0 = recolor(col0, hsv_shift)?;
                    col1 = recolor(col1, hsv_shift)?;
                }
                let mut contains_alpha = 0xffu32;
                let mut indices = [0u32; 16];
                for a in 0..2u64 {
                    let p = alpha_block + 3 * a;
                    let b2 = u32::from(self.mem_byte(p + 2)?);
                    let b3 = u32::from(self.mem_byte(p + 3)?);
                    let b4 = u32::from(self.mem_byte(p + 4)?);
                    let ai = &mut indices[8 * a as usize..8 * a as usize + 8];
                    ai[0] = b2 & 0x7;
                    ai[1] = (b2 >> 3) & 0x7;
                    ai[2] = (b2 >> 6) | ((b3 & 1) << 2);
                    ai[3] = (b3 >> 1) & 0x7;
                    ai[4] = (b3 >> 4) & 0x7;
                    ai[5] = (b3 >> 7) | ((b4 & 3) << 1);
                    ai[6] = (b4 >> 2) & 0x7;
                    ai[7] = b4 >> 5;
                }
                let mut aii = 0;
                for y in 0..4u64 {
                    let mut tmp = at;
                    at += u64::from(dest_pitch);
                    let mut line = self.mem_byte(color_block + 4 + y)?;
                    for _ in 0..4 {
                        let alpha = alphas[indices[aii] as usize];
                        aii += 1;
                        contains_alpha &= alpha;
                        let alpha = alpha << 24;
                        let pixel = match line & 3 {
                            0 => col0 | alpha,
                            1 => col1 | alpha,
                            2 => combine_colors(col1, col0, 85) | alpha,
                            _ => combine_colors(col0, col1, 85) | alpha,
                        };
                        line >>= 2;
                        dest.pixel(tmp, dest_format, pixel)?;
                        tmp += dest_bpp;
                    }
                }
                Ok(contains_alpha != 0xff)
            }
            _ => Ok(false),
        }
    }

    /// `Copy_Level_To_Surface(level, dest_format, dest_width, dest_height,
    /// dest_surface, dest_pitch, hsv_shift)` into `dest`.
    #[allow(clippy::too_many_arguments)]
    pub fn copy_level_to_surface(
        &self,
        level: u32,
        dest_format: Ww3dFormat,
        dest_width: u32,
        dest_height: u32,
        dest: &mut [u8],
        dest_pitch: u32,
        hsv_shift: [f32; 3],
    ) -> DdsResult<()> {
        if self.memory.is_none() {
            return Ok(());
        }
        let src = self.level_ptr(level)?;
        self.copy_to_surface(
            src,
            level,
            dest_format,
            dest_width,
            dest_height,
            &mut Dest { buf: dest },
            dest_pitch,
            hsv_shift,
        )
    }

    /// `Get_CubeMap_Memory_Pointer(face, level)`, as an offset into the
    /// payload: `CubeFaceSize*face + LevelOffsets[level]` in `unsigned`.
    pub fn get_cube_map_offset(&self, face: u32, level: u32) -> DdsResult<u32> {
        Ok(self
            .cube_face_size
            .wrapping_mul(face)
            .wrapping_add(self.get_level_offset(level)?))
    }

    /// `Copy_CubeMap_Level_To_Surface(face, level, ...)`: as
    /// [`copy_level_to_surface`](Self::copy_level_to_surface) from the
    /// face's level, except that decoding reads face 0 (see the module docs).
    #[allow(clippy::too_many_arguments)]
    pub fn copy_cube_map_level_to_surface(
        &self,
        face: u32,
        level: u32,
        dest_format: Ww3dFormat,
        dest_width: u32,
        dest_height: u32,
        dest: &mut [u8],
        dest_pitch: u32,
        hsv_shift: [f32; 3],
    ) -> DdsResult<()> {
        if self.memory.is_none() {
            return Ok(());
        }
        let src = u64::from(self.get_cube_map_offset(face, level)?);
        self.copy_to_surface(
            src,
            level,
            dest_format,
            dest_width,
            dest_height,
            &mut Dest { buf: dest },
            dest_pitch,
            hsv_shift,
        )
    }

    /// `Copy_Volume_Level_To_Surface(...)`: copies nothing
    /// (`Get_Volume_Memory_Pointer` returns NULL).
    pub fn copy_volume_level_to_surface(&self) {}

    /// The body shared by the two copy functions, from the source level at
    /// `src`.
    #[allow(clippy::too_many_arguments)]
    fn copy_to_surface(
        &self,
        mut src: u64,
        level: u32,
        dest_format: Ww3dFormat,
        dest_width: u32,
        dest_height: u32,
        dest: &mut Dest<'_>,
        dest_pitch: u32,
        hsv_shift: [f32; 3],
    ) -> DdsResult<()> {
        let has_hsv = has_hsv_shift(hsv_shift);
        let blocks = |w: u32, h: u32| u64::from(w.div_ceil(4)) * u64::from(h.div_ceil(4));
        if dest_format == self.format
            && dest_width == self.get_width(level)?
            && dest_height == self.get_height(level)?
        {
            if has_hsv {
                let n = blocks(dest_width, dest_height);
                let mut d = 0u64;
                match self.format {
                    Ww3dFormat::Dxt1 | Ww3dFormat::Dxt5 => {
                        for _ in 0..n {
                            if self.format == Ww3dFormat::Dxt5 {
                                for _ in 0..2 {
                                    dest.put(d, &self.mem_u32(src)?.to_le_bytes())?;
                                    src += 4;
                                    d += 4;
                                }
                            }
                            let cols = self.recolor_block_colours(self.mem_u32(src)?, hsv_shift)?;
                            src += 4;
                            dest.put(d, &cols.to_le_bytes())?;
                            d += 4;
                            dest.put(d, &self.mem_u32(src)?.to_le_bytes())?;
                            src += 4;
                            d += 4;
                        }
                    }
                    _ => {}
                }
            } else {
                let size = self.get_level_size(level)?;
                let mut bytes = Vec::with_capacity(size.min(1 << 24) as usize);
                for i in 0..u64::from(size) {
                    bytes.push(self.mem_byte(src + i)?);
                }
                dest.put(0, &bytes)?;
            }
        } else if dest_width == self.get_width(level)? && dest_height == self.get_height(level)? {
            if self.format == Ww3dFormat::Dxt1 && dest_format == Ww3dFormat::Dxt2 {
                let n = blocks(dest_width, dest_height);
                let mut d = 0u64;
                for _ in 0..n {
                    dest.put(d, &[0xff; 8])?;
                    d += 8;
                    let cols = self.mem_u32(src)?;
                    let cols = if has_hsv {
                        self.recolor_block_colours(cols, hsv_shift)?
                    } else {
                        cols
                    };
                    src += 4;
                    dest.put(d, &cols.to_le_bytes())?;
                    d += 4;
                    dest.put(d, &self.mem_u32(src)?.to_le_bytes())?;
                    src += 4;
                    d += 4;
                }
            } else {
                if !matches!(self.format, Ww3dFormat::Dxt1 | Ww3dFormat::Dxt5) {
                    // Get_4x4_Block does nothing for these formats, so the
                    // loop changes nothing (it only takes time: up to 2^60
                    // calls); it ends unless a counter wraps
                    if dest_height > 0xFFFF_FFFC || (dest_height > 0 && dest_width > 0xFFFF_FFFC) {
                        return Err(DdsUndefined::NonTerminating);
                    }
                    return Ok(());
                }
                let dest_bpp = bytes_per_pixel(dest_format);
                let mut y = 0u32;
                while y < dest_height {
                    let mut at = u64::from(y.wrapping_mul(dest_pitch));
                    let mut x = 0u32;
                    while x < dest_width {
                        self.block_4x4(dest, at, dest_pitch, dest_format, level, x, y, hsv_shift)?;
                        x = x.wrapping_add(4);
                        at += u64::from(dest_bpp.wrapping_mul(4));
                        if x == 0 {
                            return Err(DdsUndefined::NonTerminating);
                        }
                    }
                    y = y.wrapping_add(4);
                    if y == 0 {
                        return Err(DdsUndefined::NonTerminating);
                    }
                }
            }
        }
        Ok(())
    }

    /// The hue-shift rewrite of a block's two colours (`cols` holds col1 in
    /// its low half, col0 in its high half as upstream names them).
    fn recolor_block_colours(&self, cols: u32, hsv_shift: [f32; 3]) -> DdsResult<u32> {
        let col0 = recolor(rgb565_to_argb8888((cols >> 16) as u16), hsv_shift)?;
        let col1 = recolor(rgb565_to_argb8888((cols & 0xffff) as u16), hsv_shift)?;
        Ok(u32::from(argb8888_to_rgb565(col0)) << 16 | u32::from(argb8888_to_rgb565(col1)))
    }
}

/// The 8 alphas of a DXT5 block: 8-alpha (`alpha0 > alpha1`) or 6-alpha
/// (with 0 and 255).
fn dxt5_alphas(a0: u32, a1: u32) -> [u32; 8] {
    if a0 > a1 {
        [
            a0,
            a1,
            (6 * a0 + a1 + 3) / 7,
            (5 * a0 + 2 * a1 + 3) / 7,
            (4 * a0 + 3 * a1 + 3) / 7,
            (3 * a0 + 4 * a1 + 3) / 7,
            (2 * a0 + 5 * a1 + 3) / 7,
            (a0 + 6 * a1 + 3) / 7,
        ]
    } else {
        [
            a0,
            a1,
            (4 * a0 + a1 + 2) / 5,
            (3 * a0 + 2 * a1 + 2) / 5,
            (2 * a0 + 3 * a1 + 2) / 5,
            (a0 + 4 * a1 + 2) / 5,
            0,
            255,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::texture::format::D3DFMT_DXT1;

    fn dds(width: u32, height: u32, mips: u32, four_cc: u32, payload: &[u8]) -> Vec<u8> {
        let mut f = b"DDS ".to_vec();
        let mut sd = [0u32; 31];
        sd[0] = 124;
        sd[2] = height;
        sd[3] = width;
        sd[6] = mips;
        sd[20] = four_cc;
        for v in sd {
            f.extend_from_slice(&v.to_le_bytes());
        }
        f.extend_from_slice(payload);
        f
    }

    #[test]
    fn level_table_and_dxt1_decode() {
        // 8x8 DXT1, 4 mips -> 2 kept: 32 bytes then 8
        let mut payload = vec![0u8; 40];
        // block 0: col0 = white, col1 = black, indices 0,1,2,3 per row
        payload[..8].copy_from_slice(&[0xff, 0xff, 0x00, 0x00, 0xE4, 0xE4, 0xE4, 0xE4]);
        let f = dds(8, 8, 4, D3DFMT_DXT1, &payload);
        let mut d = DdsFile::new(&f, 0).unwrap();
        assert!(d.is_available());
        assert_eq!(d.mip_levels, 2);
        assert_eq!((d.get_level_size(0), d.get_level_size(1)), (Ok(32), Ok(8)));
        assert_eq!(d.get_level_offset(1), Ok(32));
        assert_eq!(d.get_level_size(2), Err(DdsUndefined::IndexOutOfRange));
        assert_eq!(d.load(), Ok(true));
        assert_eq!(d.load(), Ok(false));
        assert_eq!(d.get_pixel(0, 0, 0), Ok(0xfff8_fcf8)); // 565 expands without replicating bits
        assert_eq!(d.get_pixel(0, 1, 0), Ok(0xff00_0000));
        assert_eq!(
            d.get_pixel(0, 2, 0),
            Ok(combine_colors(0, 0x00f8_fcf8, 85) | 0xff00_0000)
        );
        let mut out = vec![0u8; 8 * 8 * 4];
        d.copy_level_to_surface(0, Ww3dFormat::A8R8G8B8, 8, 8, &mut out, 32, [0.0; 3])
            .unwrap();
        assert_eq!(&out[..8], &[0xf8, 0xfc, 0xf8, 0xff, 0, 0, 0, 0xff]);
        // the R3G3B2 quirk writes one byte past an exactly sized surface
        let mut small = vec![0u8; 64];
        assert_eq!(
            d.copy_level_to_surface(0, Ww3dFormat::R3G3B2, 8, 8, &mut small, 8, [0.0; 3]),
            Err(DdsUndefined::WriteOutOfRange)
        );
    }

    #[test]
    fn level_offsets_closed_form() {
        let t = LevelTable {
            count: 100,
            first: 4096,
            dec: 4,
        };
        let mut s = 4096u32;
        let mut off = 0u32;
        for l in 0..100 {
            assert_eq!((t.size(l), t.offset(l)), (s, off), "{l}");
            off = off.wrapping_add(s);
            if s > 16 {
                s /= 4;
            }
        }
    }
}
