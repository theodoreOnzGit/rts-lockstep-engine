//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/surfaceclass.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/surfaceclass.h
//!   (`Has_Alpha`, `Alpha_Bits` from ww3dformat.h)
//!
//! Upstream wraps an `IDirect3DSurface8`. Here the surface is a CPU pixel buffer (`pixels`, tightly packed,
//! `pitch = width * PixelSize`) so the pixel-level members (`Clear`, `Copy` from bytes, `CreateCopy`, `FindBB`,
//! `Is_Transparent_Column`, `Get_Pixel`, `DrawPixel`, `DrawHLine`, `Is_Monochrome`, `Hue_Shift`) translate
//! exactly. Device-only, not translated: `_Create_DX8_Surface(filename)` (file loading: the caller supplies the
//! decoded pixels), `Attach`/`Detach` (D3D refcounts), `Stretch_Copy` (`D3DXLoadSurfaceFromSurface` with
//! `D3DX_FILTER_TRIANGLE`, a D3DX filter), and the format-converting and resizing `Copy`
//! (`D3DXLoadSurfaceFromSurface` with `D3DX_FILTER_NONE`); [`SurfaceClass::copy_from_surface`] implements the same
//! format-and-size path (`_Copy_DX8_Rects`, a rectangle copy) and, for other cases, a per-pixel ARGB conversion
//! for the formats [`read_argb`] supports. That fallback is NOT D3DX and is flagged for the differential pass.

use crate::formats::texture::colorspace::recolor_rgb;
use crate::formats::texture::format::Ww3dFormat;
use crate::ww::vector3::Vector3;

/// `Has_Alpha(WW3DFormat)` (ww3dformat.h:126).
pub fn has_alpha(format: Ww3dFormat) -> bool {
    use Ww3dFormat::*;
    matches!(format, A8R8G8B8 | A1R5G5B5 | A4R4G4B4 | A8 | A8R3G3B2 | A8P8 | A8L8 | A4L4 | Dxt2 | Dxt3 | Dxt4 | Dxt5)
}

/// `Alpha_Bits(WW3DFormat)` (ww3dformat.h:148).
pub fn alpha_bits(format: Ww3dFormat) -> i32 {
    use Ww3dFormat::*;
    match format {
        A8R8G8B8 | A8 | A8R3G3B2 | A8P8 | A8L8 => 8,
        A4R4G4B4 | A4L4 | Dxt3 | Dxt4 | Dxt5 => 4,
        A1R5G5B5 | Dxt2 => 1,
        _ => 0,
    }
}

/// `SurfaceClass::SurfaceDescription` (surfaceclass.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceDescription {
    pub format: Ww3dFormat,
    pub width: u32,
    pub height: u32,
}

/// `PixelSize(sd)` (surfaceclass.cpp:75): bytes per pixel; 0 for formats not listed (compressed, bump, unknown).
pub fn pixel_size(sd: &SurfaceDescription) -> u32 {
    use Ww3dFormat::*;
    match sd.format {
        A8R8G8B8 | X8R8G8B8 => 4,
        R8G8B8 => 3,
        R5G6B5 | X1R5G5B5 | A1R5G5B5 | A4R4G4B4 | A8R3G3B2 | X4R4G4B4 | A8P8 | A8L8 => 2,
        R3G3B2 | A8 | P8 | L8 | A4L4 => 1,
        _ => 0,
    }
}

/// `Convert_Pixel(Vector3 &rgb, sd, const unsigned char *pixel)` (surfaceclass.cpp:110): the pixel to 0..1 RGB.
/// Unlisted formats `WWASSERT(0)`; the unmodified (0,0,0) scaled is returned for them.
pub fn convert_pixel_read(sd: &SurfaceDescription, pixel: &[u8]) -> Vector3 {
    use Ww3dFormat::*;
    let scale = 1.0f32 / 255.0f32;
    let mut rgb = Vector3::new(0.0, 0.0, 0.0);
    match sd.format {
        A8R8G8B8 | X8R8G8B8 | R8G8B8 => {
            rgb.x = pixel[2] as f32; // R
            rgb.y = pixel[1] as f32; // G
            rgb.z = pixel[0] as f32; // B
        }
        A4R4G4B4 => {
            let tmp = u16::from_le_bytes([pixel[0], pixel[1]]);
            rgb.x = ((tmp & 0x0f00) >> 4) as f32;
            rgb.y = (tmp & 0x00f0) as f32;
            rgb.z = ((tmp & 0x000f) << 4) as f32;
        }
        A1R5G5B5 => {
            let tmp = u16::from_le_bytes([pixel[0], pixel[1]]);
            rgb.x = ((tmp >> 7) & 0xf8) as f32;
            rgb.y = ((tmp >> 2) & 0xf8) as f32;
            rgb.z = ((tmp << 3) & 0xf8) as f32;
        }
        R5G6B5 => {
            let tmp = u16::from_le_bytes([pixel[0], pixel[1]]);
            rgb.x = ((tmp >> 8) & 0xf8) as f32;
            rgb.y = ((tmp >> 3) & 0xfc) as f32;
            rgb.z = ((tmp << 3) & 0xf8) as f32;
        }
        _ => debug_assert!(false),
    }
    rgb *= scale;
    rgb
}

/// `Convert_Pixel(unsigned char *pixel, sd, const Vector3 &rgb)` (surfaceclass.cpp:160): writes RGB, keeping the
/// alpha bits of the 16-bit alpha formats.
pub fn convert_pixel_write(pixel: &mut [u8], sd: &SurfaceDescription, rgb: &Vector3) {
    use Ww3dFormat::*;
    let r = (rgb.x * 255.0f32) as u8;
    let g = (rgb.y * 255.0f32) as u8;
    let b = (rgb.z * 255.0f32) as u8;
    match sd.format {
        A8R8G8B8 | X8R8G8B8 | R8G8B8 => {
            pixel[0] = b;
            pixel[1] = g;
            pixel[2] = r;
        }
        A4R4G4B4 => {
            let mut tmp = u16::from_le_bytes([pixel[0], pixel[1]]);
            tmp &= 0xF000;
            tmp |= ((r & 0xF0) as u16) << 4;
            tmp |= (g & 0xF0) as u16;
            tmp |= ((b & 0xF0) as u16) >> 4;
            pixel[..2].copy_from_slice(&tmp.to_le_bytes());
        }
        A1R5G5B5 => {
            let mut tmp = u16::from_le_bytes([pixel[0], pixel[1]]);
            tmp &= 0x8000;
            tmp |= ((r & 0xF8) as u16) << 7;
            tmp |= ((g & 0xF8) as u16) << 2;
            tmp |= ((b & 0xF8) as u16) >> 3;
            pixel[..2].copy_from_slice(&tmp.to_le_bytes());
        }
        R5G6B5 => {
            let mut tmp = ((r & 0xf8) as u16) << 8;
            tmp |= ((g & 0xfc) as u16) << 3;
            tmp |= ((b & 0xf8) as u16) >> 3;
            pixel[..2].copy_from_slice(&tmp.to_le_bytes());
        }
        _ => debug_assert!(false),
    }
}

/// Reads a pixel as 8-bit-per-channel ARGB (for the non-D3DX fallback copy), `None` for unsupported formats.
pub fn read_argb(format: Ww3dFormat, p: &[u8]) -> Option<u32> {
    use Ww3dFormat::*;
    Some(match format {
        A8R8G8B8 => u32::from_le_bytes([p[0], p[1], p[2], p[3]]),
        X8R8G8B8 => u32::from_le_bytes([p[0], p[1], p[2], 0xFF]),
        R8G8B8 => u32::from_le_bytes([p[0], p[1], p[2], 0xFF]),
        A4R4G4B4 => {
            let t = u16::from_le_bytes([p[0], p[1]]) as u32;
            let (a, r, g, b) = ((t >> 12) & 0xF, (t >> 8) & 0xF, (t >> 4) & 0xF, t & 0xF);
            (a * 17) << 24 | (r * 17) << 16 | (g * 17) << 8 | (b * 17)
        }
        A1R5G5B5 => {
            let t = u16::from_le_bytes([p[0], p[1]]) as u32;
            let (a, r, g, b) = ((t >> 15) & 1, (t >> 10) & 0x1F, (t >> 5) & 0x1F, t & 0x1F);
            (a * 255) << 24 | ((r << 3) | (r >> 2)) << 16 | ((g << 3) | (g >> 2)) << 8 | ((b << 3) | (b >> 2))
        }
        R5G6B5 => {
            let t = u16::from_le_bytes([p[0], p[1]]) as u32;
            let (r, g, b) = ((t >> 11) & 0x1F, (t >> 5) & 0x3F, t & 0x1F);
            0xFF00_0000 | ((r << 3) | (r >> 2)) << 16 | ((g << 2) | (g >> 4)) << 8 | ((b << 3) | (b >> 2))
        }
        _ => return None,
    })
}

/// Writes ARGB into a pixel of `format` (the inverse of [`read_argb`], truncating).
pub fn write_argb(format: Ww3dFormat, p: &mut [u8], argb: u32) -> bool {
    use Ww3dFormat::*;
    let (a, r, g, b) = ((argb >> 24) & 0xFF, (argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF);
    match format {
        A8R8G8B8 | X8R8G8B8 => p[..4].copy_from_slice(&argb.to_le_bytes()),
        R8G8B8 => {
            p[0] = b as u8;
            p[1] = g as u8;
            p[2] = r as u8;
        }
        A4R4G4B4 => {
            let t = ((a >> 4) << 12) | ((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4);
            p[..2].copy_from_slice(&(t as u16).to_le_bytes());
        }
        A1R5G5B5 => {
            let t = ((a >> 7) << 15) | ((r >> 3) << 10) | ((g >> 3) << 5) | (b >> 3);
            p[..2].copy_from_slice(&(t as u16).to_le_bytes());
        }
        R5G6B5 => {
            let t = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);
            p[..2].copy_from_slice(&(t as u16).to_le_bytes());
        }
        _ => return false,
    }
    true
}

/// `class SurfaceClass` (surfaceclass.h) over a CPU pixel buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceClass {
    pub surface_format: Ww3dFormat,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl SurfaceClass {
    /// `SurfaceClass::SurfaceClass(width, height, format)` (surfaceclass.cpp:228): a zeroed surface.
    pub fn new(width: u32, height: u32, format: Ww3dFormat) -> Self {
        assert!(width != 0);
        assert!(height != 0);
        let sd = SurfaceDescription { format, width, height };
        let size = pixel_size(&sd) as usize;
        SurfaceClass { surface_format: format, width, height, pixels: vec![0; size * width as usize * height as usize] }
    }

    /// A surface over decoded pixels (what `_Create_DX8_Surface(filename)` + the file decoder produce).
    pub fn from_pixels(width: u32, height: u32, format: Ww3dFormat, pixels: Vec<u8>) -> Self {
        SurfaceClass { surface_format: format, width, height, pixels }
    }

    /// `SurfaceClass::Get_Description`.
    pub fn get_description(&self) -> SurfaceDescription {
        SurfaceDescription { format: self.surface_format, width: self.width, height: self.height }
    }

    fn pitch(&self) -> usize {
        pixel_size(&self.get_description()) as usize * self.width as usize
    }

    /// `SurfaceClass::Lock(&pitch)` / `Unlock`: the pixel buffer and its pitch.
    pub fn lock(&mut self) -> (&mut [u8], usize) {
        let pitch = self.pitch();
        (&mut self.pixels, pitch)
    }

    /// `SurfaceClass::Clear` (surfaceclass.cpp:289): zero every pixel.
    pub fn clear(&mut self) {
        for b in self.pixels.iter_mut() {
            *b = 0;
        }
    }

    /// `SurfaceClass::Copy(const unsigned char *other)` (surfaceclass.cpp:328): whole surface from a packed array.
    pub fn copy_from_bytes(&mut self, other: &[u8]) {
        let sd = self.get_description();
        let size = pixel_size(&sd) as usize;
        let pitch = self.pitch();
        for i in 0..sd.height as usize {
            let n = size * sd.width as usize;
            self.pixels[i * pitch..i * pitch + n].copy_from_slice(&other[i * sd.width as usize * size..][..n]);
        }
    }

    /// `SurfaceClass::Copy(Vector2i &min, Vector2i &max, const unsigned char *other)` (surfaceclass.cpp:367): a
    /// sub-rectangle `[min, max)` from a full-surface packed array (read at the same coordinates). Upstream writes
    /// row `i - min.J` of the locked rectangle (the destination rectangle starts at `min`).
    pub fn copy_rect_from_bytes(&mut self, min: (i32, i32), max: (i32, i32), other: &[u8]) {
        let sd = self.get_description();
        let size = pixel_size(&sd) as usize;
        let pitch = self.pitch();
        let dx = (max.0 - min.0) as usize;
        let mut row = min.1;
        while row < max.1 {
            let dst = (row as usize) * pitch + min.0 as usize * size;
            let src = (row as usize * sd.width as usize + min.0 as usize) * size;
            self.pixels[dst..dst + size * dx].copy_from_slice(&other[src..src + size * dx]);
            row += 1;
        }
    }

    /// `SurfaceClass::CreateCopy(&width, &height, &size, flip)` (surfaceclass.cpp:412): `(pixels, width, height,
    /// pixel size)`, optionally flipped vertically.
    pub fn create_copy(&self, flip: bool) -> (Vec<u8>, u32, u32, u32) {
        let sd = self.get_description();
        let mysize = pixel_size(&sd);
        let mut other = vec![0u8; (sd.height * sd.width * mysize) as usize];
        let pitch = self.pitch();
        let row_bytes = (mysize * sd.width) as usize;
        for i in 0..sd.height as usize {
            let dst = if flip { (sd.height as usize - i - 1) * row_bytes } else { i * row_bytes };
            other[dst..dst + row_bytes].copy_from_slice(&self.pixels[i * pitch..i * pitch + row_bytes]);
        }
        (other, sd.width, sd.height, mysize)
    }

    /// `SurfaceClass::Copy(dstx, dsty, srcx, srcy, width, height, other)` (surfaceclass.cpp:465). Source rectangle
    /// is clamped to the source surface; with equal format AND equal dimensions upstream does a rectangle copy
    /// (`_Copy_DX8_Rects`), otherwise `D3DXLoadSurfaceFromSurface(.., D3DX_FILTER_NONE)` into the clamped
    /// destination rectangle (see the module note: the second path is a per-pixel stand-in here).
    pub fn copy_from_surface(&mut self, dstx: u32, dsty: u32, srcx: u32, srcy: u32, width: u32, height: u32, other: &SurfaceClass) {
        assert!(width != 0);
        assert!(height != 0);
        let sd = self.get_description();
        let osd = other.get_description();
        let mut src_right = (srcx + width) as i32;
        let mut src_bottom = (srcy + height) as i32;
        if src_right > osd.width as i32 {
            src_right = osd.width as i32;
        }
        if src_bottom > osd.height as i32 {
            src_bottom = osd.height as i32;
        }
        let size = pixel_size(&sd) as usize;
        let osize = pixel_size(&osd) as usize;
        let pitch = self.pitch();
        let opitch = other.pitch();

        if sd.format == osd.format && sd.width == osd.width && sd.height == osd.height {
            // _Copy_DX8_Rects: rectangle copy to (dstx, dsty)
            let w = (src_right - srcx as i32).max(0) as usize;
            let h = (src_bottom - srcy as i32).max(0) as usize;
            for y in 0..h {
                let s = (srcy as usize + y) * opitch + srcx as usize * osize;
                let d = (dsty as usize + y) * pitch + dstx as usize * size;
                self.pixels[d..d + w * size].copy_from_slice(&other.pixels[s..s + w * osize]);
            }
        } else {
            let mut dst_right = (dstx + width) as i32;
            let mut dst_bottom = (dsty + height) as i32;
            if dst_right > sd.width as i32 {
                dst_right = sd.width as i32;
            }
            if dst_bottom > sd.height as i32 {
                dst_bottom = sd.height as i32;
            }
            // D3DX_FILTER_NONE: point sampling of the source rectangle onto the destination rectangle.
            let sw = (src_right - srcx as i32).max(0);
            let sh = (src_bottom - srcy as i32).max(0);
            let dw = (dst_right - dstx as i32).max(0);
            let dh = (dst_bottom - dsty as i32).max(0);
            for y in 0..dh {
                for x in 0..dw {
                    let sx = srcx as i32 + if dw > 0 { x * sw / dw } else { 0 };
                    let sy = srcy as i32 + if dh > 0 { y * sh / dh } else { 0 };
                    let sp = sy as usize * opitch + sx as usize * osize;
                    if let Some(argb) = read_argb(osd.format, &other.pixels[sp..sp + osize]) {
                        let dp = (dsty as i32 + y) as usize * pitch + (dstx as i32 + x) as usize * size;
                        write_argb(sd.format, &mut self.pixels[dp..dp + size], argb);
                    }
                }
            }
        }
    }

    // SurfaceClass::Stretch_Copy: D3DXLoadSurfaceFromSurface with D3DX_FILTER_TRIANGLE — device library, not
    // translated.

    fn alpha_mask(alphabits: i32) -> i32 {
        match alphabits {
            1 => 1,
            4 => 0xf,
            8 => 0xff,
            _ => 0,
        }
    }

    /// `SurfaceClass::FindBB(min, max)` (surfaceclass.cpp:566): the bounding box of pixels with non-zero alpha in
    /// `[min, max)`; both are in/out as upstream. The alpha is the last byte of the pixel.
    pub fn find_bb(&self, min: &mut (i32, i32), max: &mut (i32, i32)) {
        let sd = self.get_description();
        debug_assert!(has_alpha(sd.format));
        let alphabits = alpha_bits(sd.format);
        let mask = Self::alpha_mask(alphabits);
        let size = pixel_size(&sd) as usize;
        let pitch = self.pitch();
        let mut realmin = *max;
        let mut realmax = *min;
        for y in min.1..max.1 {
            for x in min.0..max.0 {
                let off = y as usize * pitch + x as usize * size;
                let mut myalpha = self.pixels[off + size - 1] as i32;
                myalpha = (myalpha >> (8 - alphabits)) & mask;
                if myalpha != 0 {
                    realmin.0 = realmin.0.min(x);
                    realmax.0 = realmax.0.max(x);
                    realmin.1 = realmin.1.min(y);
                    realmax.1 = realmax.1.max(y);
                }
            }
        }
        *max = realmax;
        *min = realmin;
    }

    /// `SurfaceClass::Is_Transparent_Column` (surfaceclass.cpp:641).
    pub fn is_transparent_column(&self, column: u32) -> bool {
        let sd = self.get_description();
        assert!(column < sd.width);
        debug_assert!(has_alpha(sd.format));
        let alphabits = alpha_bits(sd.format);
        let mask = Self::alpha_mask(alphabits);
        let size = pixel_size(&sd) as usize;
        let pitch = self.pitch();
        for y in 0..sd.height as usize {
            let off = y * pitch + column as usize * size;
            let mut myalpha = self.pixels[off + size - 1] as i32;
            myalpha = (myalpha >> (8 - alphabits)) & mask;
            if myalpha != 0 {
                return false;
            }
        }
        true
    }

    /// `SurfaceClass::Get_Pixel(rgb, x, y)` (surfaceclass.cpp:709): coordinates clamped to the last pixel.
    pub fn get_pixel(&self, x: i32, y: i32) -> Vector3 {
        let sd = self.get_description();
        let x = x.min(sd.width as i32 - 1);
        let y = y.min(sd.height as i32 - 1);
        let size = pixel_size(&sd) as usize;
        let off = y as usize * self.pitch() + x as usize * size;
        convert_pixel_read(&sd, &self.pixels[off..off + size.max(3)])
    }

    /// `SurfaceClass::DrawPixel(x, y, color)` (surfaceclass.cpp:806): the low `size` bytes of `color`.
    pub fn draw_pixel(&mut self, x: u32, y: u32, color: u32) {
        let sd = self.get_description();
        let size = pixel_size(&sd) as usize;
        let off = y as usize * self.pitch() + x as usize * size;
        match size {
            1 => self.pixels[off] = (color & 0xFF) as u8,
            2 => self.pixels[off..off + 2].copy_from_slice(&((color & 0xFFFF) as u16).to_le_bytes()),
            4 => self.pixels[off..off + 4].copy_from_slice(&color.to_le_bytes()),
            _ => {}
        }
    }

    /// `SurfaceClass::DrawHLine(y, x1, x2, color)` (surfaceclass.cpp:860): inclusive of `x2`.
    pub fn draw_hline(&mut self, y: u32, x1: u32, x2: u32, color: u32) {
        let mut x = x1;
        while x <= x2 {
            self.draw_pixel(x, y, color);
            x += 1;
        }
    }

    /// `SurfaceClass::Is_Monochrome` (surfaceclass.cpp:919). Compressed formats are converted to a valid
    /// uncompressed format first (`Get_Valid_Texture_Format`, device caps): not available here, so they return
    /// `None` (cannot decide).
    pub fn is_monochrome(&self) -> Option<bool> {
        use Ww3dFormat::*;
        let sd = self.get_description();
        match sd.format {
            A8L8 | A8 | L8 | A4L4 => return Some(true),
            Unknown | A8P8 | P8 | U8V8 | L6V5U5 | X8L8V8U8 => return Some(false),
            Dxt1 | Dxt2 | Dxt3 | Dxt4 | Dxt5 => return None,
            _ => {}
        }
        let size = pixel_size(&sd) as usize;
        let pitch = self.pitch();
        let mut mono = true;
        for y in 0..sd.height as usize {
            for x in 0..sd.width as usize {
                let off = y * pitch + x * size;
                let rgb = convert_pixel_read(&sd, &self.pixels[off..off + size.max(3)]);
                mono &= rgb.x == rgb.y;
                mono &= rgb.x == rgb.z;
                mono &= rgb.z == rgb.y;
                if !mono {
                    return Some(false);
                }
            }
        }
        Some(true)
    }

    /// `SurfaceClass::Hue_Shift(hsv_shift)` (surfaceclass.cpp:1009): `Recolor` each pixel, bounded to 0..1.
    pub fn hue_shift(&mut self, hsv_shift: &Vector3) {
        let sd = self.get_description();
        let size = pixel_size(&sd) as usize;
        let pitch = self.pitch();
        for y in 0..sd.height as usize {
            for x in 0..sd.width as usize {
                let off = y * pitch + x * size;
                let rgb = convert_pixel_read(&sd, &self.pixels[off..off + size.max(3)]);
                let mut a = [rgb.x, rgb.y, rgb.z];
                let _ = recolor_rgb(&mut a, [hsv_shift.x, hsv_shift.y, hsv_shift.z]);
                // Bound(x, 0.0f, 1.0f)
                let b = |v: f32| if v < 0.0 { 0.0 } else if v > 1.0 { 1.0 } else { v };
                convert_pixel_write(&mut self.pixels[off..off + size.max(3)], &sd, &Vector3::new(b(a[0]), b(a[1]), b(a[2])));
            }
        }
    }
}
