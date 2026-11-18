//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/bitmaphandler.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/bitmaphandler.h
//!
//! Reused from the port: `BitmapHandlerClass::Write_B8G8R8A8` is
//! `crate::formats::texture::format::write_b8g8r8a8` (already translated, with the R3G3B2 two-byte quirk);
//! `WW3DFormat` and `Get_Bytes_Per_Pixel` are in the same module; `Recolor` is
//! `crate::formats::texture::colorspace::recolor`.
//!
//! Surfaces are byte slices addressed by byte offsets, as upstream's `unsigned char *`. Upstream's pointer
//! arithmetic that leaves the surface (the bump-map branch reads one pixel left of a row start and, at the first
//! row, one row up before it replaces the pointer) is undefined there; reads outside the slice give 0 here.
//! A pixel upstream cannot handle (`Bitmap_Assert(0)`) is a debug assertion and leaves the output unchanged.
//! The in-place mipmap of `Copy_Image` writes into the *source* surface (`mip_ptr = src_surface`), so that
//! surface is `&mut`.

use crate::formats::texture::colorspace::recolor;
use crate::formats::texture::format::{bytes_per_pixel, write_b8g8r8a8, Ww3dFormat};

/// `Bitmap_Assert(condition)` (bitmaphandler.cpp:30): `WWASSERT(condition)`.
pub fn bitmap_assert(condition: bool) {
    debug_assert!(condition);
}

/// A byte of the surface at a possibly out-of-range offset (0 outside; see the module comment).
fn byte_at(buf: &[u8], at: isize) -> u8 {
    if at < 0 {
        return 0;
    }
    buf.get(at as usize).copied().unwrap_or(0)
}

fn rd32(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([byte_at(buf, at as isize), byte_at(buf, at as isize + 1), byte_at(buf, at as isize + 2), byte_at(buf, at as isize + 3)])
}

fn wr32(buf: &mut [u8], at: usize, v: u32) {
    if let Some(s) = buf.get_mut(at..at + 4) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

fn rd16(buf: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([byte_at(buf, at as isize), byte_at(buf, at as isize + 1)])
}

fn wr16(buf: &mut [u8], at: usize, v: u16) {
    if let Some(s) = buf.get_mut(at..at + 2) {
        s.copy_from_slice(&v.to_le_bytes());
    }
}

/// `Recolor(b8g8r8a8, hsv_shift)`: where the hue is out of range upstream leaves the colour alone.
fn recolor_in_place(b8g8r8a8: &mut u32, hsv_shift: [f32; 3]) {
    if let Ok(c) = recolor(*b8g8r8a8, hsv_shift) {
        *b8g8r8a8 = c;
    }
}

/// `BitmapHandlerClass::Read_B8G8R8A8(unsigned char *argb, src_ptr, src_format, palette, palette_bpp)`
/// (bitmaphandler.h): reads the pixel at `src[at..]` as B, G, R, A bytes. Formats it cannot read
/// (DXT, unlisted, a palette depth other than 3 or 4) leave `argb` as it was.
pub fn read_b8g8r8a8_bytes(argb: &mut [u8; 4], src: &[u8], at: isize, src_format: Ww3dFormat, palette: &[u8], palette_bpp: u32) {
    use Ww3dFormat::*;
    match src_format {
        A8R8G8B8 | X8R8G8B8 => {
            for i in 0..4 {
                argb[i] = byte_at(src, at + i as isize);
            }
        }
        R8G8B8 => {
            argb[0] = byte_at(src, at);
            argb[1] = byte_at(src, at + 1);
            argb[2] = byte_at(src, at + 2);
            argb[3] = 0xff;
        }
        A4R4G4B4 => {
            let tmp = u16::from_le_bytes([byte_at(src, at), byte_at(src, at + 1)]);
            argb[0] = ((tmp & 0x000f) << 4) as u8;
            argb[1] = (tmp & 0x00f0) as u8;
            argb[2] = ((tmp & 0x0f00) >> 4) as u8;
            argb[3] = ((tmp & 0xf000) >> 8) as u8;
        }
        A1R5G5B5 => {
            let tmp = u16::from_le_bytes([byte_at(src, at), byte_at(src, at + 1)]);
            argb[3] = if tmp & 0x8000 != 0 { 0xff } else { 0x0 };
            argb[2] = ((tmp >> 7) & 0xf8) as u8;
            argb[1] = ((tmp >> 2) & 0xf8) as u8;
            argb[0] = ((tmp << 3) & 0xf8) as u8;
        }
        R5G6B5 => {
            let tmp = u16::from_le_bytes([byte_at(src, at), byte_at(src, at + 1)]);
            argb[3] = 0xff;
            argb[2] = ((tmp >> 8) & 0xf8) as u8;
            argb[1] = ((tmp >> 3) & 0xfc) as u8;
            argb[0] = ((tmp << 3) & 0xf8) as u8;
        }
        R3G3B2 => {
            let tmp = byte_at(src, at);
            argb[3] = 0xff;
            argb[2] = tmp & 0xe0;
            argb[1] = (tmp << 3) & 0xe0;
            argb[0] = (tmp << 6) & 0xc0;
        }
        L8 => {
            let tmp = byte_at(src, at);
            argb[0] = tmp;
            argb[1] = tmp;
            argb[2] = tmp;
            argb[3] = 0xff;
        }
        A8 => {
            argb[0] = 0;
            argb[1] = 0;
            argb[2] = 0;
            argb[3] = byte_at(src, at);
        }
        P8 => {
            let index = byte_at(src, at) as usize;
            let p = |k: usize| palette.get(palette_bpp as usize * index + k).copied().unwrap_or(0);
            match palette_bpp {
                4 => {
                    argb[0] = p(3);
                    argb[1] = p(2);
                    argb[2] = p(1);
                    argb[3] = p(0);
                }
                3 => {
                    argb[0] = p(2);
                    argb[1] = p(1);
                    argb[2] = p(0);
                    argb[3] = 0xff;
                }
                _ => bitmap_assert(false),
            }
        }
        _ => bitmap_assert(false), // DXT1..DXT5 and the rest
    }
}

/// `Read_B8G8R8A8(unsigned &argb, ...)` (bitmaphandler.h): the same as a little-endian word.
pub fn read_b8g8r8a8(argb: &mut u32, src: &[u8], at: isize, src_format: Ww3dFormat, palette: &[u8], palette_bpp: u32) {
    let mut b = argb.to_le_bytes();
    read_b8g8r8a8_bytes(&mut b, src, at, src_format, palette, palette_bpp);
    *argb = u32::from_le_bytes(b);
}

/// `Read_B8G8R8A8(unsigned &argb, src_ptr, src_format, x, y, width, height, palette, palette_bpp)`
/// (bitmaphandler.h): 0 outside the image, else the pixel at (x, y) of a tightly packed image.
pub fn read_b8g8r8a8_xy(argb: &mut u32, src: &[u8], src_format: Ww3dFormat, x: i32, y: i32, width: i32, height: i32, palette: &[u8], palette_bpp: u32) {
    if x < 0 || y < 0 || x >= width || y >= height {
        *argb = 0;
        return;
    }
    let bpp = bytes_per_pixel(src_format) as i64;
    let at = bpp * i64::from(x) + i64::from(width) * bpp * i64::from(y);
    read_b8g8r8a8(argb, src, at as isize, src_format, palette, palette_bpp);
}

/// `BitmapHandlerClass::Write_B8G8R8A8(dest_ptr, dest_format, const unsigned &argb)` (bitmaphandler.h): see
/// `crate::formats::texture::format::write_b8g8r8a8` (the port's translation; a write outside `dest` is dropped).
/// DXT, P8 (paletted destination) and unlisted formats are `Bitmap_Assert(0)` upstream; the port's version writes
/// nothing for them.
pub fn write_b8g8r8a8_word(dest: &mut [u8], at: usize, dest_format: Ww3dFormat, argb: u32) {
    let _ = write_b8g8r8a8(dest, at, dest_format, argb);
}

/// `BitmapHandlerClass::Copy_Pixel(dest_ptr, dest_format, src_ptr, src_format, palette, palette_bpp)`
/// (bitmaphandler.h): same format copies the pixel (the 16-bit formats **swap their channel nibbles** as upstream
/// writes them: `A4R4G4B4` reverses its four nibbles, `A1R5G5B5` and `R5G6B5` swap red and blue), different
/// formats go through B8G8R8A8.
pub fn copy_pixel(dest: &mut [u8], dest_at: usize, dest_format: Ww3dFormat, src: &[u8], src_at: usize, src_format: Ww3dFormat, palette: &[u8], palette_bpp: u32) {
    use Ww3dFormat::*;
    if dest_format == src_format {
        match dest_format {
            A8R8G8B8 | X8R8G8B8 => wr32(dest, dest_at, rd32(src, src_at)),
            R8G8B8 => {
                for i in 0..3 {
                    if let Some(d) = dest.get_mut(dest_at + i) {
                        *d = byte_at(src, (src_at + i) as isize);
                    }
                }
            }
            A4R4G4B4 => {
                let tmp = rd16(src, src_at);
                wr16(dest, dest_at, ((tmp & 0x000f) << 12) | ((tmp & 0x00f0) << 4) | ((tmp & 0x0f00) >> 4) | ((tmp & 0xf000) >> 12));
            }
            A1R5G5B5 => {
                let tmp = rd16(src, src_at);
                wr16(dest, dest_at, ((tmp & 0x001f) << 11) | ((tmp & 0x03e0) << 1) | ((tmp & 0x7c00) >> 9) | ((tmp & 0x8000) >> 15));
            }
            R5G6B5 => {
                let tmp = rd16(src, src_at);
                wr16(dest, dest_at, ((tmp & 0x001f) << 11) | (tmp & 0x07e0) | ((tmp & 0xf800) >> 11));
            }
            R3G3B2 | L8 | A8 => {
                if let Some(d) = dest.get_mut(dest_at) {
                    *d = byte_at(src, src_at as isize);
                }
            }
            _ => bitmap_assert(false), // P8: paletted destinations not supported
        }
    } else {
        let mut b8g8r8a8: u32 = 0;
        read_b8g8r8a8(&mut b8g8r8a8, src, src_at as isize, src_format, palette, palette_bpp);
        write_b8g8r8a8_word(dest, dest_at, dest_format, b8g8r8a8);
    }
}

/// `Copy_Pixel(..., const Vector3 &hsv_shift)` (bitmaphandler.h): via B8G8R8A8, recoloured.
pub fn copy_pixel_hsv(dest: &mut [u8], dest_at: usize, dest_format: Ww3dFormat, src: &[u8], src_at: usize, src_format: Ww3dFormat, palette: &[u8], palette_bpp: u32, hsv_shift: [f32; 3]) {
    let mut b8g8r8a8: u32 = 0;
    read_b8g8r8a8(&mut b8g8r8a8, src, src_at as isize, src_format, palette, palette_bpp);
    recolor_in_place(&mut b8g8r8a8, hsv_shift);
    write_b8g8r8a8_word(dest, dest_at, dest_format, b8g8r8a8);
}

/// `BitmapHandlerClass::Combine_A8R8G8B8` (bitmaphandler.h): the per-channel average of four pixels, each
/// channel quartered (low two bits dropped) before the sum.
pub fn combine_a8r8g8b8(mut bgra1: u32, mut bgra2: u32, mut bgra3: u32, mut bgra4: u32) -> u32 {
    bgra1 &= 0xfcfcfcfc;
    bgra2 &= 0xfcfcfcfc;
    bgra3 &= 0xfcfcfcfc;
    bgra4 &= 0xfcfcfcfc;
    bgra1 >>= 2;
    bgra2 >>= 2;
    bgra3 >>= 2;
    bgra4 >>= 2;
    bgra1 = bgra1.wrapping_add(bgra2);
    bgra3 = bgra3.wrapping_add(bgra4);
    bgra1.wrapping_add(bgra3)
}

/// `BitmapHandlerClass::Create_Mipmap_B8G8R8A8` (bitmaphandler.cpp:34): one mip level of a 32-bit surface; every
/// second row and column of the source, each output pixel the [`combine_a8r8g8b8`] of the 2x2 block.
pub fn create_mipmap_b8g8r8a8(dest_surface: &mut [u8], dest_surface_pitch: u32, src_surface: &[u8], src_surface_pitch: u32, width: u32, height: u32) {
    let mut dest_row = 0usize;
    let mut src_row = 0usize;
    let mut y = 0;
    while y < height {
        let mut dest = dest_row;
        let mut src = src_row;
        dest_row += dest_surface_pitch as usize;
        src_row += src_surface_pitch as usize;
        let mut x = 0;
        while x < width {
            let bgra3 = rd32(src_surface, src + src_surface_pitch as usize);
            let bgra1 = rd32(src_surface, src);
            src += 4;
            let bgra4 = rd32(src_surface, src + src_surface_pitch as usize);
            let bgra2 = rd32(src_surface, src);
            src += 4;
            wr32(dest_surface, dest, combine_a8r8g8b8(bgra1, bgra2, bgra3, bgra4));
            dest += 4;
            x += 2;
        }
        y += 2;
    }
}

/// `BitmapHandlerClass::Copy_Image_Generate_Mipmap` (bitmaphandler.cpp:57): copies the image to `dest` and
/// writes its next mip level to `mip`, optionally recolouring the mip level only (the copy is not recoloured).
/// Pitches are in bytes. Palettised sources are not supported.
pub fn copy_image_generate_mipmap(
    width: u32,
    height: u32,
    dest_surface: &mut [u8],
    dest_pitch: u32,
    dest_format: Ww3dFormat,
    src_surface: &[u8],
    src_pitch: u32,
    src_format: Ww3dFormat,
    mip_surface: &mut [u8],
    mip_pitch: u32,
    hsv_shift: [f32; 3],
) {
    let has_hsv_shift = hsv_shift[0] != 0.0 || hsv_shift[1] != 0.0 || hsv_shift[2] != 0.0;

    if src_format == dest_format && src_format == Ww3dFormat::A8R8G8B8 {
        let dest_pitch = (dest_pitch / 4) as usize;
        let src_pitch = (src_pitch / 4) as usize;
        let mip_pitch = (mip_pitch / 4) as usize;
        for y in 0..(height / 2) as usize {
            let mut dest_ptr = 2 * y * dest_pitch;
            let mut src_ptr = y * 2 * src_pitch;
            let mut mip_ptr = y * mip_pitch;
            for _x in 0..width / 2 {
                let b8g8r8a8_10 = rd32(src_surface, (src_ptr + src_pitch) * 4);
                wr32(dest_surface, (dest_ptr + dest_pitch) * 4, b8g8r8a8_10);
                let b8g8r8a8_00 = rd32(src_surface, src_ptr * 4);
                src_ptr += 1;
                wr32(dest_surface, dest_ptr * 4, b8g8r8a8_00);
                dest_ptr += 1;

                let b8g8r8a8_11 = rd32(src_surface, (src_ptr + src_pitch) * 4);
                wr32(dest_surface, (dest_ptr + dest_pitch) * 4, b8g8r8a8_11);
                let b8g8r8a8_01 = rd32(src_surface, src_ptr * 4);
                src_ptr += 1;
                wr32(dest_surface, dest_ptr * 4, b8g8r8a8_01);
                dest_ptr += 1;

                let mut b8g8r8a8 = combine_a8r8g8b8(b8g8r8a8_00, b8g8r8a8_01, b8g8r8a8_10, b8g8r8a8_11);
                if has_hsv_shift {
                    recolor_in_place(&mut b8g8r8a8, hsv_shift);
                }
                wr32(mip_surface, mip_ptr * 4, b8g8r8a8);
                mip_ptr += 1;
            }
        }
        return;
    }

    bitmap_assert(src_format != Ww3dFormat::P8); // This function doesn't support paletted formats

    let src_bpp = bytes_per_pixel(src_format) as usize;
    let dest_bpp = bytes_per_pixel(dest_format) as usize;
    let no_palette: [u8; 0] = [];

    for y in 0..(height / 2) as usize {
        let mut dest_ptr = 2 * y * dest_pitch as usize;
        let mut src_ptr = y * 2 * src_pitch as usize;
        let mut mip_ptr = y * mip_pitch as usize;
        for _x in 0..width / 2 {
            let (mut b00, mut b01, mut b10, mut b11) = (0u32, 0u32, 0u32, 0u32);
            read_b8g8r8a8(&mut b00, src_surface, src_ptr as isize, src_format, &no_palette, 0);
            write_b8g8r8a8_word(dest_surface, dest_ptr, dest_format, b00);
            read_b8g8r8a8(&mut b01, src_surface, (src_ptr + src_bpp) as isize, src_format, &no_palette, 0);
            write_b8g8r8a8_word(dest_surface, dest_ptr + dest_bpp, dest_format, b01);
            read_b8g8r8a8(&mut b10, src_surface, (src_ptr + src_pitch as usize) as isize, src_format, &no_palette, 0);
            write_b8g8r8a8_word(dest_surface, dest_ptr + dest_pitch as usize, dest_format, b10);
            read_b8g8r8a8(&mut b11, src_surface, (src_ptr + src_bpp + src_pitch as usize) as isize, src_format, &no_palette, 0);
            write_b8g8r8a8_word(dest_surface, dest_ptr + dest_bpp + dest_pitch as usize, dest_format, b11);

            let mut b8g8r8a8 = combine_a8r8g8b8(b00, b01, b10, b11);
            if has_hsv_shift {
                recolor_in_place(&mut b8g8r8a8, hsv_shift);
            }
            write_b8g8r8a8_word(mip_surface, mip_ptr, dest_format, b8g8r8a8);

            dest_ptr += dest_bpp * 2;
            src_ptr += src_bpp * 2;
            mip_ptr += dest_bpp;
        }
    }
}

/// `BitmapHandlerClass::Copy_Image` (bitmaphandler.cpp:154): converts and/or scales an image. Three cases:
/// bump-map destinations (`U8V8`, `L6V5U5`, `X8L8V8U8`: a height map's slopes); 32-bit to the same 32-bit format
/// (straight copy, nearest-neighbour scale, or copy plus in-place mip level); everything else per pixel.
/// `src_surface` is `&mut` because the mip level is written into it (see the module comment).
pub fn copy_image(
    dest_surface: &mut [u8],
    dest_surface_width: u32,
    dest_surface_height: u32,
    dest_surface_pitch: u32,
    dest_surface_format: Ww3dFormat,
    src_surface: &mut [u8],
    src_surface_width: u32,
    src_surface_height: u32,
    src_surface_pitch: u32,
    src_surface_format: Ww3dFormat,
    src_palette: &[u8],
    src_palette_bpp: u32,
    generate_mip_level: bool,
    hsv_shift: [f32; 3],
) {
    assert!(dest_surface_width != 0);
    assert!(dest_surface_height != 0);

    if dest_surface_format == Ww3dFormat::U8V8 || dest_surface_format == Ww3dFormat::L6V5U5 || dest_surface_format == Ww3dFormat::X8L8V8U8 {
        let src_bpp = bytes_per_pixel(src_surface_format) as isize;
        let no_palette: [u8; 0] = [];

        for y in 0..dest_surface_height {
            let mut dest_ptr = (y * dest_surface_pitch) as usize;
            let mut src_ptr_mid = y as isize * src_surface_pitch as isize;
            let mut src_ptr_next_line = src_ptr_mid + src_surface_pitch as isize;
            let mut src_ptr_prev_line = src_ptr_mid - src_surface_pitch as isize;

            if y == src_surface_height.wrapping_sub(1) {
                // Don't go past the last line
                src_ptr_next_line = src_ptr_mid;
            }
            if y == 0 {
                // Don't go before first line
                src_ptr_prev_line = src_ptr_mid;
            }

            for _x in 0..dest_surface_width {
                let (mut pixel00, mut pixel01, mut pixel_m1, mut pixel10, mut pixel1m) = (0u32, 0u32, 0u32, 0u32, 0u32);
                read_b8g8r8a8(&mut pixel00, src_surface, src_ptr_mid, src_surface_format, &no_palette, 0);
                read_b8g8r8a8(&mut pixel01, src_surface, src_ptr_mid + src_bpp, src_surface_format, &no_palette, 0);
                read_b8g8r8a8(&mut pixel_m1, src_surface, src_ptr_mid - src_bpp, src_surface_format, &no_palette, 0);
                read_b8g8r8a8(&mut pixel10, src_surface, src_ptr_prev_line, src_surface_format, &no_palette, 0);
                read_b8g8r8a8(&mut pixel1m, src_surface, src_ptr_next_line, src_surface_format, &no_palette, 0);

                let (mut bv00, mut bv01, mut bv_m1, mut bv10, mut bv1m) = ([0u8; 1], [0u8; 1], [0u8; 1], [0u8; 1], [0u8; 1]);
                write_b8g8r8a8_word(&mut bv00, 0, Ww3dFormat::L8, pixel00);
                write_b8g8r8a8_word(&mut bv01, 0, Ww3dFormat::L8, pixel01);
                write_b8g8r8a8_word(&mut bv_m1, 0, Ww3dFormat::L8, pixel_m1);
                write_b8g8r8a8_word(&mut bv10, 0, Ww3dFormat::L8, pixel10);
                write_b8g8r8a8_word(&mut bv1m, 0, Ww3dFormat::L8, pixel1m);

                let (v00, v01, v_m1, v10, v1m) = (i32::from(bv00[0]), i32::from(bv01[0]), i32::from(bv_m1[0]), i32::from(bv10[0]), i32::from(bv1m[0]));

                let mut i_du = v_m1 - v01; // The delta-u bump value
                let i_dv = v1m - v10; // The delta-v bump value

                if v00 < v_m1 && v00 < v01 {
                    // If we are at valley
                    i_du = v_m1 - v00; // Choose greater of 1st order diffs
                    if i_du < v00 - v01 {
                        i_du = v00 - v01;
                    }
                }

                let u_l: u16 = if v00 > 1 { 63 } else { 127 };

                match dest_surface_format {
                    Ww3dFormat::U8V8 => {
                        dest_surface[dest_ptr] = i_du as u8;
                        dest_surface[dest_ptr + 1] = i_dv as u8;
                        dest_ptr += 2;
                    }
                    Ww3dFormat::L6V5U5 => {
                        let mut v: u16 = (((i_du >> 3) & 0x1f) << 0) as u16;
                        v |= (((i_dv >> 3) & 0x1f) << 5) as u16;
                        v |= (((u_l >> 2) & 0x3f) << 10) as u16;
                        wr16(dest_surface, dest_ptr, v);
                        dest_ptr += 2;
                    }
                    Ww3dFormat::X8L8V8U8 => {
                        dest_surface[dest_ptr] = i_du as u8;
                        dest_surface[dest_ptr + 1] = i_dv as u8;
                        dest_surface[dest_ptr + 2] = u_l as u8;
                        dest_surface[dest_ptr + 3] = 0;
                        dest_ptr += 4;
                    }
                    _ => bitmap_assert(false), // Unknown bumpmap format
                }

                src_ptr_mid += src_bpp;
                src_ptr_prev_line += src_bpp;
                src_ptr_next_line += src_bpp;
            }
        }
        return;
    }

    let has_hsv_shift = hsv_shift[0] != 0.0 || hsv_shift[1] != 0.0 || hsv_shift[2] != 0.0;

    if src_surface_format == dest_surface_format && (src_surface_format == Ww3dFormat::A8R8G8B8 || src_surface_format == Ww3dFormat::X8R8G8B8) {
        let dest_pitch = (dest_surface_pitch / 4) as usize;
        let src_pitch = (src_surface_pitch / 4) as usize;

        if dest_surface_width == src_surface_width && dest_surface_height == src_surface_height {
            if generate_mip_level {
                if dest_surface_width == 1 {
                    let mut b8g8r8a8 = rd32(src_surface, 0);
                    if has_hsv_shift {
                        recolor_in_place(&mut b8g8r8a8, hsv_shift);
                    }
                    wr32(dest_surface, 0, b8g8r8a8);
                } else {
                    for y in 0..(dest_surface_height / 2) as usize {
                        let mut dest_ptr = 2 * y * dest_pitch;
                        let mut src_ptr = y * 2 * src_pitch;
                        let mut mip_ptr = y * src_pitch;

                        for _x in 0..dest_surface_width / 2 {
                            let mut b10 = rd32(src_surface, (src_ptr + src_pitch) * 4);
                            let mut b00 = rd32(src_surface, src_ptr * 4);
                            src_ptr += 1;
                            let mut b11 = rd32(src_surface, (src_ptr + src_pitch) * 4);
                            let mut b01 = rd32(src_surface, src_ptr * 4);
                            src_ptr += 1;

                            if has_hsv_shift {
                                recolor_in_place(&mut b00, hsv_shift);
                                recolor_in_place(&mut b01, hsv_shift);
                                recolor_in_place(&mut b10, hsv_shift);
                                recolor_in_place(&mut b11, hsv_shift);
                            }

                            wr32(dest_surface, (dest_ptr + dest_pitch) * 4, b10);
                            wr32(dest_surface, dest_ptr * 4, b00);
                            dest_ptr += 1;
                            wr32(dest_surface, (dest_ptr + dest_pitch) * 4, b11);
                            wr32(dest_surface, dest_ptr * 4, b01);
                            dest_ptr += 1;

                            wr32(src_surface, mip_ptr * 4, combine_a8r8g8b8(b00, b01, b10, b11));
                            mip_ptr += 1;
                        }
                    }
                }
            } else {
                for y in 0..dest_surface_height as usize {
                    let mut dest_ptr = y * dest_pitch;
                    let mut src_ptr = y * src_pitch;
                    for _x in 0..dest_surface_width {
                        let mut b8g8r8a8 = rd32(src_surface, src_ptr * 4);
                        src_ptr += 1;
                        if has_hsv_shift {
                            recolor_in_place(&mut b8g8r8a8, hsv_shift);
                        }
                        wr32(dest_surface, dest_ptr * 4, b8g8r8a8);
                        dest_ptr += 1;
                    }
                }
            }
        } else {
            for y in 0..dest_surface_height {
                let mut dest_ptr = y as usize * dest_pitch;
                let src_y = y * src_surface_height / dest_surface_height;
                let src_ptr = src_y as usize * src_pitch;
                for x in 0..dest_surface_width {
                    let src_x = x * src_surface_width / dest_surface_width;
                    let mut b8g8r8a8 = rd32(src_surface, (src_ptr + src_x as usize) * 4);
                    if has_hsv_shift {
                        recolor_in_place(&mut b8g8r8a8, hsv_shift);
                    }
                    wr32(dest_surface, dest_ptr * 4, b8g8r8a8);
                    dest_ptr += 1;
                }
            }
        }
        return;
    }

    let dest_bpp = bytes_per_pixel(dest_surface_format) as usize;
    let src_bpp = bytes_per_pixel(src_surface_format) as usize;

    if dest_surface_width == src_surface_width && dest_surface_height == src_surface_height {
        if generate_mip_level {
            bitmap_assert(src_surface_format != Ww3dFormat::P8); // Paletted textures can't be mipmapped
            if dest_surface_width == 1 {
                let mut b8g8r8a8: u32 = 0;
                read_b8g8r8a8(&mut b8g8r8a8, src_surface, 0, src_surface_format, src_palette, src_palette_bpp);
                if has_hsv_shift {
                    recolor_in_place(&mut b8g8r8a8, hsv_shift);
                }
                write_b8g8r8a8_word(dest_surface, 0, dest_surface_format, b8g8r8a8);
            } else {
                for y in 0..(dest_surface_height / 2) as usize {
                    let mut dest_ptr = 2 * y * dest_surface_pitch as usize;
                    let mut src_ptr = y * 2 * src_surface_pitch as usize;
                    let mut mip_ptr = y * src_surface_pitch as usize;

                    for _x in 0..dest_surface_width / 2 {
                        let (mut b00, mut b01, mut b10, mut b11) = (0u32, 0u32, 0u32, 0u32);
                        read_b8g8r8a8(&mut b00, src_surface, src_ptr as isize, src_surface_format, src_palette, src_palette_bpp);
                        read_b8g8r8a8(&mut b01, src_surface, (src_ptr + src_bpp) as isize, src_surface_format, src_palette, src_palette_bpp);
                        read_b8g8r8a8(&mut b10, src_surface, (src_ptr + src_surface_pitch as usize) as isize, src_surface_format, src_palette, src_palette_bpp);
                        read_b8g8r8a8(&mut b11, src_surface, (src_ptr + src_bpp + src_surface_pitch as usize) as isize, src_surface_format, src_palette, src_palette_bpp);

                        if has_hsv_shift {
                            recolor_in_place(&mut b00, hsv_shift);
                            recolor_in_place(&mut b01, hsv_shift);
                            recolor_in_place(&mut b10, hsv_shift);
                            recolor_in_place(&mut b11, hsv_shift);
                        }

                        write_b8g8r8a8_word(dest_surface, dest_ptr, dest_surface_format, b00);
                        write_b8g8r8a8_word(dest_surface, dest_ptr + dest_bpp, dest_surface_format, b01);
                        write_b8g8r8a8_word(dest_surface, dest_ptr + dest_surface_pitch as usize, dest_surface_format, b10);
                        write_b8g8r8a8_word(dest_surface, dest_ptr + dest_bpp + dest_surface_pitch as usize, dest_surface_format, b11);

                        let b8g8r8a8 = combine_a8r8g8b8(b00, b01, b10, b11);
                        write_b8g8r8a8_word(src_surface, mip_ptr, src_surface_format, b8g8r8a8);

                        dest_ptr += dest_bpp * 2;
                        src_ptr += src_bpp * 2;
                        mip_ptr += src_bpp;
                    }
                }
            }
        } else {
            for y in 0..dest_surface_height as usize {
                let mut dest_ptr = y * dest_surface_pitch as usize;
                let mut src_ptr = y * src_surface_pitch as usize;
                for _x in 0..dest_surface_width {
                    if has_hsv_shift {
                        copy_pixel_hsv(dest_surface, dest_ptr, dest_surface_format, src_surface, src_ptr, src_surface_format, src_palette, src_palette_bpp, hsv_shift);
                    } else {
                        copy_pixel(dest_surface, dest_ptr, dest_surface_format, src_surface, src_ptr, src_surface_format, src_palette, src_palette_bpp);
                    }
                    dest_ptr += dest_bpp;
                    src_ptr += src_bpp;
                }
            }
        }
    } else {
        for y in 0..dest_surface_height {
            let mut dest_ptr = y as usize * dest_surface_pitch as usize;
            let src_y = y * src_surface_height / dest_surface_height;
            let src_ptr = src_y as usize * src_surface_pitch as usize;
            for x in 0..dest_surface_width {
                let src_x = (x * src_surface_width / dest_surface_width) as usize * src_bpp;
                if has_hsv_shift {
                    copy_pixel_hsv(dest_surface, dest_ptr, dest_surface_format, src_surface, src_ptr + src_x, src_surface_format, src_palette, src_palette_bpp, hsv_shift);
                } else {
                    copy_pixel(dest_surface, dest_ptr, dest_surface_format, src_surface, src_ptr + src_x, src_surface_format, src_palette, src_palette_bpp);
                }
                dest_ptr += dest_bpp;
            }
        }
    }
}
