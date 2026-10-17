// SPDX-License-Identifier: GPL-3.0-only
//! Texture files as WW3D2 and WWLib read them, into raw pixel buffers (no
//! Direct3D): Targa images ([`targa`]), DDS files with upstream's DXT1/DXT5
//! decompression ([`dds`]), and what they share: surface formats and the
//! pixel writer ([`format`]) and the hue shift textures are loaded with
//! ([`colorspace`]).

pub mod colorspace;
pub mod dds;
pub mod format;
pub mod targa;
