// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/System/Image.cpp (Image's field parse table, parseImageCoords,
//     parseImageStatus, the constructor, setStatus, clearStatus; ImageCollection's addImage, findImageByName, load)
//   GeneralsMD/Code/GameEngine/Include/GameClient/Image.h (ImageStatus, imageStatusNames)
//   GeneralsMD/Code/GameEngine/Source/Common/INI/INIMappedImage.cpp (INI::parseMappedImageDefinition)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Image.cpp: Colin Day, June 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Mapped images (GitHub #12): `TheMappedImageCollection`, the named rectangles of the UI's texture pages that the
//! window layouts, the command buttons and the control bar scheme draw with.
//!
//! # As upstream reads it
//!
//! `ImageCollection::load(512)` (GameClient.cpp:280) loads every INI file under `Data\INI\MappedImages\TextureSize_512`,
//! then under `Data\INI\MappedImages\HandCreated`, as `INI_LOAD_OVERWRITE`. Each `MappedImage name` block
//! (`INI::parseMappedImageDefinition`) finds the image of that name (case-insensitively) or makes one, then reads its
//! fields into it:
//!
//! ```text
//! MappedImage SCSmShellUserMB_01
//!   Texture = SCSmShellUserMB.tga
//!   TextureWidth = 512
//!   TextureHeight = 256
//!   Coords = Left:0 Top:0 Right:113 Bottom:39
//!   Status = NONE
//! End
//! ```
//!
//! `Coords` stores the rectangle's size in pixels and its UVs, the pixel edges divided by `TextureWidth` and
//! `TextureHeight` **as read so far** (an axis whose size is still 0 keeps its pixels). `Status = ROTATED_90_CLOCKWISE`
//! swaps the size read so far (the image is stored rotated on its page, as the ImagePacker tool packs it).
//!
//! # Upstream quirks that are kept
//!
//! - The order of the fields matters: `Coords` before `TextureWidth` keeps pixel coordinates as UVs, and `Status`
//!   swaps whatever size `Coords` gave before it (a second `Coords` after it un-swaps it).
//! - A second block of the same name (any case) changes the first image: there are no overrides ("images are assets
//!   as they are"), and the image keeps the first block's spelling of its name.
//! - `Status` is `parseBitString32`: a plain list replaces the bits, `+NAME`/`-NAME` change them; `NONE` clears them.
//!   The swap happens whenever the bit is set after the parse, also when it was set before.
//!
//! # Where this differs from upstream
//!
//! - The collection is keyed by the lowercased name rather than by `nameToLowercaseKey(name)`: the same lookup, without
//!   giving the client's images keys in the logic's name key generator.
//! - The user's own `INI\MappedImages\*.ini` (in the player's user data folder, loaded first upstream) are not read.
//! - `m_rawTextureData` (images made at run time from raw texture data, the `RAW_TEXTURE` status) is not kept.
//!
//! # Representation
//!
//! [`Image`] holds the fields; [`ImageCollection`] the images by lowercased name. The texture is a file name the
//! client opens (`Art\Textures\<name>`); the UVs are `f32`, as upstream's `Real`: the client's drawing, not the logic.

use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::logic::world::game_data::GameDataFiles;
use std::collections::BTreeMap;

/// `IMAGE_STATUS_ROTATED_90_CLOCKWISE`.
pub const IMAGE_STATUS_ROTATED_90_CLOCKWISE: u32 = 0x0000_0001;
/// `IMAGE_STATUS_RAW_TEXTURE`.
pub const IMAGE_STATUS_RAW_TEXTURE: u32 = 0x0000_0002;
/// `imageStatusNames`.
pub const IMAGE_STATUS_NAMES: &[&str] = &["ROTATED_90_CLOCKWISE", "RAW_TEXTURE"];

/// `class Image`.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// `m_name`.
    pub name: Vec<u8>,
    /// `m_filename`: the texture page.
    pub filename: Vec<u8>,
    /// `m_textureSize`.
    pub texture_size: (i32, i32),
    /// `m_UVCoords`: (lo.x, lo.y, hi.x, hi.y).
    pub uv: [f32; 4],
    /// `m_imageSize`.
    pub image_size: (i32, i32),
    /// `m_status`.
    pub status: u32,
}

impl Image {
    /// `Image::Image()`, with its name.
    pub fn new(name: &[u8]) -> Self {
        Image { name: name.to_vec(), filename: Vec::new(), texture_size: (0, 0), uv: [0.0, 0.0, 1.0, 1.0], image_size: (0, 0), status: 0 }
    }
    /// `getImageWidth()`.
    pub fn width(&self) -> i32 {
        self.image_size.0
    }
    /// `getImageHeight()`.
    pub fn height(&self) -> i32 {
        self.image_size.1
    }
    /// `setStatus(bit)`: the status before.
    pub fn set_status(&mut self, bit: u32) -> u32 {
        let prev = self.status;
        self.status |= bit;
        prev
    }
    /// `clearStatus(bit)`: the status before.
    pub fn clear_status(&mut self, bit: u32) -> u32 {
        let prev = self.status;
        self.status &= !bit;
        prev
    }
    /// `getStatus() & IMAGE_STATUS_ROTATED_90_CLOCKWISE`.
    pub fn is_rotated(&self) -> bool {
        self.status & IMAGE_STATUS_ROTATED_90_CLOCKWISE != 0
    }
}

/// `Image::parseImageCoords`: `Left:L Top:T Right:R Bottom:B`.
fn parse_image_coords(ini: &mut Ini<'_>, image: &mut Image, _: &UserData) -> IniResult<()> {
    let left = Ini::scan_int(&ini.get_next_sub_token(b"Left")?)?;
    let top = Ini::scan_int(&ini.get_next_sub_token(b"Top")?)?;
    let right = Ini::scan_int(&ini.get_next_sub_token(b"Right")?)?;
    let bottom = Ini::scan_int(&ini.get_next_sub_token(b"Bottom")?)?;
    let mut uv = [left as f32, top as f32, right as f32, bottom as f32];
    let (tw, th) = image.texture_size;
    if tw != 0 {
        uv[0] /= tw as f32;
        uv[2] /= tw as f32;
    }
    if th != 0 {
        uv[1] /= th as f32;
        uv[3] /= th as f32;
    }
    image.uv = uv;
    // (`right - left` on Int: wraps, as the differential build's -fwrapv)
    image.image_size = (right.wrapping_sub(left), bottom.wrapping_sub(top));
    Ok(())
}

/// `Image::parseImageStatus`: the bit string, then the swap of a rotated image's size.
fn parse_image_status(ini: &mut Ini<'_>, image: &mut Image, _: &UserData) -> IniResult<()> {
    ini.parse_bit_string32(&mut image.status, IMAGE_STATUS_NAMES)?;
    if image.is_rotated() {
        image.image_size = (image.height(), image.width());
    }
    Ok(())
}

/// `Image::m_imageFieldParseTable`.
pub const IMAGE_FIELD_PARSE_TABLE: &[FieldParse<Image>] = &[
    FieldParse::new("Texture", |ini, i, _| ini.parse_ascii_string().map(|s| i.filename = s), UserData::None),
    FieldParse::new("TextureWidth", |ini, i, _| ini.parse_int().map(|v| i.texture_size.0 = v), UserData::None),
    FieldParse::new("TextureHeight", |ini, i, _| ini.parse_int().map(|v| i.texture_size.1 = v), UserData::None),
    FieldParse::new("Coords", parse_image_coords, UserData::None),
    FieldParse::new("Status", parse_image_status, UserData::None),
    FieldParse::end(),
];

/// `class ImageCollection` (`TheMappedImageCollection`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageCollection {
    /// `m_imageMap`, by lowercased name (see the module docs).
    images: BTreeMap<Vec<u8>, Image>,
}

impl ImageCollection {
    /// `ImageCollection::ImageCollection()`.
    pub fn new() -> Self {
        Self::default()
    }
    /// `addImage(image)`: replaces an image of the same name (any case).
    pub fn add_image(&mut self, image: Image) {
        self.images.insert(image.name.to_ascii_lowercase(), image);
    }
    /// `findImageByName(name)`, case-insensitively.
    pub fn find_image_by_name(&self, name: &[u8]) -> Option<&Image> {
        self.images.get(&name.to_ascii_lowercase())
    }
    /// The number of images.
    pub fn len(&self) -> usize {
        self.images.len()
    }
    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
    /// Every image, by lowercased name.
    pub fn iter(&self) -> impl Iterator<Item = &Image> {
        self.images.values()
    }

    /// `INI::load(path, INI_LOAD_OVERWRITE)` of one file's text, with the `MappedImage` block only (upstream's block
    /// table has 62 types; the other blocks are not found in the mapped image files).
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, MAPPED_IMAGE_BLOCKS, self)
    }

    /// `ImageCollection::load(textureSize)`: `Data\INI\MappedImages\TextureSize_<size>`, then
    /// `Data\INI\MappedImages\HandCreated`, each through `INI::loadDirectory` (the user's own folder is not read).
    pub fn load(&mut self, texture_size: i32, files: &impl GameDataFiles) -> IniResult<()> {
        let dir = format!("Data\\INI\\MappedImages\\TextureSize_{texture_size}").into_bytes();
        for d in [dir.as_slice(), b"Data\\INI\\MappedImages\\HandCreated"] {
            let list = files.list_ini(d);
            Ini::load_directory(d, true, IniLoadType::Overwrite, None, &list, |p| files.open(p), MAPPED_IMAGE_BLOCKS, self)?;
        }
        Ok(())
    }
}

/// `INI::parseMappedImageDefinition`: the image of the block's name (found, or new and added), then its fields.
pub fn parse_mapped_image_definition(ini: &mut Ini<'_>, coll: &mut ImageCollection) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let key = name.to_ascii_lowercase();
    let mut image = coll.images.remove(&key).unwrap_or_else(|| Image::new(&name));
    let r = ini.init_from_ini(&mut image, IMAGE_FIELD_PARSE_TABLE);
    // (the image is in the collection before its fields are read, so it stays there when a field throws)
    coll.images.insert(key, image);
    r
}

/// The block table the mapped image files are loaded with.
pub const MAPPED_IMAGE_BLOCKS: &[BlockParse<ImageCollection>] = &[BlockParse { token: "MappedImage", parse: parse_mapped_image_definition }];

#[cfg(test)]
mod tests {
    use super::*;

    /// #12: mapped images. Predicted from Image.cpp before the run:
    /// - `A` (512x256 page, coords 0,0..128,64): size 128x64, UVs (0, 0, 0.25, 0.25), texture `a.tga`;
    /// - `B` with `Coords` before `TextureWidth`: UVs stay pixels on x and y (64, 32, 96, 48), size 32x16;
    /// - `C` rotated: Coords 0,0..10,40 then `Status = ROTATED_90_CLOCKWISE`: size 40x10, status 1;
    /// - a second block `a` (lower case) changes `A`'s `Texture` to `a2.tga` and keeps the rest and the name `A`;
    /// - three images; `find_image_by_name(b"c")` finds `C`; `D` is not there.
    #[test]
    fn mapped_images_parse_as_upstream() {
        let text = b"MappedImage A\n  Texture = a.tga\n  TextureWidth = 512\n  TextureHeight = 256\n  Coords = Left:0 Top:0 Right:128 Bottom:64\n  Status = NONE\nEnd\n\
MappedImage B\n  Texture = b.tga\n  Coords = Left:64 Top:32 Right:96 Bottom:48\n  TextureWidth = 256\n  TextureHeight = 256\nEnd\n\
MappedImage C\n  Coords = Left:0 Top:0 Right:10 Bottom:40\n  Status = ROTATED_90_CLOCKWISE\nEnd\n\
MappedImage a\n  Texture = a2.tga\nEnd\n";
        let mut c = ImageCollection::new();
        c.load_text(b"Data\\INI\\MappedImages\\TextureSize_512\\t.ini", text).expect("parse");
        assert_eq!(c.len(), 3);
        let a = c.find_image_by_name(b"A").unwrap();
        assert_eq!((a.name.as_slice(), a.filename.as_slice(), a.image_size, a.uv), (&b"A"[..], &b"a2.tga"[..], (128, 64), [0.0, 0.0, 0.25, 0.25]));
        let b = c.find_image_by_name(b"b").unwrap();
        assert_eq!((b.uv, b.image_size), ([64.0, 32.0, 96.0, 48.0], (32, 16)));
        let cc = c.find_image_by_name(b"c").unwrap();
        assert_eq!((cc.image_size, cc.status, cc.is_rotated()), ((40, 10), 1, true));
        assert!(c.find_image_by_name(b"D").is_none());
    }
}
