// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/TARGA.CPP, TARGA.H
//   (class Targa: Targa(), Open in read mode, Close, both Load functions,
//   XFlip, YFlip with its _swapBytes, SetPalette, IsCompressed,
//   GetExtension, DecodeImage, InvertImage; the header, footer and
//   extension structs)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, Targa.h/.cpp by Denzil E. Long, Jr., July 15, 1998)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Targa (`.tga`) images as WWLib's `Targa` class reads them, from an
//! in-memory byte buffer, into raw pixel and palette buffers.
//!
//! # The format, as upstream reads it
//!
//! ```text
//! TGAHeader (18 bytes, packed): IDLength, ColorMapType, ImageType (chars),
//!   CMapStart, CMapLength (shorts), CMapDepth (char), XOffset, YOffset,
//!   Width, Height (shorts), PixelDepth, ImageDescriptor (chars)
//! IDLength bytes skipped, the palette (CMapLength * (CMapDepth >> 3)
//!   bytes), the pixels (Width * Height * ((PixelDepth + 7) >> 3) bytes),
//!   raw (types 1, 2, 3) or run-length encoded (9, 10)
//! the last 26 bytes: TGA2Footer (Extension offset, Developer offset,
//!   "TRUEVISION-XFILE", '.', 0); a Targa 2.0 file with a non-zero
//!   Extension offset has a 495-byte TGA2Extension there
//! ```
//!
//! All the header fields are signed (`char` is signed to MSVC), and the size
//! arithmetic is 32-bit `long`. `Open` reads the footer, the extension and
//! the header, and leaves the file after the ID field. `Load(flags)`
//! allocates the image (and the palette if asked and the file has one) and
//! reads them; the game's texture loader opens the file first, toggles the
//! Y-origin bit, gives a 1024-byte palette of its own (`SetPalette`), then
//! calls `Load(name, TGAF_IMAGE, false)`.
//!
//! # Upstream quirks that are kept
//!
//! - **`YFlip` swaps only the first `stride & ~15` bytes of each row pair.**
//!   Its `_swapBytes` swaps 16-byte blocks correctly, but its byte loop for
//!   the rest (`mov al,[esi]; xchg al,[edi]; mov [edi],al`) puts back what
//!   it took: rows whose length in bytes is not a multiple of 16 keep their
//!   last `stride % 16` bytes where they were (a 4-pixel-wide 24-bit image
//!   is not flipped at all). The game toggles the Y-origin bit of every
//!   texture it loads, so bottom-up files are flipped this way.
//! - `Load(flags)` on a file that is already open (as the game's is) does
//!   not read the header again (`Open` returns at once), so changes to the
//!   header between `Open` and `Load` are what `Load` uses.
//! - Every error of `Open` inside `Load(flags)` is reported as
//!   `TGAERR_OPEN`.
//! - In `Open`, after a failed footer or extension read, the header is
//!   still read from where the file is (the `else` binds to the seek test).
//!   A failed extension read keeps the bytes it read and the Targa 2.0 flag.
//! - The palette is read to `palette + CMapStart * depth`: an allocated
//!   palette (`TGAF_PAL`) is only `CMapLength * depth` bytes, so a non-zero
//!   start writes past it (undefined), and the game's 1024-byte palette is
//!   overrun by any colour map larger than 1024 bytes.
//! - A negative palette size reads (or seeks) backwards; the run-length
//!   decoder takes its depth as an `unsigned char` and its pixel count as an
//!   `unsigned long`, so negative sizes become large ones.
//! - `InvertImage` (BGR to RGB, by reversing each pixel's bytes) does
//!   nothing at 16 bits or fewer.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`TgaUndefined`]: a read or write outside the image or palette buffer
//! (including a run-length packet that runs past the end of the image,
//! which upstream writes past its buffer), a signed overflow in a size
//! computation. Buffers upstream leaves uninitialised (`malloc`) are zeros
//! here; they are only partly written when a load fails.
//!
//! # What is left out
//!
//! - Writing: `Open` in `TGA_WRITEMODE`/`TGA_RDWRMODE`, `Save`,
//!   `EncodeImage`, `SetImage` (no game path reads into a caller's image).
//! - `Load(name, palette, image)` on caller buffers: here it runs only as
//!   `Load(flags)` calls it, on the Targa's own buffers.
//! - The file factory and names: the bytes are given (each `Open` of a
//!   closed file starts a fresh `GameFileClass` over them).

use crate::ww::chunkio::{GameFile, SeekFrom};

pub const TGA_NOIMAGE: i8 = 0;
pub const TGA_CMAPPED: i8 = 1;
pub const TGA_TRUECOLOR: i8 = 2;
pub const TGA_MONO: i8 = 3;
pub const TGA_CMAPPED_ENCODED: i8 = 9;
pub const TGA_TRUECOLOR_ENCODED: i8 = 10;
pub const TGA_MONO_ENCODED: i8 = 11;

pub const TGAIDF_ATTRIB_BITS: i8 = 0x0F;
pub const TGAIDF_XORIGIN: i8 = 1 << 4;
pub const TGAIDF_YORIGIN: i8 = 1 << 5;

pub const TGA_READMODE: i32 = 0;

pub const TGAERR_OPEN: i32 = -1;
pub const TGAERR_READ: i32 = -2;
pub const TGAERR_WRITE: i32 = -3;
pub const TGAERR_SYNTAX: i32 = -4;
pub const TGAERR_NOMEM: i32 = -5;
pub const TGAERR_NOTSUPPORTED: i32 = -6;

pub const TGAF_IMAGE: i32 = 1 << 0;
pub const TGAF_PAL: i32 = 1 << 1;
pub const TGAF_COMPRESS: i32 = 1 << 2;
pub const TGAF_TGA2: i32 = 1 << 3;

/// `TGA2_SIGNATURE`.
pub const TGA2_SIGNATURE: &[u8; 16] = b"TRUEVISION-XFILE";
/// `sizeof(TGAHeader)`.
pub const TGA_HEADER_SIZE: usize = 18;
/// `sizeof(TGA2Footer)`.
pub const TGA2_FOOTER_SIZE: usize = 26;
/// `sizeof(TGA2Extension)`.
pub const TGA2_EXTENSION_SIZE: usize = 495;

/// `TGA_BytesPerPixel(a)`: `(a + 7) >> 3`, on the signed pixel depth.
pub fn tga_bytes_per_pixel(pixel_depth: i8) -> i32 {
    (i32::from(pixel_depth) + 7) >> 3
}

/// Where upstream is undefined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TgaUndefined {
    /// A read from or write to the image buffer outside it.
    ImageOutOfRange,
    /// A write to the palette buffer outside it.
    PaletteOutOfRange,
    /// A signed 32-bit overflow in a size computation.
    Overflow,
    /// A read from the file before its start (after a backwards read).
    ReadBeforeStart,
}

/// Result of the Targa functions: upstream's error code (0 or a `TGAERR_`),
/// or where it is undefined.
pub type TgaResult<T> = Result<T, TgaUndefined>;

/// `TGAHeader`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TgaHeader {
    pub id_length: i8,
    pub color_map_type: i8,
    pub image_type: i8,
    pub cmap_start: i16,
    pub cmap_length: i16,
    pub cmap_depth: i8,
    pub x_offset: i16,
    pub y_offset: i16,
    pub width: i16,
    pub height: i16,
    pub pixel_depth: i8,
    pub image_descriptor: i8,
}

impl TgaHeader {
    pub fn from_bytes(b: &[u8; TGA_HEADER_SIZE]) -> Self {
        let s = |i: usize| i16::from_le_bytes([b[i], b[i + 1]]);
        TgaHeader {
            id_length: b[0] as i8,
            color_map_type: b[1] as i8,
            image_type: b[2] as i8,
            cmap_start: s(3),
            cmap_length: s(5),
            cmap_depth: b[7] as i8,
            x_offset: s(8),
            y_offset: s(10),
            width: s(12),
            height: s(14),
            pixel_depth: b[16] as i8,
            image_descriptor: b[17] as i8,
        }
    }

    pub fn to_bytes(&self) -> [u8; TGA_HEADER_SIZE] {
        let mut b = [0u8; TGA_HEADER_SIZE];
        b[0] = self.id_length as u8;
        b[1] = self.color_map_type as u8;
        b[2] = self.image_type as u8;
        b[3..5].copy_from_slice(&self.cmap_start.to_le_bytes());
        b[5..7].copy_from_slice(&self.cmap_length.to_le_bytes());
        b[7] = self.cmap_depth as u8;
        b[8..10].copy_from_slice(&self.x_offset.to_le_bytes());
        b[10..12].copy_from_slice(&self.y_offset.to_le_bytes());
        b[12..14].copy_from_slice(&self.width.to_le_bytes());
        b[14..16].copy_from_slice(&self.height.to_le_bytes());
        b[16] = self.pixel_depth as u8;
        b[17] = self.image_descriptor as u8;
        b
    }
}

/// `TGA2Footer`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tga2Footer {
    pub extension: i32,
    pub developer: i32,
    pub signature: [u8; 16],
    pub rsvd_char: u8,
    pub bzst: u8,
}

impl Tga2Footer {
    pub fn from_bytes(b: &[u8; TGA2_FOOTER_SIZE]) -> Self {
        let mut signature = [0u8; 16];
        signature.copy_from_slice(&b[8..24]);
        Tga2Footer {
            extension: i32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            developer: i32::from_le_bytes([b[4], b[5], b[6], b[7]]),
            signature,
            rsvd_char: b[24],
            bzst: b[25],
        }
    }
}

/// `TGA2Extension`, as its 495 packed bytes, with accessors for the fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tga2Extension {
    pub bytes: [u8; TGA2_EXTENSION_SIZE],
}

impl Default for Tga2Extension {
    fn default() -> Self {
        Tga2Extension { bytes: [0; TGA2_EXTENSION_SIZE] }
    }
}

impl Tga2Extension {
    fn i16_at(&self, o: usize) -> i16 {
        i16::from_le_bytes([self.bytes[o], self.bytes[o + 1]])
    }
    fn i32_at(&self, o: usize) -> i32 {
        i32::from_le_bytes([self.bytes[o], self.bytes[o + 1], self.bytes[o + 2], self.bytes[o + 3]])
    }
    /// `ExtSize`.
    pub fn ext_size(&self) -> i16 {
        self.i16_at(0)
    }
    /// `AuthName[41]`.
    pub fn auth_name(&self) -> &[u8] {
        &self.bytes[2..43]
    }
    /// `AuthComment[324]`.
    pub fn auth_comment(&self) -> &[u8] {
        &self.bytes[43..367]
    }
    /// `Date` (month, day, year).
    pub fn date(&self) -> [i16; 3] {
        [self.i16_at(367), self.i16_at(369), self.i16_at(371)]
    }
    /// `Time` (hour, minute, second).
    pub fn time(&self) -> [i16; 3] {
        [self.i16_at(373), self.i16_at(375), self.i16_at(377)]
    }
    /// `JobName[41]`.
    pub fn job_name(&self) -> &[u8] {
        &self.bytes[379..420]
    }
    /// `JobTime`.
    pub fn job_time(&self) -> [i16; 3] {
        [self.i16_at(420), self.i16_at(422), self.i16_at(424)]
    }
    /// `SoftID[41]`.
    pub fn soft_id(&self) -> &[u8] {
        &self.bytes[426..467]
    }
    /// `SoftVer` (number, letter).
    pub fn soft_ver(&self) -> (i16, i8) {
        (self.i16_at(467), self.bytes[469] as i8)
    }
    /// `KeyColor`.
    pub fn key_color(&self) -> i32 {
        self.i32_at(470)
    }
    /// `Aspect` (numerator, denominator).
    pub fn aspect(&self) -> (i16, i16) {
        (self.i16_at(474), self.i16_at(476))
    }
    /// `Gamma` (numerator, denominator).
    pub fn gamma(&self) -> (i16, i16) {
        (self.i16_at(478), self.i16_at(480))
    }
    /// `ColorCor`, `PostStamp`, `ScanLine` offsets.
    pub fn offsets(&self) -> [i32; 3] {
        [self.i32_at(482), self.i32_at(486), self.i32_at(490)]
    }
    /// `Attributes`.
    pub fn attributes(&self) -> i8 {
        self.bytes[494] as i8
    }
}

/// `class Targa`, reading.
#[derive(Clone, Debug, Default)]
pub struct Targa<'a> {
    pub header: TgaHeader,
    file: Option<GameFile<'a>>,
    access: i32,
    flags: i32,
    image: Option<Buf>,
    palette: Option<Buf>,
    extension: Tga2Extension,
}

/// An image or palette buffer of `size` bytes, of which the first
/// `data.len()` have been written (the rest are zeros here; `malloc`'s
/// uninitialised bytes upstream). Growing it as it is written keeps a header
/// that asks for a huge image from allocating it before any pixel is read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Buf {
    size: usize,
    data: Vec<u8>,
}

impl Buf {
    fn full(data: Vec<u8>) -> Self {
        Buf { size: data.len(), data }
    }

    /// Makes `[0, end)` addressable; false if `end` is past the buffer.
    fn ensure(&mut self, end: usize) -> bool {
        if end > self.size {
            return false;
        }
        if end > self.data.len() {
            self.data.resize(end, 0);
        }
        true
    }
}

fn file_err(_: crate::ww::chunkio::ChunkIoError) -> TgaUndefined {
    // the only errors GameFile::read gives at a valid destination
    TgaUndefined::ReadBeforeStart
}

impl<'a> Targa<'a> {
    /// `Targa()`.
    pub fn new() -> Self {
        Targa::default()
    }

    /// `mFlags` (`TGAF_*`).
    pub fn flags(&self) -> i32 {
        self.flags
    }

    /// `GetImage()`.
    ///
    /// After a successful load the whole image has been written; after a
    /// failed one, the bytes not read are zeros here (uninitialised
    /// upstream).
    pub fn image(&self) -> Option<&[u8]> {
        self.image.as_ref().map(|b| &b.data[..])
    }

    /// `GetPalette()`. A palette `Load` allocated is only written when the
    /// colour map length is positive (with a negative length and depth its
    /// size is positive, but it is skipped): its other bytes are zeros here,
    /// uninitialised upstream.
    pub fn palette(&self) -> Option<&[u8]> {
        self.palette.as_ref().map(|b| &b.data[..])
    }

    /// `SetPalette(buffer)`: frees a palette this Targa allocated, then uses
    /// `buffer`; returns the previous caller palette.
    pub fn set_palette(&mut self, buffer: Option<Vec<u8>>) -> Option<Vec<u8>> {
        if self.palette.is_some() && self.flags & TGAF_PAL != 0 {
            self.palette = None;
            self.flags &= !TGAF_PAL;
        }
        std::mem::replace(&mut self.palette, buffer.map(Buf::full)).map(|b| b.data)
    }

    /// `IsCompressed()`: `ImageType > 8`.
    pub fn is_compressed(&self) -> bool {
        self.header.image_type > 8
    }

    /// `GetExtension()`: the Targa 2.0 extension, if the flag is set.
    pub fn extension(&self) -> Option<&Tga2Extension> {
        (self.flags & TGAF_TGA2 != 0).then_some(&self.extension)
    }

    /// `mExtension` whatever the flag: the bytes the last extension read
    /// left (zeros before any).
    pub fn raw_extension(&self) -> &Tga2Extension {
        &self.extension
    }

    /// `Is_File_Open()`.
    pub fn is_open(&self) -> bool {
        self.file.is_some()
    }

    /// `Close()`.
    pub fn close(&mut self) {
        self.file = None;
    }

    fn f(&mut self) -> &mut GameFile<'a> {
        self.file.as_mut().expect("Targa file is open here")
    }

    /// `Open(name, TGA_READMODE)` on the file `data`: 0 or a `TGAERR_` code.
    pub fn open(&mut self, data: &'a [u8]) -> TgaResult<i32> {
        if self.is_open() && self.access == TGA_READMODE {
            return Ok(0);
        }
        self.close();
        self.access = TGA_READMODE;
        self.flags &= !TGAF_TGA2;
        let mut error = 0;
        // File_Open_Read: a file that does not fit an `int` cannot be a
        // RAMFile (not reachable from the game, whose archives are 32-bit)
        let Some(file) = GameFile::new(data) else {
            return Ok(TGAERR_OPEN);
        };
        self.file = Some(file);

        self.f().seek(-(TGA2_FOOTER_SIZE as i32), SeekFrom::End);
        let mut fb = [0u8; TGA2_FOOTER_SIZE];
        let n = self.f().read(&mut fb, TGA2_FOOTER_SIZE as i32).map_err(file_err)?;
        if n != TGA2_FOOTER_SIZE as i32 {
            error = TGAERR_READ;
        } else {
            let footer = Tga2Footer::from_bytes(&fb);
            if &footer.signature == TGA2_SIGNATURE && footer.extension != 0 {
                self.flags |= TGAF_TGA2;
            }
            if self.flags & TGAF_TGA2 != 0 {
                self.f().seek(footer.extension, SeekFrom::Set);
                let mut ext = self.extension.bytes;
                let n = self.f().read(&mut ext, TGA2_EXTENSION_SIZE as i32).map_err(file_err)?;
                self.extension.bytes = ext;
                if n != TGA2_EXTENSION_SIZE as i32 {
                    error = TGAERR_READ;
                }
            }
        }
        if error == 0 {
            self.f().seek(0, SeekFrom::Set);
        }
        let mut hb = self.header.to_bytes();
        let n = self.f().read(&mut hb, TGA_HEADER_SIZE as i32).map_err(file_err)?;
        self.header = TgaHeader::from_bytes(&hb);
        if n != TGA_HEADER_SIZE as i32 {
            error = TGAERR_READ;
        }
        if error == 0 && self.header.id_length != 0 {
            let id = i32::from(self.header.id_length);
            self.f().seek(id, SeekFrom::Cur);
        }
        if error != 0 {
            self.close();
        }
        Ok(error)
    }

    /// `(Header.Width * Header.Height) * TGA_BytesPerPixel(Header.PixelDepth)`
    /// in 32-bit `long`.
    fn image_size(&self) -> TgaResult<i32> {
        let wh = i32::from(self.header.width) * i32::from(self.header.height);
        wh.checked_mul(tga_bytes_per_pixel(self.header.pixel_depth)).ok_or(TgaUndefined::Overflow)
    }

    /// `Load(name, flags, invert_image)` on the file `data`: allocates the
    /// image (`TGAF_IMAGE`) and the palette (`TGAF_PAL`, if the file has a
    /// colour map and no palette was set) and reads them. 0 or a `TGAERR_`
    /// code.
    pub fn load(&mut self, data: &'a [u8], flags: i32, invert_image: bool) -> TgaResult<i32> {
        let mut error = 0;
        if self.open(data)? == 0 {
            if flags & TGAF_PAL != 0 && self.header.color_map_type == 1 {
                if self.palette.is_some() && self.flags & TGAF_PAL != 0 {
                    self.palette = None;
                    self.flags &= !TGAF_PAL;
                }
                if self.palette.is_none() && self.flags & TGAF_PAL == 0 {
                    let size = i32::from(self.header.cmap_length) * (i32::from(self.header.cmap_depth) >> 3);
                    if size != 0 {
                        match malloc(size) {
                            Some(b) => {
                                self.palette = Some(b);
                                self.flags |= TGAF_PAL;
                            }
                            None => error = TGAERR_NOMEM,
                        }
                    }
                }
            }
            if error == 0 && flags & TGAF_IMAGE != 0 {
                if self.image.is_some() && self.flags & TGAF_IMAGE != 0 {
                    self.image = None;
                    self.flags &= !TGAF_IMAGE;
                }
                if self.image.is_none() && self.flags & TGAF_IMAGE == 0 {
                    let size = self.image_size()?;
                    if size != 0 {
                        match malloc(size) {
                            Some(b) => {
                                self.image = Some(b);
                                self.flags |= TGAF_IMAGE;
                            }
                            None => error = TGAERR_NOMEM,
                        }
                    }
                }
            }
            if error == 0 {
                error = self.load_buffers(data, invert_image)?;
            }
            // the accessors give the whole buffers: bytes never written
            // (malloc's, uninitialised upstream) are zeros
            for b in [&mut self.image, &mut self.palette].into_iter().flatten() {
                let size = b.size;
                b.ensure(size);
            }
            self.close();
        } else {
            error = TGAERR_OPEN;
        }
        Ok(error)
    }

    /// `Load(name, mPalette, mImage, invert_image)`, as `Load(flags)`
    /// calls it.
    fn load_buffers(&mut self, data: &'a [u8], invert_image: bool) -> TgaResult<i32> {
        let mut error = 0;
        if self.open(data)? == 0 {
            if self.header.color_map_type == 1 {
                let depth = i32::from(self.header.cmap_depth) >> 3;
                let size = i32::from(self.header.cmap_length) * depth;
                if self.palette.is_some() && self.header.cmap_length > 0 {
                    let start = i64::from(self.header.cmap_start) * i64::from(depth);
                    let mut pal = self.palette.take().unwrap_or_default();
                    let r = read_at(self.f(), &mut pal, start, size, TgaUndefined::PaletteOutOfRange);
                    self.palette = Some(pal);
                    if r? != size {
                        error = TGAERR_READ;
                    }
                } else {
                    self.f().seek(size, SeekFrom::Cur);
                }
            }
            if error == 0 && self.image.is_some() {
                let size = self.image_size()?;
                match self.header.image_type {
                    TGA_CMAPPED | TGA_MONO => {
                        if self.read_image(size)? != size {
                            error = TGAERR_READ;
                        }
                    }
                    TGA_TRUECOLOR => {
                        if self.read_image(size)? == size {
                            if invert_image {
                                self.invert_image()?;
                            }
                        } else {
                            error = TGAERR_READ;
                        }
                    }
                    TGA_CMAPPED_ENCODED => error = self.decode_image()?,
                    TGA_TRUECOLOR_ENCODED => {
                        error = self.decode_image()?;
                        if error == 0 && invert_image {
                            self.invert_image()?;
                        }
                    }
                    _ => error = TGAERR_NOTSUPPORTED,
                }
                if error == 0 {
                    if self.header.image_descriptor & TGAIDF_XORIGIN != 0 {
                        self.x_flip()?;
                        self.header.image_descriptor &= !TGAIDF_XORIGIN;
                    }
                    if self.header.image_descriptor & TGAIDF_YORIGIN != 0 {
                        self.y_flip()?;
                        self.header.image_descriptor &= !TGAIDF_YORIGIN;
                    }
                }
            }
            self.close();
        } else {
            error = TGAERR_OPEN;
        }
        Ok(error)
    }

    fn read_image(&mut self, size: i32) -> TgaResult<i32> {
        let mut img = self.image.take().unwrap_or_default();
        let r = read_at(self.f(), &mut img, 0, size, TgaUndefined::ImageOutOfRange);
        self.image = Some(img);
        r
    }

    /// `DecodeImage()`: the run-length decoder.
    fn decode_image(&mut self) -> TgaResult<i32> {
        let mut img = self.image.take().unwrap_or_default();
        let r = self.decode_into(&mut img);
        self.image = Some(img);
        r
    }

    fn decode_into(&mut self, img: &mut Buf) -> TgaResult<i32> {
        let mut pos = 0usize;
        let depth = tga_bytes_per_pixel(self.header.pixel_depth) as u8;
        let mut pixel_count = (i32::from(self.header.width) * i32::from(self.header.height)) as u32;
        let mut error = 0;
        while pixel_count > 0 && error == 0 {
            let mut count = [0u8];
            if self.f().read(&mut count, 1).map_err(file_err)? == 1 {
                let mut count = count[0];
                if count & 0x80 != 0 {
                    count &= 0x7F;
                    count += 1;
                    let got = read_at(self.f(), img, pos as i64, i32::from(depth), TgaUndefined::ImageOutOfRange)?;
                    if got == i32::from(depth) {
                        let color = pos;
                        pos += usize::from(depth);
                        for _ in 1..count {
                            for i in 0..usize::from(depth) {
                                if !img.ensure(pos + 1) {
                                    return Err(TgaUndefined::ImageOutOfRange);
                                }
                                img.data[pos] = img.data[color + i];
                                pos += 1;
                            }
                        }
                    } else {
                        error = TGAERR_READ;
                    }
                } else {
                    count += 1;
                    let size = u32::from(count) * u32::from(depth);
                    let got = read_at(self.f(), img, pos as i64, size as i32, TgaUndefined::ImageOutOfRange)?;
                    if got as u32 == size {
                        pos += size as usize;
                    } else {
                        error = TGAERR_READ;
                    }
                }
                pixel_count = pixel_count.wrapping_sub(u32::from(count));
            } else {
                error = TGAERR_READ;
            }
        }
        Ok(error)
    }

    /// `InvertImage()`: reverses the bytes of each pixel (above 2 bytes).
    fn invert_image(&mut self) -> TgaResult<()> {
        let depth = tga_bytes_per_pixel(self.header.pixel_depth);
        let mut pixel_count = i32::from(self.header.width) * i32::from(self.header.height);
        if depth > 2 {
            let img = full_image(&mut self.image)?;
            let d = depth as usize;
            let mut at = 0usize;
            while pixel_count > 0 {
                let px = img.get_mut(at..at + d).ok_or(TgaUndefined::ImageOutOfRange)?;
                for i in 0..d / 2 {
                    px.swap(i, d - 1 - i);
                }
                pixel_count -= 1;
                at += d;
            }
        }
        Ok(())
    }

    /// `XFlip()`: mirrors each row.
    fn x_flip(&mut self) -> TgaResult<()> {
        // `char depth`
        let depth = tga_bytes_per_pixel(self.header.pixel_depth) as i8 as i64;
        let (w, h) = (i64::from(self.header.width), i64::from(self.header.height));
        let img = full_image(&mut self.image)?;
        let len = img.len() as i64;
        for y in 0..h {
            let mut ptr = w * depth * y;
            let mut ptr1 = ptr + (w * depth - depth);
            for _ in 0..(w / 2) {
                for d in 0..depth {
                    let (a, b) = (ptr + d, ptr1 + d);
                    if a < 0 || b < 0 || a >= len || b >= len {
                        return Err(TgaUndefined::ImageOutOfRange);
                    }
                    img.swap(a as usize, b as usize);
                }
                ptr += depth;
                ptr1 -= depth;
            }
        }
        Ok(())
    }

    /// `YFlip()`: swaps rows top to bottom with `_swapBytes`, whose byte
    /// tail does not swap (see the module docs). The pointer arithmetic is
    /// Win32's: 32-bit, wrapping.
    fn y_flip(&mut self) -> TgaResult<()> {
        let stride = (i32::from(self.header.width).wrapping_mul(tga_bytes_per_pixel(self.header.pixel_depth))) as u32;
        let img = full_image(&mut self.image)?;
        let mut top = 0u32;
        let mut bottom = stride.wrapping_mul((i32::from(self.header.height) - 1) as u32);
        let mut y = (i32::from(self.header.height) / 2) as u32;
        while y != 0 {
            swap_bytes(img, top, bottom, stride)?;
            y -= 1;
            top = top.wrapping_add(stride);
            bottom = bottom.wrapping_sub(stride);
        }
        Ok(())
    }
}

/// `_swapBytes(p1, p2, count)` (MSVC inline assembly): 16-byte blocks are
/// swapped as four dword exchanges, in its order; the remaining
/// `count & 15` bytes are read and written back unchanged
/// (`mov al,[esi]; xchg al,[edi]; mov [edi],al`).
fn swap_bytes(buf: &mut [u8], p1: u32, p2: u32, count: u32) -> TgaResult<()> {
    let len = buf.len() as u64;
    let blocks = count >> 4;
    let (mut esi, mut edi) = (u64::from(p1), u64::from(p2));
    let get = |b: &[u8], at: u64| u32::from_le_bytes([b[at as usize], b[at as usize + 1], b[at as usize + 2], b[at as usize + 3]]);
    let set = |b: &mut [u8], at: u64, v: u32| b[at as usize..at as usize + 4].copy_from_slice(&v.to_le_bytes());
    for _ in 0..blocks {
        if esi + 16 > len || edi + 16 > len {
            return Err(TgaUndefined::ImageOutOfRange);
        }
        for half in [0u64, 8] {
            let eax = get(buf, esi + half);
            let edx = get(buf, esi + half + 4);
            let old_eax = get(buf, edi + half);
            set(buf, edi + half, eax);
            let old_edx = get(buf, edi + half + 4);
            set(buf, edi + half + 4, edx);
            set(buf, esi + half, old_eax);
            set(buf, esi + half + 4, old_edx);
        }
        esi += 16;
        edi += 16;
    }
    let tail = u64::from(count & 15);
    if tail != 0 && (esi + tail > len || edi + tail > len) {
        return Err(TgaUndefined::ImageOutOfRange);
    }
    Ok(())
}

/// `malloc(size)` of a `long`: a negative size is a huge `size_t`, which
/// fails (NULL); otherwise a buffer (assumed to succeed).
fn malloc(size: i32) -> Option<Buf> {
    Some(Buf { size: usize::try_from(size).ok()?, data: Vec::new() })
}

/// The whole image buffer, for the functions that run after it has been
/// read (`InvertImage`, the flips).
fn full_image(image: &mut Option<Buf>) -> TgaResult<&mut [u8]> {
    let b = image.as_mut().ok_or(TgaUndefined::ImageOutOfRange)?;
    let size = b.size;
    b.ensure(size);
    Ok(&mut b.data[..])
}

/// `File_Read(buffer + offset, size)`: what the read copies must land inside
/// `buf` (else `oob`); returns the count (negative for a backwards read).
fn read_at(file: &mut GameFile<'_>, buf: &mut Buf, offset: i64, size: i32, oob: TgaUndefined) -> TgaResult<i32> {
    // RAMFile::read copies min(size, what is left)
    let left = file.file_size().wrapping_sub(file.position());
    let n = size.min(left);
    if n <= 0 {
        return file.read(&mut [], size).map_err(file_err);
    }
    if file.position() < 0 {
        return Err(TgaUndefined::ReadBeforeStart);
    }
    let end = offset + i64::from(n);
    if offset < 0 || end > buf.size as i64 || !buf.ensure(end as usize) {
        return Err(oob);
    }
    file.read(&mut buf.data[offset as usize..end as usize], size).map_err(file_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tga(w: i16, h: i16, depth: i8, image_type: i8, desc: i8, pixels: &[u8]) -> Vec<u8> {
        let hd = TgaHeader { image_type, width: w, height: h, pixel_depth: depth, image_descriptor: desc, ..Default::default() };
        let mut f = hd.to_bytes().to_vec();
        f.extend_from_slice(pixels);
        f.extend_from_slice(&[0; 26]);
        f
    }

    #[test]
    fn truecolor_and_rle() {
        let px: Vec<u8> = (0..2 * 2 * 4).collect();
        let f = tga(2, 2, 32, TGA_TRUECOLOR, 0, &px);
        let mut t = Targa::new();
        assert_eq!(t.load(&f, TGAF_IMAGE, true), Ok(0));
        assert_eq!(&t.image().unwrap()[..4], &[3, 2, 1, 0]);
        // RLE: a run of 3 pixels then 1 raw pixel, 24-bit
        let f = tga(2, 2, 24, TGA_TRUECOLOR_ENCODED, 0, &[0x82, 1, 2, 3, 0x00, 4, 5, 6]);
        let mut t = Targa::new();
        assert_eq!(t.load(&f, TGAF_IMAGE, false), Ok(0));
        assert_eq!(t.image().unwrap(), &[1, 2, 3, 1, 2, 3, 1, 2, 3, 4, 5, 6]);
        // a run past the end of the image is undefined
        let f = tga(1, 1, 24, TGA_TRUECOLOR_ENCODED, 0, &[0x81, 1, 2, 3]);
        assert_eq!(Targa::new().load(&f, TGAF_IMAGE, false), Err(TgaUndefined::ImageOutOfRange));
    }

    #[test]
    fn y_flip_leaves_the_row_tail() {
        // 4 pixels of 24 bits: 12 bytes a row, all in the unswapped tail
        let px: Vec<u8> = (0..24).collect();
        let f = tga(4, 2, 24, TGA_TRUECOLOR, TGAIDF_YORIGIN, &px);
        let mut t = Targa::new();
        assert_eq!(t.load(&f, TGAF_IMAGE, false), Ok(0));
        assert_eq!(t.image().unwrap(), &px[..]);
        // 8 pixels of 24 bits: the first 16 bytes of each row swap
        let px: Vec<u8> = (0..48).collect();
        let f = tga(8, 2, 24, TGA_TRUECOLOR, TGAIDF_YORIGIN, &px);
        let mut t = Targa::new();
        assert_eq!(t.load(&f, TGAF_IMAGE, false), Ok(0));
        let img = t.image().unwrap();
        assert_eq!(&img[..16], &px[24..40]);
        assert_eq!(&img[16..24], &px[16..24]);
        assert_eq!(&img[24..40], &px[..16]);
    }
}
