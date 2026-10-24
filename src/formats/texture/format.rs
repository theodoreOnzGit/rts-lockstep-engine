// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/ww3dformat.h, ww3dformat.cpp
//   (WW3DFormat, Get_Bytes_Per_Pixel, Get_WW3D_Format from a Targa header)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/formconv.cpp
//   (D3DFormat_To_WW3DFormat, Init_D3D_To_WW3_Conversion)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/bitmaphandler.h
//   (BitmapHandlerClass::Write_B8G8R8A8)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D2)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Surface formats: WW3D's `WW3DFormat`, the Direct3D 8 format codes a DDS
//! header carries, and the one pixel writer the DDS decompressor uses.
//!
//! The Direct3D 8 SDK is not in the release, so the `D3DFMT_*` values are
//! the documented ones of `d3d8types.h` (`D3DFMT_R8G8B8 = 20` ...,
//! `D3DFMT_X8L8V8U8 = 62`, the DXT codes `MAKEFOURCC('D','X','T','n')`);
//! the differential tests compile `formconv.cpp` against a header with
//! exactly these values.

/// `enum WW3DFormat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum Ww3dFormat {
    Unknown = 0,
    R8G8B8,
    A8R8G8B8,
    X8R8G8B8,
    R5G6B5,
    X1R5G5B5,
    A1R5G5B5,
    A4R4G4B4,
    R3G3B2,
    A8,
    A8R3G3B2,
    X4R4G4B4,
    A8P8,
    P8,
    L8,
    A8L8,
    A4L4,
    U8V8,
    L6V5U5,
    X8L8V8U8,
    Dxt1,
    Dxt2,
    Dxt3,
    Dxt4,
    Dxt5,
}

impl Ww3dFormat {
    /// All formats, in enum order (`WW3D_FORMAT_COUNT` of them).
    pub const ALL: [Ww3dFormat; 25] = [
        Ww3dFormat::Unknown,
        Ww3dFormat::R8G8B8,
        Ww3dFormat::A8R8G8B8,
        Ww3dFormat::X8R8G8B8,
        Ww3dFormat::R5G6B5,
        Ww3dFormat::X1R5G5B5,
        Ww3dFormat::A1R5G5B5,
        Ww3dFormat::A4R4G4B4,
        Ww3dFormat::R3G3B2,
        Ww3dFormat::A8,
        Ww3dFormat::A8R3G3B2,
        Ww3dFormat::X4R4G4B4,
        Ww3dFormat::A8P8,
        Ww3dFormat::P8,
        Ww3dFormat::L8,
        Ww3dFormat::A8L8,
        Ww3dFormat::A4L4,
        Ww3dFormat::U8V8,
        Ww3dFormat::L6V5U5,
        Ww3dFormat::X8L8V8U8,
        Ww3dFormat::Dxt1,
        Ww3dFormat::Dxt2,
        Ww3dFormat::Dxt3,
        Ww3dFormat::Dxt4,
        Ww3dFormat::Dxt5,
    ];

    /// The format with enum value `v`, if there is one.
    pub fn from_u32(v: u32) -> Option<Ww3dFormat> {
        Ww3dFormat::ALL.get(v as usize).copied()
    }

    /// One of the DXTn formats.
    pub fn is_dxt(self) -> bool {
        matches!(
            self,
            Ww3dFormat::Dxt1
                | Ww3dFormat::Dxt2
                | Ww3dFormat::Dxt3
                | Ww3dFormat::Dxt4
                | Ww3dFormat::Dxt5
        )
    }
}

/// `MAKEFOURCC(a, b, c, d)`.
pub const fn make_fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    a as u32 | (b as u32) << 8 | (c as u32) << 16 | (d as u32) << 24
}

pub const D3DFMT_DXT1: u32 = make_fourcc(b'D', b'X', b'T', b'1');
pub const D3DFMT_DXT2: u32 = make_fourcc(b'D', b'X', b'T', b'2');
pub const D3DFMT_DXT3: u32 = make_fourcc(b'D', b'X', b'T', b'3');
pub const D3DFMT_DXT4: u32 = make_fourcc(b'D', b'X', b'T', b'4');
pub const D3DFMT_DXT5: u32 = make_fourcc(b'D', b'X', b'T', b'5');
/// `HIGHEST_SUPPORTED_D3DFORMAT` (`D3DFMT_X8L8V8U8`).
pub const HIGHEST_SUPPORTED_D3DFORMAT: u32 = 62;

/// `D3DFormatToWW3DFormatConversionArray` once `Init_D3D_To_WW3_Conversion`
/// has filled it (which the game does when it starts Direct3D, before any
/// texture is read): the entries it sets, the rest `WW3D_FORMAT_UNKNOWN`.
fn d3d_table(code: u32) -> Ww3dFormat {
    match code {
        20 => Ww3dFormat::R8G8B8,
        21 => Ww3dFormat::A8R8G8B8,
        22 => Ww3dFormat::X8R8G8B8,
        23 => Ww3dFormat::R5G6B5,
        24 => Ww3dFormat::X1R5G5B5,
        25 => Ww3dFormat::A1R5G5B5,
        26 => Ww3dFormat::A4R4G4B4,
        27 => Ww3dFormat::R3G3B2,
        28 => Ww3dFormat::A8,
        29 => Ww3dFormat::A8R3G3B2,
        30 => Ww3dFormat::X4R4G4B4,
        40 => Ww3dFormat::A8P8,
        41 => Ww3dFormat::P8,
        50 => Ww3dFormat::L8,
        51 => Ww3dFormat::A8L8,
        52 => Ww3dFormat::A4L4,
        60 => Ww3dFormat::U8V8,
        61 => Ww3dFormat::L6V5U5,
        62 => Ww3dFormat::X8L8V8U8,
        _ => Ww3dFormat::Unknown,
    }
}

/// `D3DFormat_To_WW3DFormat(d3d_format)`. `None` where upstream is
/// undefined: `D3DFORMAT` is an `int` enum to MSVC, so a code with the top
/// bit set is negative, passes the `> HIGHEST_SUPPORTED_D3DFORMAT` check
/// and indexes the table before its start.
pub fn d3d_format_to_ww3d(d3d_format: u32) -> Option<Ww3dFormat> {
    Some(match d3d_format {
        D3DFMT_DXT1 => Ww3dFormat::Dxt1,
        D3DFMT_DXT2 => Ww3dFormat::Dxt2,
        D3DFMT_DXT3 => Ww3dFormat::Dxt3,
        D3DFMT_DXT4 => Ww3dFormat::Dxt4,
        D3DFMT_DXT5 => Ww3dFormat::Dxt5,
        _ => {
            if (d3d_format as i32) < 0 {
                return None;
            }
            if d3d_format > HIGHEST_SUPPORTED_D3DFORMAT {
                Ww3dFormat::Unknown
            } else {
                d3d_table(d3d_format)
            }
        }
    })
}

/// `Get_Bytes_Per_Pixel(format)`: 0 for the formats it does not list (the
/// DXT and paletted-with-alpha ones, among others).
pub fn bytes_per_pixel(format: Ww3dFormat) -> u32 {
    use Ww3dFormat::*;
    match format {
        X8R8G8B8 | X8L8V8U8 | A8R8G8B8 => 4,
        R8G8B8 => 3,
        A1R5G5B5 | A4R4G4B4 | U8V8 | L6V5U5 | R5G6B5 => 2,
        R3G3B2 | L8 | A8 | P8 => 1,
        _ => 0,
    }
}

/// `BitmapHandlerClass::Write_B8G8R8A8(dest_ptr, dest_format, argb)`: writes
/// the 32-bit colour `argb` (bytes B, G, R, A in memory) at `dest[at..]` in
/// `dest_format`. Formats it does not handle (DXT, P8 and the rest) write
/// nothing. Returns `None` if a byte it writes is outside `dest`.
///
/// Upstream quirk, reproduced: `R3G3B2` is one byte per pixel but is stored
/// through an `unsigned short*`, so it writes **two** bytes, the second 0
/// (the next pixel's byte, or one past the end of the row).
pub fn write_b8g8r8a8(
    dest: &mut [u8],
    at: usize,
    dest_format: Ww3dFormat,
    argb: u32,
) -> Option<()> {
    use Ww3dFormat::*;
    let c = argb.to_le_bytes();
    let mut put = |bytes: &[u8]| -> Option<()> {
        dest.get_mut(at..at.checked_add(bytes.len())?)?
            .copy_from_slice(bytes);
        Some(())
    };
    match dest_format {
        A8R8G8B8 | X8R8G8B8 => put(&c),
        R8G8B8 => put(&c[..3]),
        A4R4G4B4 => {
            let tmp = (u16::from(c[3]) & 0xf0) << 8
                | (u16::from(c[2]) & 0xf0) << 4
                | (u16::from(c[1]) & 0xf0)
                | (u16::from(c[0]) & 0xf0) >> 4;
            put(&tmp.to_le_bytes())
        }
        A1R5G5B5 => {
            let mut tmp: u16 = if c[3] != 0 { 0x8000 } else { 0 };
            tmp |= (u16::from(c[2]) & 0xf8) << 7;
            tmp |= (u16::from(c[1]) & 0xf8) << 2;
            tmp |= (u16::from(c[0]) & 0xf8) >> 3;
            put(&tmp.to_le_bytes())
        }
        R5G6B5 => {
            let tmp = (u16::from(c[2]) & 0xf8) << 8
                | (u16::from(c[1]) & 0xfc) << 3
                | (u16::from(c[0]) & 0xf8) >> 3;
            put(&tmp.to_le_bytes())
        }
        R3G3B2 => {
            let tmp = (c[2] & 0xe0) | (c[1] & 0xe0) >> 3 | (c[0] & 0xc0) >> 6;
            put(&u16::from(tmp).to_le_bytes())
        }
        L8 => {
            let y =
                (u32::from(c[0]) * 0x1275 + u32::from(c[1]) * 0xB725 + u32::from(c[2]) * 0x3666)
                    >> 16;
            put(&[y as u8])
        }
        A8 => put(&c[..1]),
        _ => Some(()),
    }
}

/// `Get_WW3D_Format(src_format, src_bpp, targa)`: the source format a
/// texture loader assumes for a Targa, from its pixel depth (and, at 8 bits,
/// its colour-map flag and image type). `(Unknown, 0)` for other depths.
pub fn ww3d_format_of_targa(
    pixel_depth: i8,
    color_map_type: i8,
    image_type: i8,
) -> (Ww3dFormat, u32) {
    match pixel_depth {
        32 => (Ww3dFormat::A8R8G8B8, 4),
        24 => (Ww3dFormat::R8G8B8, 3),
        16 => (Ww3dFormat::A1R5G5B5, 2),
        8 => {
            let f = if color_map_type == 1 {
                Ww3dFormat::P8
            } else if image_type == super::targa::TGA_MONO {
                Ww3dFormat::L8
            } else {
                Ww3dFormat::A8
            };
            (f, 1)
        }
        _ => (Ww3dFormat::Unknown, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d3d_codes() {
        assert_eq!(D3DFMT_DXT1, 0x3154_5844);
        assert_eq!(d3d_format_to_ww3d(21), Some(Ww3dFormat::A8R8G8B8));
        assert_eq!(d3d_format_to_ww3d(63), Some(Ww3dFormat::Unknown));
        assert_eq!(d3d_format_to_ww3d(0x8000_0000), None);
        assert_eq!(d3d_format_to_ww3d(D3DFMT_DXT5), Some(Ww3dFormat::Dxt5));
    }

    #[test]
    fn r3g3b2_writes_two_bytes() {
        let mut b = [0xAAu8; 3];
        assert_eq!(
            write_b8g8r8a8(&mut b, 0, Ww3dFormat::R3G3B2, 0x00FF_FFFF),
            Some(())
        );
        assert_eq!(b, [0xFF, 0x00, 0xAA]);
        assert_eq!(write_b8g8r8a8(&mut b, 2, Ww3dFormat::R3G3B2, 0), None);
    }
}
